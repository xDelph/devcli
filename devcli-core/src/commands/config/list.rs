//! Configuration listing and display functionality
//!
//! This module handles displaying configuration information in various formats.
//! It provides the `devcli config list`, `config show`, and `config list-commands`
//! command functionality.

use crate::config::models::Environment;
use crate::config::{list_all_apps, load_config, resolve_app};
use crate::utils::path::expand_tilde;
use crate::Result;
use serde_json::json;
use std::collections::HashMap;

/// List all projects and apps
///
/// Example: `devcli config list`
/// Example: `devcli config list --project qm`
/// Example: `devcli config list --apps-only`
///
/// Two display modes:
/// 1. Full mode (default): Shows projects with app details
/// 2. Apps-only mode: Just lists app names (useful for scripts)
///
/// # Arguments
///
/// * `project_filter` - Optional project name to filter results
/// * `apps_only` - If true, only show app names without details
pub async fn config_list(project_filter: Option<String>, apps_only: bool) -> Result<()> {
    let config = load_config()?;
    ensure_project_exists(&config, project_filter.as_deref())?;

    if crate::output::json_enabled() {
        return config_list_json(&config, project_filter, apps_only);
    }

    if apps_only {
        // Apps-only mode: Just print app names, one per line
        // This is useful for shell scripts that need a list of apps
        let all_apps = list_all_apps(&config);

        for (project_name, app_name, _) in all_apps {
            // If project filter is specified, only show apps from that project
            if let Some(ref filter) = project_filter {
                if &project_name == filter {
                    println!("{}", app_name);
                }
            } else {
                // No filter - show all apps
                println!("{}", app_name);
            }
        }
    } else {
        // Full mode: Show projects with detailed app information

        // Iterate through all projects
        for (project_name, project) in &config.projects {
            // Apply project filter if specified
            if let Some(ref filter) = project_filter
                && project_name != filter
            {
                continue; // Skip this project
            }

            // Display project header (with alternative name if present)
            let project_display_name = if let Some(ref alt_name) = project.alternative_name {
                format!("{}({})", project_name, alt_name)
            } else {
                project_name.clone()
            };
            println!("PROJECT: {}", project_display_name);

            // Display each app in this project
            for (app_name, app) in &project.apps {
                // Show app name (and alternative name if present) and type
                let display_name = if let Some(ref alt_name) = app.alternative_name {
                    format!("{}({})", app_name, alt_name)
                } else {
                    app_name.clone()
                };
                println!("  [{}] ({})", display_name, app.app_type);

                // Show path
                println!("    Path: {}", app.path);

                // Show default stages if configured
                if let Some(ref default_stages) = app.default_stages {
                    println!("    Default Stages:");
                    for (env, stage) in default_stages {
                        println!("      {}: {}", env, stage);
                    }
                }

                // Show default commands (if configured)
                println!("    Defaults: {}", defaults_summary(app));

                // Show environment files for each configured environment
                // Expand the path to resolve ~ and check for env files
                let expanded_path = expand_tilde(&app.path);
                let app_path = expanded_path.as_path();

                // Only show env files if the path exists
                if app_path.exists() {
                    let mut env_files_shown = false;

                    // Check each environment that has commands configured
                    for env in crate::config::Environment::all() {
                        let env = env.as_str();
                        if app.commands.get(env).is_some() {
                            let env_file = crate::env_flow_support::resolve_runtime_env_display(
                                app_path,
                                app.env_files.as_ref(),
                                None,
                                env,
                                app.dockerfile_path.as_deref(),
                            )
                            .ok()
                            .flatten();

                            // Only print header if we have at least one env file to show
                            if !env_files_shown && env_file.is_some() {
                                println!("    Environment Files:");
                                env_files_shown = true;
                            }

                            if let Some(path) = env_file {
                                println!("      {}: {}", env, path);
                            }
                        }
                    }
                }

                // Show dependencies if any
                if !app.dependencies.is_empty() {
                    println!("    Dependencies:");
                    for dep in &app.dependencies {
                        println!("      - {}/{}", dep.project, dep.app);
                    }
                }
            }

            // Blank line between projects
            println!();
        }
    }

    Ok(())
}

/// An unknown `--project` is an error, as for every other command: it used to
/// list nothing and exit 0, as if the project had no apps.
pub(crate) fn ensure_project_exists(
    config: &crate::config::Config,
    project: Option<&str>,
) -> Result<()> {
    let Some(project) = project else {
        return Ok(());
    };
    if config.projects.contains_key(project) {
        return Ok(());
    }
    let names: Vec<String> = config.projects.keys().cloned().collect();
    let hint = config_manager::resolver::fuzzy_match(project, &names, 3)
        .map(|p| format!(" Did you mean '{p}'?"))
        .unwrap_or_default();
    anyhow::bail!("Project '{project}' not found in config.{hint}")
}

/// Machine-readable `config list` for agents (`devcli --json config list`).
fn config_list_json(
    config: &crate::config::Config,
    project_filter: Option<String>,
    apps_only: bool,
) -> Result<()> {
    if apps_only {
        let apps: Vec<String> = list_all_apps(config)
            .into_iter()
            .filter(|(project, _, _)| project_filter.as_ref().is_none_or(|f| project == f))
            .map(|(_, app, _)| app)
            .collect();
        return crate::output::print_json(&json!({ "apps": apps }));
    }

    let mut projects = Vec::new();
    for (project_name, project) in &config.projects {
        if let Some(ref filter) = project_filter
            && project_name != filter
        {
            continue;
        }

        let apps: Vec<_> = project
            .apps
            .iter()
            .map(|(app_name, app)| {
                json!({
                    "name": app_name,
                    "alternative_name": app.alternative_name,
                    "type": app.app_type,
                    "path": app.path,
                    "default_stages": app.default_stages,
                    "defaults": defaults_json(app),
                    "dependencies": app.dependencies.iter().map(|d| json!({
                        "project": d.project,
                        "app": d.app,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();

        projects.push(json!({
            "name": project_name,
            "alternative_name": project.alternative_name,
            "apps": apps,
        }));
    }

    crate::output::print_json(&json!({ "projects": projects }))
}

/// Show details of a specific app
///
/// Example: `devcli config show api-private`
/// Example: `devcli config show api --project qm`
///
/// Displays all configuration for one app:
/// - Project and type
/// - Path
/// - Default commands
/// - Dependencies
/// - All available commands (local and docker)
///
/// # Arguments
///
/// * `app_name` - Name of the app to show
/// * `project` - Optional project name to resolve ambiguous app names
pub async fn config_show(app_name: String, project: Option<String>) -> Result<()> {
    let config = load_config()?;

    // Resolve the app (handles ambiguous names with --project)
    // .as_deref() converts Option<String> to Option<&str>
    let resolved = resolve_app(&config, &app_name, project.as_deref())?;

    // Get the project config for alternative name
    let project_config = config.projects.get(&resolved.project).unwrap();

    if crate::output::json_enabled() {
        return config_show_json(&resolved, project_config);
    }

    // Display basic info (with alternative name if present)
    let display_name = if let Some(ref alt_name) = resolved.app.alternative_name {
        format!("{}({})", resolved.app_name, alt_name)
    } else {
        resolved.app_name.clone()
    };
    println!("App: {}", display_name);

    // Display project name (with alternative name if present)
    let project_display_name = if let Some(ref alt_name) = project_config.alternative_name {
        format!("{}({})", resolved.project, alt_name)
    } else {
        resolved.project.clone()
    };
    println!("Project: {}", project_display_name);
    println!("Type: {}", resolved.app.app_type);
    println!("Path: {}", resolved.app.path);

    // Display default stages if configured
    if let Some(ref default_stages) = resolved.app.default_stages {
        println!("\nDefault Stages:");
        for (env, stage) in default_stages {
            println!("  {}: {}", env, stage);
        }
    }

    // Display defaults (if configured)
    println!("\nDefaults: {}", defaults_summary(&resolved.app));

    if let Some(port) = resolved.app.port {
        println!("Port: {port}");
    }
    if let Some(check) = &resolved.app.health_check {
        println!(
            "Health check: {}",
            serde_json::to_string(check).unwrap_or_default()
        );
    }
    if let Some(policy) = &resolved.app.restart_policy {
        println!(
            "Restart policy: {}",
            serde_json::to_string(policy).unwrap_or_default()
        );
    }

    // Display dependencies if any
    if !resolved.app.dependencies.is_empty() {
        println!("\nDependencies:");
        for dep in &resolved.app.dependencies {
            println!("  - {}/{}", dep.project, dep.app);
        }
    }

    // Display the commands of every configured environment
    for env in Environment::all() {
        let Some(commands) = resolved.app.commands.get(env.as_str()) else {
            continue;
        };
        println!("\n{} Commands:", env.display_name());
        for (name, cmd) in sorted(commands) {
            println!("  {}: {}", name, cmd);
        }
    }
    if resolved.app.commands.available_envs().is_empty() {
        println!("\nCommands: (none configured)");
    }

    // Display environment files for each configured environment
    let expanded_path = expand_tilde(&resolved.app.path);
    let app_path = expanded_path.as_path();

    if app_path.exists() {
        println!("\nEnvironment Files:");
        let mut found_any = false;

        for env in crate::config::Environment::all() {
            let env = env.as_str();
            if resolved.app.commands.get(env).is_some() {
                let env_file = crate::env_flow_support::resolve_runtime_env_display(
                    app_path,
                    resolved.app.env_files.as_ref(),
                    None,
                    env,
                    resolved.app.dockerfile_path.as_deref(),
                )
                .ok()
                .flatten();

                match env_file {
                    Some(path) => {
                        println!("  {}: {}", env, path);
                        found_any = true;
                    }
                    None => {
                        println!("  {}: No environment file", env);
                    }
                }
            }
        }

        if !found_any {
            println!("  (no environment files found)");
        }
    }

    Ok(())
}

/// Machine-readable `config show` for agents (`devcli --json config show <app>`).
fn config_show_json(
    resolved: &crate::config::resolver::ResolvedApp,
    project_config: &crate::config::Project,
) -> Result<()> {
    crate::output::print_json(&json!({
        "project": resolved.project,
        "project_alternative_name": project_config.alternative_name,
        "app": resolved.app_name,
        "alternative_name": resolved.app.alternative_name,
        "type": resolved.app.app_type,
        "path": resolved.app.path,
        "default_stages": resolved.app.default_stages,
        "defaults": defaults_json(&resolved.app),
        "dependencies": resolved.app.dependencies.iter().map(|d| json!({
            "project": d.project,
            "app": d.app,
        })).collect::<Vec<_>>(),
        "commands": commands_json(&resolved.app),
        "env_files": resolved.app.env_files,
        "dockerfile_path": resolved.app.dockerfile_path,
        "port": resolved.app.port,
        "health_check": resolved.app.health_check,
        "restart_policy": resolved.app.restart_policy,
    }))
}

/// List all commands for an app
///
/// Example: `devcli config list-commands api-private`
/// Example: `devcli config list-commands api --project qm`
/// Example: `devcli config list-commands api-private --env local`
/// Example: `devcli config list-commands` (interactive mode)
///
/// Shows all available commands for an app, optionally filtered by environment
///
/// # Arguments
///
/// * `app_name` - Optional app name (prompts if not provided)
/// * `project` - Optional project name to resolve ambiguous app names
/// * `environment` - Optional environment filter (local, docker, k8s)
pub async fn config_list_commands(
    app_name: Option<String>,
    project: Option<String>,
    environment: Option<String>,
) -> Result<()> {
    let config = load_config()?;

    // Interactive prompts for missing parameters
    let (resolved_project, resolved_app_name) = if let Some(app_name) = app_name {
        let resolved = resolve_app(&config, &app_name, project.as_deref())?;
        (resolved.project, resolved.app_name)
    } else {
        crate::commands::config::prompt_for_app(&config, project.as_deref())?
    };

    // Validate (and canonicalise: `compose`, `kubernetes`) the filter
    let filter = match environment {
        Some(ref env) => Some(Environment::from_string(env.as_str()).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid environment '{}'. Must be one of: {}.",
                env,
                Environment::all_names()
            )
        })?),
        None => None,
    };

    // Get the project and app
    let project_config = config
        .projects
        .get(&resolved_project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved_project))?;

    let app = project_config.apps.get(&resolved_app_name).ok_or_else(|| {
        anyhow::anyhow!(
            "App '{}' not found in project '{}'",
            resolved_app_name,
            resolved_project
        )
    })?;

    // Display project name (with alternative name if present)
    let project_display_name = if let Some(ref alt_name) = project_config.alternative_name {
        format!("{}({})", resolved_project, alt_name)
    } else {
        resolved_project.clone()
    };

    // Display app name (with alternative name if present)
    let app_display_name = if let Some(ref alt_name) = app.alternative_name {
        format!("{}({})", resolved_app_name, alt_name)
    } else {
        resolved_app_name.clone()
    };

    let envs: Vec<Environment> = Environment::all()
        .iter()
        .copied()
        .filter(|env| filter.is_none_or(|f| f == *env))
        .collect();

    if crate::output::json_enabled() {
        let mut environments = serde_json::Map::new();
        for env in &envs {
            environments.insert(
                env.as_str().to_string(),
                json!({
                    "default": app.defaults.get(env.as_str()),
                    "commands": app.commands.get(env.as_str()),
                }),
            );
        }
        return crate::output::print_json(&json!({
            "project": resolved_project,
            "app": resolved_app_name,
            "environments": environments,
        }));
    }

    println!("Commands for {}/{}", project_display_name, app_display_name);

    // Every environment (or the one asked for): orbstack, docker-compose and ci
    // used to be skipped, so `--env orbstack` printed nothing at all.
    for env in envs {
        let default = app.defaults.get(env.as_str());
        let commands = app.commands.get(env.as_str());
        if filter.is_none() && commands.is_none() && default.is_none() {
            continue;
        }

        println!("\n{} environment:", env.display_name());
        match commands {
            Some(commands) if commands.is_empty() => println!("  (no commands defined)"),
            Some(commands) => {
                for (name, cmd) in sorted(commands) {
                    let marker = if default == Some(name) {
                        " (default)"
                    } else {
                        ""
                    };
                    println!("  {}: {}{}", name, cmd, marker);
                }
            }
            None => println!("  (not configured)"),
        }
    }

    Ok(())
}

/// `{environment: default command or null}` for every environment.
fn defaults_json(app: &crate::config::models::App) -> serde_json::Value {
    Environment::all()
        .iter()
        .map(|env| {
            (
                env.as_str().to_string(),
                json!(app.defaults.get(env.as_str())),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// `{environment: {name: command} or null}` for every environment.
fn commands_json(app: &crate::config::models::App) -> serde_json::Value {
    Environment::all()
        .iter()
        .map(|env| {
            (
                env.as_str().to_string(),
                json!(app.commands.get(env.as_str())),
            )
        })
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// `local='start', orbstack='run'` (or `none`) for the human views.
fn defaults_summary(app: &crate::config::models::App) -> String {
    let set: Vec<String> = Environment::all()
        .iter()
        .filter_map(|env| {
            app.defaults
                .get(env.as_str())
                .map(|d| format!("{}='{}'", env.as_str(), d))
        })
        .collect();
    if set.is_empty() {
        "none".to_string()
    } else {
        set.join(", ")
    }
}

/// Commands sorted by name: the map's own order is arbitrary.
fn sorted(commands: &HashMap<String, String>) -> Vec<(&String, &String)> {
    let mut entries: Vec<_> = commands.iter().collect();
    entries.sort();
    entries
}

#[cfg(test)]
#[path = "list_test.rs"]
mod tests;
