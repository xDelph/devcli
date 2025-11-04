// Auto-add command - Automatically detect and configure apps
// Scans directories to detect app type and configuration
// Supports both single apps and Nx monorepos

use crate::config::{load_config, save_config, App, Commands, Defaults, Project};
use crate::detection::detect_app;
use crate::Result;
use inquire::{MultiSelect, Select, Confirm, Text};
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};

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
        handle_nx_monorepo(&target_path).await
    } else {
        handle_single_app(&target_path).await
    }
}

// Handle detection and addition of apps in an Nx monorepo
// Scans apps/ and packages/ directories for Nx projects
// Allows the user to select which apps to add interactively
async fn handle_nx_monorepo(workspace_root: &std::path::Path) -> Result<()> {
    // Use the detection module to find all apps in apps/ and packages/ directories
    let detected_apps = crate::detection::detect_nx_apps(workspace_root)?;
    
    // Display all detected apps with their command counts
    println!("✓ Detected Nx monorepo with {} apps:\n", detected_apps.len());
    
    for (idx, app) in detected_apps.iter().enumerate() {
        let cmd_count = app.local_commands.as_ref().map(|c| c.len()).unwrap_or(0);
        println!("  {}. {} ({} commands)", idx + 1, app.app_name, cmd_count);
    }
    
    // Prompt user to select which apps to add
    println!();
    println!("\nSelect apps to add:");
    println!("  - Enter numbers separated by spaces (e.g., '1 3 4')");
    println!("  - Or type 'all' to add all apps");
    print!("> ");
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    
    // Parse user input into a list of indices
    // "all" = select all apps
    // "1 3 4" = select apps at indices 1, 3, and 4
    let selected_indices: Vec<usize> = if input.to_lowercase() == "all" {
        (0..detected_apps.len()).collect()
    } else {
        input
            .split_whitespace()
            .filter_map(|s| s.parse::<usize>().ok())
            .filter(|&n| n > 0 && n <= detected_apps.len())
            .map(|n| n - 1) // Convert to 0-based index
            .collect()
    };
    
    if selected_indices.is_empty() {
        anyhow::bail!("No valid apps selected");
    }
    
    // Load existing config or create a new empty one
    let mut config = load_config().unwrap_or_else(|_| crate::config::Config {
        projects: HashMap::new(),
    });
    
    // Ask which project to add apps to (or create a new one)
    let project_name = prompt_project_selection(&config)?;
    
    println!("\nAdding {} app(s) to project '{}'...\n", selected_indices.len(), project_name);
    
    // Iterate through selected apps and add each one to the config
    for &idx in &selected_indices {
        let detected = &detected_apps[idx];
        
        // Skip apps with no commands - they can't be run
        if detected.local_commands.is_none() 
            && detected.docker_commands.is_none() 
            && detected.k8s_commands.is_none() {
            println!("⚠ Skipping {} - no commands detected", detected.app_name);
            continue;
        }
        
        // Show a preview of what will be added
        show_preview(&project_name, &detected.app_name, detected);
        
        // Build the App struct from detected data
        let app = App {
            app_type: detected.app_type.clone(),
            path: detected.path.clone(),
            commands: Commands {
                local: detected.local_commands.clone(),
                docker: detected.docker_commands.clone(),
                k8s: detected.k8s_commands.clone(),
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: detected.suggested_local_default.clone(),
                docker: detected.suggested_docker_default.clone(),
                k8s: detected.k8s_commands.as_ref().and_then(|cmds| cmds.keys().next().cloned()),
            },
        };
        
        // Insert the app into the config
        // This creates the project if it doesn't exist, then adds the app
        config
            .projects
            .entry(project_name.clone())
            .or_insert_with(|| Project {
                apps: HashMap::new(),
            })
            .apps
            .insert(detected.app_name.clone(), app);
        
        println!("✓ Added {}\n", detected.app_name);
    }
    
    // Save the updated config to disk
    save_config(&config)?;
    println!("✓ All apps added successfully!");
    
    Ok(())
}

// Handle detection and addition of a single app (not in an Nx monorepo)
// Detects the app type, prompts for confirmation, and adds to config
async fn handle_single_app(target_path: &std::path::Path) -> Result<()> {
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
    if let Some(k8s_cmds) = &detected.k8s_commands {
        println!("  - Kubernetes: {} commands available", k8s_cmds.len());
    }
    
    // Apps need at least one environment to be useful
    if detected.local_commands.is_none() 
        && detected.docker_commands.is_none() 
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
        k8s_commands: None,    // Could add K8s detection later
        suggested_local_default: Some("start".to_string()),
        suggested_docker_default: None,
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
        k8s_commands: None,    // Could add K8s detection later
        suggested_local_default: Some("start".to_string()),
        suggested_docker_default: None,
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
        
        // Build the App struct from detected data
        let app = App {
            app_type: detected.app_type.clone(),
            path: detected.path.clone(),
            commands: Commands {
                local: detected.local_commands.clone(),
                docker: detected.docker_commands.clone(),
                k8s: detected.k8s_commands.clone(),
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: detected.suggested_local_default.clone(),
                docker: detected.suggested_docker_default.clone(),
                k8s: detected.k8s_commands.as_ref().and_then(|cmds| cmds.keys().next().cloned()),
            },
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

// Interactive prompt for selecting an existing project or creating a new one
// Uses inquire for proper arrow key navigation
fn prompt_project_selection(config: &crate::config::Config) -> Result<String> {
    // Get all project names and sort them alphabetically, but put "global" first
    let mut projects: Vec<String> = config.projects.keys().cloned().collect();
    projects.sort();
    
    // Move "global" to the front if it exists, or add it as option 1
    let mut display_projects = Vec::new();
    let has_global = projects.contains(&"global".to_string());
    
    if has_global {
        display_projects.push("global (default)".to_string());
        // Add other projects except global
        for project in projects {
            if project != "global" {
                display_projects.push(project);
            }
        }
    } else {
        // Add "global" as first option even if it doesn't exist yet
        display_projects.push("global (default)".to_string());
        display_projects.extend(projects);
    }
    
    // Add "Create new project" option
    display_projects.push("Create new project".to_string());

    println!();

    // Use inquire's Select for arrow key navigation
    let selection = Select::new("Which project?", display_projects.clone())
        .with_help_message("Use ↑/↓ to navigate, Enter to select")
        .prompt()?;
    
    // Handle the selection
    if selection == "Create new project" {
        // Prompt for new project name
        let project_name = Text::new("New project name:")
            .with_validator(|input: &str| {
                if input.trim().is_empty() {
                    Ok(inquire::validator::Validation::Invalid("Project name cannot be empty".into()))
                } else {
                    Ok(inquire::validator::Validation::Valid)
                }
            })
            .prompt()?;
        
        Ok(project_name.trim().to_string())
    } else if selection.starts_with("global") {
        Ok("global".to_string())
    } else {
        Ok(selection)
    }
}

// Prompt to confirm or edit the detected app name using inquire
fn prompt_app_name(suggested_name: &str) -> Result<String> {
    // Validate the suggested name first
    if suggested_name.is_empty() {
        return prompt_custom_app_name();
    }
    
    // Use inquire's Confirm with default to true
    let use_suggested = Confirm::new(&format!("Use app name '{}'?", suggested_name))
        .with_default(true)
        .with_help_message("Press Enter to accept, or 'n' to enter a custom name")
        .prompt()?;
    
    if use_suggested {
        Ok(suggested_name.to_string())
    } else {
        prompt_custom_app_name()
    }
}

// Helper function to prompt for a custom app name with validation using inquire
fn prompt_custom_app_name() -> Result<String> {
    let app_name = Text::new("Enter app name:")
        .with_validator(|input: &str| {
            match validate_app_name(input) {
                Ok(_) => Ok(inquire::validator::Validation::Valid),
                Err(e) => Ok(inquire::validator::Validation::Invalid(e.to_string().into())),
            }
        })
        .prompt()?;
    
    Ok(app_name)
}

// Validate app name according to rules
// Returns Ok(()) if valid, Err with message if invalid
pub fn validate_app_name(name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("App name cannot be empty. Please try again.");
    }
    
    if name.contains(' ') {
        anyhow::bail!("App name cannot contain spaces. Use dashes or underscores instead.");
    }
    
    // Additional validation: check for other problematic characters
    if name.contains('/') || name.contains('\\') {
        anyhow::bail!("App name cannot contain path separators.");
    }
    
    Ok(())
}

// Display a preview of what will be added to the config
// Shows the project/app path, type, commands, and defaults
fn show_preview(project: &str, app_name: &str, detected: &crate::detection::DetectedApp) {
    println!("\nPreview:");
    println!("[{}/{}]", project, app_name);
    println!("  type: {}", detected.app_type);
    println!("  path: {}", detected.path);
    
    // Show local commands and default if available
    if let Some(local_cmds) = &detected.local_commands {
        println!("  local commands: {}", local_cmds.keys().cloned().collect::<Vec<_>>().join(", "));
        if let Some(default) = &detected.suggested_local_default {
            println!("    default: {}", default);
        }
    }
    
    // Show docker commands and default if available
    if let Some(docker_cmds) = &detected.docker_commands {
        println!("  docker commands: {}", docker_cmds.keys().cloned().collect::<Vec<_>>().join(", "));
        if let Some(default) = &detected.suggested_docker_default {
            println!("    default: {}", default);
        }
    }
    
    // Show kubernetes commands if available
    if let Some(k8s_cmds) = &detected.k8s_commands {
        println!("  k8s commands: {}", k8s_cmds.keys().cloned().collect::<Vec<_>>().join(", "));
    }
    
    println!();
}

// Yes/no confirmation prompt with default to yes using inquire
fn confirm_default_yes(prompt: &str) -> Result<bool> {
    let confirmed = Confirm::new(prompt)
        .with_default(true)
        .with_help_message("Press Enter to confirm, or 'n' to skip")
        .prompt()?;
    
    Ok(confirmed)
}

// Interactive app selection using inquire for proper UI with arrow keys
fn interactive_app_selection(apps: &[crate::detection::DetectedApp]) -> Result<Vec<usize>> {
    // Create display options for each app
    let options: Vec<String> = apps
        .iter()
        .map(|app| {
            let cmd_count = app.local_commands.as_ref().map(|c| c.len()).unwrap_or(0)
                + app.docker_commands.as_ref().map(|c| c.len()).unwrap_or(0)
                + app.k8s_commands.as_ref().map(|c| c.len()).unwrap_or(0);
            
            format!("{} ({}) - {} commands | {}", 
                app.app_name, 
                app.app_type, 
                cmd_count,
                app.path
            )
        })
        .collect();

    println!();
    
    // Use inquire's MultiSelect for checkbox-style selection with arrow keys
    let selected = MultiSelect::new("Select apps to add:", options.clone())
        .with_help_message("Use ↑/↓ to navigate, Space to select/deselect, Enter to confirm")
        .prompt();
    
    match selected {
        Ok(selections) => {
            // Convert selected display strings back to indices
            let mut indices = Vec::new();
            for selection in selections {
                if let Some(index) = options.iter().position(|opt| opt == &selection) {
                    indices.push(index);
                }
            }
            Ok(indices)
        }
        Err(_) => {
            // User cancelled (Ctrl+C or ESC)
            Ok(vec![])
        }
    }
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
    // Build the App struct from detection results
    let app = App {
        app_type: detected.app_type,
        path: detected.path,
        commands: Commands {
            local: detected.local_commands,
            docker: detected.docker_commands,
            k8s: detected.k8s_commands.clone(),
        },
        dependencies: Vec::new(), // No dependencies detected automatically (user must add manually)
        defaults: Defaults {
            local: detected.suggested_local_default,
            docker: detected.suggested_docker_default,
            k8s: detected.k8s_commands.as_ref().and_then(|cmds| cmds.keys().next().cloned()),
        },
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

