// Single app detection and addition functionality
// Handles detection of individual applications (not in Nx monorepos)
// Supports discovery of multiple apps in subdirectories

use crate::config::{load_config, save_config, App, Commands, Defaults, Project};
use crate::detection::detect_app;
use crate::Result;
use std::collections::HashMap;
use std::env;

use super::interactive::{
    prompt_project_selection, prompt_app_name, show_preview, confirm_default_yes,
    interactive_app_selection
};

// Entry point for the auto-add command
// Detects if we're in an Nx monorepo or a single app, then handles accordingly
// Args:
//   - path: Optional custom path to scan (defaults to current directory)
pub async fn auto_add_command(path: Option<String>) -> Result<()> {
    // Use provided path or default to current directory
    let target_path = if let Some(p) = path {
        std::path::PathBuf::from(p)
    } else {
        env::current_dir()?
    };
    
    // Verify the path exists before proceeding
    if !target_path.exists() {
        anyhow::bail!("Path does not exist: {}", target_path.display());
    }
    
    println!("Detecting app in {}...\n", target_path.display());
    
    // Check if we're in an Nx monorepo by looking for nx.json
    // Nx monorepos need special handling to detect multiple apps
    if target_path.join("nx.json").exists() {
        super::nx_monorepo::handle_nx_monorepo(&target_path).await
    } else {
        handle_single_app(&target_path).await
    }
}

// Handle detection and addition of a single app (not in an Nx monorepo)
// Detects the app type, prompts for confirmation, and adds to config
pub async fn handle_single_app(target_path: &std::path::Path) -> Result<()> {
    // Check if there are multiple potential apps in subdirectories
    let discovered_apps = discover_all_apps(target_path)?;
    
    if discovered_apps.len() > 1 {
        // Multiple apps found, let user choose
        return handle_multiple_apps(discovered_apps).await;
    }
    
    // Single app case - use the first (and only) discovered app
    let detected = if discovered_apps.is_empty() {
        // Fallback to direct detection if discovery found nothing
        detect_app(target_path)?
    } else {
        discovered_apps.into_iter().next().unwrap()
    };
    
    // Display what was detected
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
    
    // Apps need at least one environment to be useful
    if detected.local_commands.is_none() 
        && detected.docker_commands.is_none() 
        && detected.orbstack_commands.is_none()
        && detected.k8s_commands.is_none() {
        anyhow::bail!("No commands detected for this app. Cannot add to config.");
    }
    
    println!();
    
    // Load existing config or create empty one
    let config = load_config().unwrap_or_else(|_| crate::config::Config {
        projects: HashMap::new(),
    });
    
    // Prompt for project selection
    let project_name = prompt_project_selection(&config)?;
    
    // Prompt to confirm or edit the detected app name
    let app_name = prompt_app_name(&detected.app_name)?;
    
    // Show preview of what will be added
    show_preview(&project_name, &app_name, &detected);
    
    // Final confirmation before adding (default to yes)
    if !confirm_default_yes("Add to config?")? {
        println!("Cancelled.");
        return Ok(());
    }
    
    // Add to config and save
    add_to_config(config, project_name, app_name, detected)?;
    
    println!("✓ Added successfully!");
    
    Ok(())
}

// Discover all potential apps in a directory and its subdirectories
// Returns a list of detected apps for user selection
// Creates separate apps for each config file to allow fine-grained control
pub fn discover_all_apps(root_path: &std::path::Path) -> Result<Vec<crate::detection::DetectedApp>> {
    let mut discovered_apps = Vec::new();
    
    // Discover individual config files in the root directory
    discovered_apps.extend(discover_individual_apps(root_path)?);
    
    // Scan subdirectories for additional apps (1 level deep)
    if let Ok(entries) = std::fs::read_dir(root_path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                // Skip common non-app directories
                let dir_name = entry.file_name();
                let dir_name_str = dir_name.to_string_lossy();
                if matches!(dir_name_str.as_ref(), "node_modules" | ".git" | "target" | "dist" | "build" | ".next" | "coverage") {
                    continue;
                }
                
                // Discover individual apps in this subdirectory
                discovered_apps.extend(discover_individual_apps(&entry.path())?);
            }
        }
    }
    
    Ok(discovered_apps)
}

// Discover individual config files as separate apps in a single directory
// This allows users to select specific config files (e.g., redis.local.conf vs redis.prod.conf)
fn discover_individual_apps(dir_path: &std::path::Path) -> Result<Vec<crate::detection::DetectedApp>> {
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
                if file_name_str.starts_with("redis") && 
                   (file_name_str.ends_with(".conf") || file_name_str.ends_with(".config")) {
                    if let Ok(app) = create_redis_app_from_config(&entry.path(), &file_name_str) {
                        apps.push(app);
                    }
                }
                
                // Check for individual Traefik config files
                if file_name_str.starts_with("traefik") && 
                   (file_name_str.ends_with(".yml") || 
                    file_name_str.ends_with(".yaml") || 
                    file_name_str.ends_with(".toml")) {
                    if let Ok(app) = create_traefik_app_from_config(&entry.path(), &file_name_str) {
                        apps.push(app);
                    }
                }
            }
        }
    }
    
    Ok(apps)
}

// Create a Redis app from a specific config file
fn create_redis_app_from_config(config_path: &std::path::Path, config_filename: &str) -> Result<crate::detection::DetectedApp> {
    let dir_path = config_path.parent().unwrap();
    
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
    local_commands.insert("start".to_string(), format!("redis-server {}", config_filename));
    
    Ok(crate::detection::DetectedApp {
        app_type: "redis".to_string(),
        app_name,
        path: crate::utils::path::contract_tilde(dir_path),
        local_commands: Some(local_commands),
        docker_commands: None, // Could add Docker detection later
        orbstack_commands: None, // Could add OrbStack detection later
        k8s_commands: None,    // Could add K8s detection later
        suggested_local_default: Some("start".to_string()),
        suggested_docker_default: None,
        suggested_orbstack_default: None,
        dockerfile_path: None,
        env_files: None, // No env files detected for Redis
    })
}

// Create a Traefik app from a specific config file
fn create_traefik_app_from_config(config_path: &std::path::Path, config_filename: &str) -> Result<crate::detection::DetectedApp> {
    let dir_path = config_path.parent().unwrap();
    
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
    local_commands.insert("start".to_string(), format!("traefik --configFile={}", config_filename));
    
    Ok(crate::detection::DetectedApp {
        app_type: "traefik".to_string(),
        app_name,
        path: crate::utils::path::contract_tilde(dir_path),
        local_commands: Some(local_commands),
        docker_commands: None, // Could add Docker detection later
        orbstack_commands: None, // Could add OrbStack detection later
        k8s_commands: None,    // Could add K8s detection later
        suggested_local_default: Some("start".to_string()),
        suggested_docker_default: None,
        suggested_orbstack_default: None,
        dockerfile_path: None,
        env_files: None, // No env files detected for Traefik
    })
}

// Handle multiple discovered apps - let user select which ones to add
async fn handle_multiple_apps(discovered_apps: Vec<crate::detection::DetectedApp>) -> Result<()> {
    println!("✓ Discovered {} potential apps:\n", discovered_apps.len());
    
    // Display all discovered apps with their details
    for (idx, app) in discovered_apps.iter().enumerate() {
        let cmd_count = app.local_commands.as_ref().map(|c| c.len()).unwrap_or(0)
            + app.docker_commands.as_ref().map(|c| c.len()).unwrap_or(0)
            + app.k8s_commands.as_ref().map(|c| c.len()).unwrap_or(0);
        
        println!("  {}. {} ({}) - {} commands", 
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
    let mut config = load_config().unwrap_or_else(|_| crate::config::Config {
        projects: HashMap::new(),
    });
    
    // Ask which project to add apps to (or create a new one)
    let project_name = prompt_project_selection(&config)?;
    
    println!("\nAdding {} app(s) to project '{}'...\n", selected_indices.len(), project_name);
    
    // Process each selected app
    for &idx in &selected_indices {
        let detected = &discovered_apps[idx];
        
        // Skip apps with no commands
        if detected.local_commands.is_none() 
            && detected.docker_commands.is_none() 
            && detected.orbstack_commands.is_none()
            && detected.k8s_commands.is_none() {
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
            let app_path = std::path::Path::new(&detected.path);
            if let Ok(detected_env_files) = crate::detection::detect_env_files(
                app_path,
                detected.dockerfile_path.as_deref()
            ) {
                if !detected_env_files.is_empty() {
                    match crate::detection::build_env_files_map_interactive(&detected_env_files, &available_envs) {
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
            app_type: detected.app_type.clone(),
            path: detected.path.clone(),
            commands: Commands {
                local: detected.local_commands.clone(),
                docker: detected.docker_commands.clone(),
                orbstack: detected.orbstack_commands.clone(),
                k8s: detected.k8s_commands.clone(),
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: detected.suggested_local_default.clone(),
                docker: detected.suggested_docker_default.clone(),
                orbstack: detected.suggested_orbstack_default.clone(),
                k8s: detected.k8s_commands.as_ref().and_then(|cmds| cmds.keys().next().cloned()),
            },
            dockerfile_path: detected.dockerfile_path.clone(),
            env_files, // Interactive env files with user-selected environments
            default_stages: None, // Will be set by user via preferences or explicit command
        };
        
        // Insert the app into the config
        config
            .projects
            .entry(project_name.clone())
            .or_insert_with(|| Project {
                apps: HashMap::new(),
            })
            .apps
            .insert(app_name.clone(), app);
        
        println!("✓ Added {}\n", app_name);
    }
    
    // Save the updated config to disk
    save_config(&config)?;
    println!("✓ All selected apps added successfully!");
    
    Ok(())
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
        println!("\n🔍 DEBUG: Detecting env files in: {}", app_path.display());
        println!("   Available envs: {:?}", available_envs);
        
        if let Ok(detected_env_files) = crate::detection::detect_env_files(
            app_path,
            detected.dockerfile_path.as_deref()
        ) {
            println!("   Found {} env files", detected_env_files.len());
            for file in &detected_env_files {
                println!("     - {} (stage: {:?}, context: {})", file.path, file.stage, file.context);
            }
            
            if !detected_env_files.is_empty() {
                match crate::detection::build_env_files_map_interactive(&detected_env_files, &available_envs) {
                    Ok(map) if !map.is_empty() => {
                        println!("   ✓ Built env_files map with {} stages", map.len());
                        Some(map)
                    },
                    Ok(_) => {
                        println!("   ⚠ Map is empty");
                        None
                    },
                    Err(e) => {
                        println!("   ✗ Error building map: {}", e);
                        None
                    }
                }
            } else {
                println!("   No env files detected");
                None
            }
        } else {
            println!("   Error detecting env files");
            None
        }
    } else {
        println!("\n⚠ No available environments");
        None
    };
    
    // Build the App struct from detection results
    let app = App {
        app_type: detected.app_type,
        path: detected.path,
        commands: Commands {
            local: detected.local_commands,
            docker: detected.docker_commands,
            orbstack: detected.orbstack_commands,
            k8s: detected.k8s_commands.clone(),
        },
        dependencies: Vec::new(), // No dependencies detected automatically (user must add manually)
        defaults: Defaults {
            local: detected.suggested_local_default,
            docker: detected.suggested_docker_default,
            orbstack: detected.suggested_orbstack_default,
            k8s: detected.k8s_commands.as_ref().and_then(|cmds| cmds.keys().next().cloned()),
        },
        dockerfile_path: detected.dockerfile_path,
        env_files, // Interactive env files with user-selected environments
        default_stages: None, // Will be set by user via preferences or explicit command
    };
    
    // Insert the app into the config
    // Creates the project if it doesn't exist
    config
        .projects
        .entry(project_name)
        .or_insert_with(|| Project {
            apps: HashMap::new(),
        })
        .apps
        .insert(app_name, app);
    
    // Save the updated config to ~/.rustycli/config.json
    save_config(&config)?;
    
    Ok(())
}

#[cfg(test)]
#[path = "single_app_test.rs"]
mod single_app_test;