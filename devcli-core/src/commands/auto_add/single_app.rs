// Single app detection and addition functionality
// Handles detection of individual applications (not in Nx monorepos)
// Supports discovery of multiple apps in subdirectories

use crate::config::{save_config, App, Commands, Defaults, Project};
use crate::detection::detect_app;
use crate::Result;
use std::collections::HashMap;
use std::env;
use std::io::IsTerminal;

use super::interactive::{
    confirm_default_yes, interactive_app_selection, prompt_app_name, prompt_project_selection,
    show_preview,
};
use super::validation::validate_app_name;
use serde_json::json;

/// Arguments for the auto-add command.
#[derive(Debug, Clone, Default)]
pub struct AutoAddArgs {
    /// Optional path to scan (defaults to the current directory)
    pub path: Option<String>,
    /// Non-interactive: accept detected defaults without prompting
    pub yes: bool,
    /// Target project name (non-interactive)
    pub project: Option<String>,
    /// App name to register (non-interactive)
    pub name: Option<String>,
    /// App type override (non-interactive)
    pub app_type: Option<String>,
    /// Replace an app that already exists (its dependencies, health check,
    /// restart policy and alternative name are kept)
    pub force: bool,
}

impl AutoAddArgs {
    /// True when any non-interactive flag was supplied.
    pub fn is_non_interactive(&self) -> bool {
        // `--json` means a program is reading the output: never prompt it (many
        // agents run in a pseudo-terminal, so the TTY check alone is not enough).
        self.yes
            || self.project.is_some()
            || self.name.is_some()
            || self.app_type.is_some()
            || crate::output::json_enabled()
    }
}

// Entry point for the auto-add command
// Detects if we're in an Nx monorepo or a single app, then handles accordingly
// Args:
//   - args.path: Optional custom path to scan (defaults to current directory)
//   - args.yes/--project/--name/--type: non-interactive mode for agents
/// App types devcli knows how to generate commands and defaults for.
pub const KNOWN_APP_TYPES: &[&str] = &[
    "nodejs",
    "nx",
    "nx-workspace",
    "rust",
    "python",
    "docker",
    "redis",
    "traefik",
];

pub async fn auto_add_command(args: AutoAddArgs) -> Result<()> {
    // A typo (`--type nodjs`) used to be saved as-is.
    if let Some(app_type) = &args.app_type
        && !KNOWN_APP_TYPES.contains(&app_type.as_str())
    {
        let candidates: Vec<String> = KNOWN_APP_TYPES.iter().map(|t| t.to_string()).collect();
        let hint = config_manager::resolver::fuzzy_match(app_type, &candidates, 3)
            .map(|t| format!(" Did you mean '{t}'?"))
            .unwrap_or_default();
        anyhow::bail!(
            "Unknown app type '{}'.{} Known types: {}.",
            app_type,
            hint,
            KNOWN_APP_TYPES.join(", ")
        );
    }

    // Use provided path or default to current directory
    let target_path = if let Some(p) = &args.path {
        std::path::PathBuf::from(p)
    } else {
        env::current_dir()?
    };

    let target_path = normalize_scan_root(&target_path);

    // Verify the path exists before proceeding
    if !target_path.exists() {
        anyhow::bail!("Path does not exist: {}", target_path.display());
    }

    // Refuse to hang on a prompt when there is no terminal to answer it.
    if !args.is_non_interactive() && !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "auto-add is interactive but no terminal is attached.\n\
             Pass --yes (optionally with --project/--name/--type) for non-interactive use."
        );
    }

    if !crate::output::json_enabled() {
        println!("Detecting app in {}...\n", target_path.display());
    }

    // Check if we're in an Nx monorepo by looking for nx.json
    // Nx monorepos need special handling to detect multiple apps
    if target_path.join("nx.json").exists() {
        if args.is_non_interactive() {
            anyhow::bail!(
                "Nx monorepo detected at {}.\n\
                 Non-interactive auto-add does not support monorepos yet; run it interactively.",
                target_path.display()
            );
        }
        super::nx_monorepo::handle_nx_monorepo(&target_path, args.force).await
    } else {
        handle_single_app(&target_path, &args).await
    }
}

// Handle detection and addition of a single app (not in an Nx monorepo)
// Detects the app type, prompts for confirmation, and adds to config
pub async fn handle_single_app(target_path: &std::path::Path, args: &AutoAddArgs) -> Result<()> {
    // Check if there are multiple potential apps in subdirectories
    let discovered_apps = discover_all_apps(target_path)?;

    if discovered_apps.len() > 1 {
        if args.is_non_interactive() {
            anyhow::bail!(
                "Multiple apps detected under {}; non-interactive auto-add needs a single app.\n\
                 Point --path at a specific app directory.",
                target_path.display()
            );
        }
        // Multiple apps found, let user choose
        return handle_multiple_apps(discovered_apps, args.force).await;
    }

    // Single app case - use the first (and only) discovered app
    let mut detected = if discovered_apps.is_empty() {
        // Fallback to direct detection if discovery found nothing
        detect_app(target_path)?
    } else {
        discovered_apps
            .into_iter()
            .next()
            .expect("BUG: discovered_apps should not be empty after is_empty check")
    };

    // Apply an explicit type override before previewing/persisting.
    if let Some(app_type) = &args.app_type {
        detected.app_type = app_type.clone();
    }

    // Display what was detected
    if !crate::output::json_enabled() {
        println!("✓ Detected: {} app", detected.app_type);

        if let Some(local_cmds) = &detected.local_commands {
            println!("  - Local: {} commands available", local_cmds.len());
        }
        if let Some(docker_cmds) = &detected.docker_commands {
            println!("  - Docker: {} commands available", docker_cmds.len());
        }
        if let Some(orbstack_cmds) = &detected.orbstack_commands {
            println!("  - OrbStack: {} commands available", orbstack_cmds.len());
        }
        if let Some(k8s_cmds) = &detected.k8s_commands {
            println!("  - Kubernetes: {} commands available", k8s_cmds.len());
        }
    }

    // Apps need at least one environment to be useful
    if detected.local_commands.is_none()
        && detected.docker_commands.is_none()
        && detected.orbstack_commands.is_none()
        && detected.k8s_commands.is_none()
    {
        anyhow::bail!("No commands detected for this app. Cannot add to config.");
    }

    if !crate::output::json_enabled() {
        println!();
    }

    // Load existing config or create empty one
    let config = crate::config::loader::load_config_or_new()?;

    let (project_name, app_name) = if args.is_non_interactive() {
        let project_name = args.project.clone().unwrap_or_else(|| "global".to_string());
        let app_name = args
            .name
            .clone()
            .unwrap_or_else(|| detected.app_name.clone());
        validate_app_name(&app_name)?;
        (project_name, app_name)
    } else {
        // Prompt for project selection
        let project_name = prompt_project_selection(&config)?;
        // Prompt to confirm or edit the detected app name
        let app_name = prompt_app_name(&detected.app_name)?;
        (project_name, app_name)
    };

    let exists = config
        .projects
        .get(&project_name)
        .is_some_and(|p| p.apps.contains_key(&app_name));
    let mut force = args.force;
    if exists && !force && !args.is_non_interactive() {
        force = super::interactive::confirm_default_no(&format!(
            "'{project_name}/{app_name}' already exists. Replace its detected settings \
             (dependencies, health check, restart policy and alternative name are kept)?"
        ))?;
        if !force {
            println!("Cancelled.");
            return Ok(());
        }
    }

    if !args.is_non_interactive() {
        // Show preview of what will be added
        show_preview(&project_name, &app_name, &detected);

        // Final confirmation before adding (default to yes)
        if !confirm_default_yes("Add to config?")? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Add to config and save
    let json = crate::output::json_enabled();
    let result = json!({
        "project": project_name,
        "app": app_name,
        "type": detected.app_type,
        // Absolute, as AGENTS.md documents; the config itself keeps `~/...`.
        "path": crate::utils::path::expand_path(&detected.path),
    });
    let non_interactive = args.is_non_interactive();
    add_to_config(
        config,
        project_name.clone(),
        app_name.clone(),
        detected,
        non_interactive,
        force,
    )?;

    if json {
        crate::output::print_json(&result)?;
    } else {
        println!("✓ Added successfully!");
    }

    Ok(())
}

// Discover all potential apps in a directory and its subdirectories
// Returns a list of detected apps for user selection
// Creates separate apps for each config file to allow fine-grained control
pub fn discover_all_apps(
    root_path: &std::path::Path,
) -> Result<Vec<crate::detection::DetectedApp>> {
    let root_path = normalize_scan_root(root_path);
    let mut discovered_apps = Vec::new();
    let workspace_members = crate::app_detector_support::cargo_workspace_member_dirs(&root_path);

    // Discover individual config files in the root directory
    discovered_apps.extend(discover_individual_apps(&root_path)?);

    // Scan subdirectories for additional apps (1 level deep)
    if let Ok(entries) = std::fs::read_dir(&root_path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let dir_name = entry.file_name();
                let dir_name_str = dir_name.to_string_lossy();
                if should_skip_scan_dir(&dir_name_str) {
                    continue;
                }

                if workspace_members
                    .as_ref()
                    .is_some_and(|members| members.contains(dir_name_str.as_ref()))
                {
                    continue;
                }

                // Discover individual apps in this subdirectory
                if has_project_manifest(&entry.path()) {
                    discovered_apps.extend(discover_individual_apps(&entry.path())?);
                }

                // Standard monorepo container dirs (apps/, packages/, libs/, …)
                // hold the real projects one level deeper — recurse into them
                // so e.g. `apps/web/package.json` is discovered, not just the
                // root-level app.
                if is_workspace_container_dir(&dir_name_str) {
                    discovered_apps.extend(discover_apps_in_container(
                        &entry.path(),
                        workspace_members.as_ref(),
                    )?);
                }
            }
        }
    }

    Ok(discovered_apps)
}

/// Standard monorepo container directory names whose direct children are
/// individual projects (e.g. `apps/web`, `packages/shared`, `libs/ui`).
fn is_workspace_container_dir(dir_name: &str) -> bool {
    matches!(
        dir_name,
        "apps" | "packages" | "libs" | "modules" | "services"
    )
}

/// Discover the sub-apps inside a workspace container dir.
///
/// Only recurses one level: `apps/web` is scanned, `apps/web/deeper` is not.
/// Cargo workspace members (leaf names) are skipped to avoid double-detection.
fn discover_apps_in_container(
    container_path: &std::path::Path,
    workspace_members: Option<&std::collections::HashSet<String>>,
) -> Result<Vec<crate::detection::DetectedApp>> {
    let mut apps = Vec::new();

    if let Ok(entries) = std::fs::read_dir(container_path) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() {
                continue;
            }

            let dir_name = entry.file_name();
            let dir_name_str = dir_name.to_string_lossy();
            if should_skip_scan_dir(&dir_name_str) {
                continue;
            }

            if workspace_members.is_some_and(|members| members.contains(dir_name_str.as_ref())) {
                continue;
            }

            if has_project_manifest(&entry.path()) {
                apps.extend(discover_individual_apps(&entry.path())?);
            }
        }
    }

    Ok(apps)
}

fn normalize_scan_root(path: &std::path::Path) -> std::path::PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn should_skip_scan_dir(dir_name: &str) -> bool {
    // Hidden directories are tooling state (.git, .venv, .turbo, .nx, .tox,
    // .mypy_cache, ...), never an app of their own.
    dir_name.starts_with('.')
        || matches!(
            dir_name,
            "node_modules"
                | "venv"
                | "__pycache__"
                | "vendor"
                | "target"
                | "dist"
                | "build"
                | ".next"
                | "coverage"
                | ".cursor"
                | ".github"
                | ".obsidian"
                | ".kiro"
                | "docs"
                | "scripts"
                | "reports"
        )
}

/// Whether `dir` holds a project of its own rather than source files of the
/// project around it.
///
/// Without this, a Python package directory (`app/` of a FastAPI service) was
/// detected as a second app through its `.py` files, and `auto-add --yes`
/// refused the very directory that is the app ("Multiple apps detected").
fn has_project_manifest(dir: &std::path::Path) -> bool {
    const MANIFESTS: &[&str] = &[
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "requirements.txt",
        "setup.py",
        "Pipfile",
        "go.mod",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "Gemfile",
        "composer.json",
        "mix.exs",
        "project.json",
        "Dockerfile",
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ];
    if MANIFESTS.iter().any(|m| dir.join(m).is_file()) {
        return true;
    }
    // Config-file apps (redis*.conf, traefik*.yml) are discovered by file name.
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            (name.starts_with("redis") && (name.ends_with(".conf") || name.ends_with(".config")))
                || (name.starts_with("traefik")
                    && (name.ends_with(".yml")
                        || name.ends_with(".yaml")
                        || name.ends_with(".toml")))
        })
    })
}

// Discover individual config files as separate apps in a single directory
// This allows users to select specific config files (e.g., redis.local.conf vs redis.prod.conf)
fn discover_individual_apps(
    dir_path: &std::path::Path,
) -> Result<Vec<crate::detection::DetectedApp>> {
    let mut apps = Vec::new();

    // First try the standard detection (for Node.js, Python, etc.)
    if let Ok(app) = detect_app(dir_path) {
        // Only add if it's not a config-file-based app (redis/traefik)
        // We'll handle those separately to get individual config files
        if !matches!(app.app_type.as_str(), "redis" | "traefik") {
            apps.push(app);
        }
    }

    // Now discover individual Redis and Traefik config files
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            if entry.path().is_file() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();

                // Check for individual Redis config files
                if file_name_str.starts_with("redis")
                    && (file_name_str.ends_with(".conf") || file_name_str.ends_with(".config"))
                    && let Ok(app) = create_redis_app_from_config(&entry.path(), &file_name_str)
                {
                    apps.push(app);
                }

                // Check for individual Traefik config files
                if file_name_str.starts_with("traefik")
                    && (file_name_str.ends_with(".yml")
                        || file_name_str.ends_with(".yaml")
                        || file_name_str.ends_with(".toml"))
                    && let Ok(app) = create_traefik_app_from_config(&entry.path(), &file_name_str)
                {
                    apps.push(app);
                }
            }
        }
    }

    Ok(apps)
}

// Create a Redis app from a specific config file
fn create_redis_app_from_config(
    config_path: &std::path::Path,
    config_filename: &str,
) -> Result<crate::detection::DetectedApp> {
    let dir_path = config_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "Config file path has no parent directory: {}",
            config_path.display()
        )
    })?;

    // Extract app name from config filename (remove .conf/.config extension)
    let app_name = if let Some(name) = config_filename.strip_suffix(".conf") {
        name.to_string()
    } else if let Some(name) = config_filename.strip_suffix(".config") {
        name.to_string()
    } else {
        config_filename.to_string()
    };

    // Create commands for this specific config - use the config file as the main start command
    let mut local_commands = HashMap::new();
    local_commands.insert(
        "start".to_string(),
        format!("redis-server {}", config_filename),
    );

    Ok(crate::detection::DetectedApp {
        app_type: "redis".to_string(),
        app_name,
        path: crate::utils::path::contract_tilde(dir_path),
        local_commands: Some(local_commands),
        docker_commands: None,   // Could add Docker detection later
        orbstack_commands: None, // Could add OrbStack detection later
        k8s_commands: None,      // Could add K8s detection later
        suggested_local_default: Some("start".to_string()),
        suggested_docker_default: None,
        suggested_orbstack_default: None,
        dockerfile_path: None,
        env_files: None, // No env files detected for Redis
    })
}

// Create a Traefik app from a specific config file
fn create_traefik_app_from_config(
    config_path: &std::path::Path,
    config_filename: &str,
) -> Result<crate::detection::DetectedApp> {
    let dir_path = config_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "Config file path has no parent directory: {}",
            config_path.display()
        )
    })?;

    // Extract app name from config filename (remove extension)
    let app_name = if let Some(name) = config_filename.strip_suffix(".yml") {
        name.to_string()
    } else if let Some(name) = config_filename.strip_suffix(".yaml") {
        name.to_string()
    } else if let Some(name) = config_filename.strip_suffix(".toml") {
        name.to_string()
    } else {
        config_filename.to_string()
    };

    // Create commands for this specific config
    let mut local_commands = HashMap::new();
    local_commands.insert(
        "start".to_string(),
        format!("traefik --configFile={}", config_filename),
    );

    Ok(crate::detection::DetectedApp {
        app_type: "traefik".to_string(),
        app_name,
        path: crate::utils::path::contract_tilde(dir_path),
        local_commands: Some(local_commands),
        docker_commands: None,   // Could add Docker detection later
        orbstack_commands: None, // Could add OrbStack detection later
        k8s_commands: None,      // Could add K8s detection later
        suggested_local_default: Some("start".to_string()),
        suggested_docker_default: None,
        suggested_orbstack_default: None,
        dockerfile_path: None,
        env_files: None, // No env files detected for Traefik
    })
}

// Handle multiple discovered apps - let user select which ones to add
async fn handle_multiple_apps(
    discovered_apps: Vec<crate::detection::DetectedApp>,
    force: bool,
) -> Result<()> {
    println!("✓ Discovered {} potential apps:\n", discovered_apps.len());

    // Display all discovered apps with their details
    for (idx, app) in discovered_apps.iter().enumerate() {
        let cmd_count = app.local_commands.as_ref().map(|c| c.len()).unwrap_or(0)
            + app.docker_commands.as_ref().map(|c| c.len()).unwrap_or(0)
            + app.k8s_commands.as_ref().map(|c| c.len()).unwrap_or(0);

        println!(
            "  {}. {} ({}) - {} commands",
            idx + 1,
            app.app_name,
            app.app_type,
            cmd_count
        );
        println!("     Path: {}", app.path);
    }

    // Interactive app selection
    let selected_indices = interactive_app_selection(&discovered_apps)?;

    if selected_indices.is_empty() {
        println!("No apps selected.");
        return Ok(());
    }

    // Load existing config or create a new empty one
    let mut config = crate::config::loader::load_config_or_new()?;
    let mut added_this_run: std::collections::HashSet<String> = Default::default();

    // Ask which project to add apps to (or create a new one)
    let project_name = prompt_project_selection(&config)?;

    println!(
        "\nAdding {} app(s) to project '{}'...\n",
        selected_indices.len(),
        project_name
    );

    // Process each selected app
    for &idx in &selected_indices {
        let detected = &discovered_apps[idx];

        // Skip apps with no commands
        if detected.local_commands.is_none()
            && detected.docker_commands.is_none()
            && detected.orbstack_commands.is_none()
            && detected.k8s_commands.is_none()
        {
            println!("⚠ Skipping {} - no commands detected", detected.app_name);
            continue;
        }

        // Prompt for app name (allow user to customize)
        let app_name = prompt_app_name(&detected.app_name)?;

        // Show preview
        show_preview(&project_name, &app_name, detected);

        // Confirm this specific app (default to yes)
        if !confirm_default_yes(&format!("Add {} to config?", app_name))? {
            println!("Skipped {}.\n", app_name);
            continue;
        }

        // Re-detect env files with interactive prompts
        let mut available_envs = Vec::new();
        if detected.local_commands.is_some() {
            available_envs.push("local");
        }
        if detected.docker_commands.is_some() {
            available_envs.push("docker");
            available_envs.push("orbstack");
        }
        if detected.k8s_commands.is_some() {
            available_envs.push("k8s");
        }

        let env_files = if !available_envs.is_empty() {
            // `detected.path` is stored tilde-contracted (`~/code/app`): expand it,
            // or env-file discovery looks for a literal `~` directory and finds
            // nothing.
            let expanded_path = crate::utils::path::expand_path(&detected.path);
            let app_path = expanded_path.as_path();
            if let Ok(detected_env_files) =
                crate::detection::detect_env_files(app_path, detected.dockerfile_path.as_deref())
            {
                if !detected_env_files.is_empty() {
                    match crate::detection::build_env_files_map_interactive(
                        &detected_env_files,
                        &available_envs,
                    ) {
                        Ok(map) if !map.is_empty() => Some(map),
                        _ => None,
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Build the App struct from detected data
        let app = App {
            extra: Default::default(),
            app_type: detected.app_type.clone(),
            path: detected.path.clone(),
            commands: Commands {
                local: detected.local_commands.clone(),
                docker: detected.docker_commands.clone(),
                orbstack: detected.orbstack_commands.clone(),
                k8s: detected.k8s_commands.clone(),
                ..Default::default()
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: detected.suggested_local_default.clone(),
                docker: detected.suggested_docker_default.clone(),
                orbstack: detected.suggested_orbstack_default.clone(),
                k8s: crate::app_detector_support::suggest_k8s_default(&detected.k8s_commands),
                ..Default::default()
            },
            dockerfile_path: detected.dockerfile_path.clone(),
            env_files,            // Interactive env files with user-selected environments
            default_stages: None, // Will be set by user via preferences or explicit command
            alternative_name: None,
            health_check: None,
            restart_policy: None,
        };

        if !added_this_run.insert(app_name.clone()) {
            println!("⚠ Skipped {app_name}: another selected app has the same name\n");
            continue;
        }
        match upsert_app(&mut config, &project_name, &app_name, app, force) {
            Ok(true) => println!("✓ Updated {}\n", app_name),
            Ok(false) => println!("✓ Added {}\n", app_name),
            Err(e) => println!("⚠ Skipped {app_name}: {e}\n"),
        }
    }

    // Save the updated config to disk
    save_config(&config)?;
    println!("✓ All selected apps added successfully!");

    Ok(())
}

/// Put `app` at `project/app_name`, creating the project if needed.
///
/// auto-add used to `insert` blindly: re-running it (an agent retrying, or a
/// second app with the same folder name) silently replaced the existing entry
/// and lost everything the user had added by hand. An existing app is now only
/// replaced with `force`, and then keeps its user-maintained settings.
///
/// Returns whether an existing app was replaced.
pub(super) fn upsert_app(
    config: &mut crate::config::Config,
    project: &str,
    app_name: &str,
    mut app: App,
    force: bool,
) -> Result<bool> {
    let apps = &mut config
        .projects
        .entry(project.to_string())
        .or_insert_with(|| Project {
            extra: Default::default(),
            apps: Default::default(),
            alternative_name: None,
        })
        .apps;

    let Some(existing) = apps.get(app_name) else {
        apps.insert(app_name.to_string(), app);
        return Ok(false);
    };
    if !force {
        anyhow::bail!(
            "app '{project}/{app_name}' already exists; re-run with --force to replace its \
             detected settings (dependencies, health check, restart policy and alternative \
             name are kept)"
        );
    }

    app.dependencies = existing.dependencies.clone();
    app.health_check = existing.health_check.clone();
    app.restart_policy = existing.restart_policy.clone();
    app.alternative_name = existing.alternative_name.clone();
    app.default_stages = existing.default_stages.clone();
    app.extra = existing.extra.clone();
    // `insert` on an existing key keeps the app at its position in the file.
    apps.insert(app_name.to_string(), app);
    Ok(true)
}

// Add the detected app to the config and save it to disk
// Args:
//   - config: The current config (will be modified)
//   - project_name: Which project to add the app to
//   - app_name: The name for the app
//   - detected: The detection results with all app information
fn add_to_config(
    mut config: crate::config::Config,
    project_name: String,
    app_name: String,
    detected: crate::detection::DetectedApp,
    non_interactive: bool,
    force: bool,
) -> Result<()> {
    // Re-detect env files with interactive prompts for unspecified contexts
    // Determine which environments are available for this app
    let mut available_envs = Vec::new();
    if detected.local_commands.is_some() {
        available_envs.push("local");
    }
    if detected.docker_commands.is_some() {
        available_envs.push("docker");
        available_envs.push("orbstack"); // Docker and OrbStack are equivalent
    }
    if detected.k8s_commands.is_some() {
        available_envs.push("k8s");
    }

    // Re-detect env files and build map with interactive prompts
    let env_files = if !available_envs.is_empty() {
        let expanded_path = crate::utils::path::expand_path(&detected.path);
        let app_path = std::path::Path::new(&expanded_path);
        if !non_interactive {
            println!("\n🔍 Detecting env files in: {}", app_path.display());
            println!("   Available envs: {:?}", available_envs);
        }

        if let Ok(detected_env_files) =
            crate::detection::detect_env_files(app_path, detected.dockerfile_path.as_deref())
        {
            if !non_interactive {
                println!("   Found {} env files", detected_env_files.len());
                for file in &detected_env_files {
                    println!(
                        "     - {} (stage: {:?}, context: {})",
                        file.path, file.stage, file.context
                    );
                }
            }

            if !detected_env_files.is_empty() {
                if non_interactive {
                    // Non-interactive: only map files whose context is unambiguous.
                    let map = crate::detection::build_env_files_map(&detected_env_files);
                    if map.is_empty() {
                        None
                    } else {
                        Some(map)
                    }
                } else {
                    match crate::detection::build_env_files_map_interactive(
                        &detected_env_files,
                        &available_envs,
                    ) {
                        Ok(map) if !map.is_empty() => {
                            println!("   ✓ Built env_files map with {} stages", map.len());
                            Some(map)
                        }
                        Ok(_) => {
                            println!("   ⚠ Map is empty");
                            None
                        }
                        Err(e) => {
                            println!("   ✗ Error building map: {}", e);
                            None
                        }
                    }
                }
            } else {
                if !non_interactive {
                    println!("   No env files detected");
                }
                None
            }
        } else {
            if !non_interactive {
                println!("   Error detecting env files");
            }
            None
        }
    } else {
        if !non_interactive {
            println!("\n⚠ No available environments");
        }
        None
    };

    // Build the App struct from detection results
    let app = App {
        extra: Default::default(),
        app_type: detected.app_type,
        path: detected.path,
        commands: Commands {
            local: detected.local_commands,
            docker: detected.docker_commands,
            orbstack: detected.orbstack_commands,
            k8s: detected.k8s_commands.clone(),
            ..Default::default()
        },
        dependencies: Vec::new(), // No dependencies detected automatically (user must add manually)
        defaults: Defaults {
            local: detected.suggested_local_default,
            docker: detected.suggested_docker_default,
            orbstack: detected.suggested_orbstack_default,
            k8s: crate::app_detector_support::suggest_k8s_default(&detected.k8s_commands),
            ..Default::default()
        },
        dockerfile_path: detected.dockerfile_path,
        env_files,            // Interactive env files with user-selected environments
        default_stages: None, // Will be set by user via preferences or explicit command
        alternative_name: None,
        health_check: None,
        restart_policy: None,
    };

    // Insert the app into the config (creates the project if needed)
    upsert_app(&mut config, &project_name, &app_name, app, force)?;

    // Save the updated config to ~/.devcli/config.json
    save_config(&config)?;

    Ok(())
}

#[cfg(test)]
#[path = "single_app_test.rs"]
mod single_app_test;

#[cfg(test)]
#[path = "workspace_discovery_test.rs"]
mod workspace_discovery_test;
