// Auto-add command - Automatically detect and configure apps
// Scans directories to detect app type and configuration
// Supports both single apps and Nx monorepos

use crate::config::{load_config, save_config, App, Commands, Defaults, Project};
use crate::detection::detect_app;
use crate::Result;
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
    println!("Select apps to add:");
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
    // Run detection on the target directory
    let detected = detect_app(target_path)?;
    
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
    
    // Final confirmation before adding
    if !confirm("Add to config?")? {
        println!("Cancelled.");
        return Ok(());
    }
    
    // Add to config and save
    add_to_config(config, project_name, app_name, detected)?;
    
    println!("✓ Added successfully!");
    
    Ok(())
}

// Interactive prompt for selecting an existing project or creating a new one
// Lists all existing projects from config and adds "Create new" option
fn prompt_project_selection(config: &crate::config::Config) -> Result<String> {
    println!("Which project?");
    
    // Get all project names and sort them alphabetically
    let mut projects: Vec<String> = config.projects.keys().cloned().collect();
    projects.sort();
    
    // Display numbered list of existing projects
    for (i, project) in projects.iter().enumerate() {
        println!("  {}) {}", i + 1, project);
    }
    println!("  {}) Create new project", projects.len() + 1);
    
    // Get user input
    print!("> ");
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();
    
    // Parse the choice
    if let Ok(choice) = input.parse::<usize>() {
        // Check if it's an existing project
        if choice > 0 && choice <= projects.len() {
            return Ok(projects[choice - 1].clone());
        } 
        // Check if it's "Create new project"
        else if choice == projects.len() + 1 {
            print!("New project name: ");
            io::stdout().flush()?;
            let mut project_name = String::new();
            io::stdin().read_line(&mut project_name)?;
            return Ok(project_name.trim().to_string());
        }
    }
    
    anyhow::bail!("Invalid selection")
}

// Prompt to confirm or edit the detected app name
// Args:
//   - suggested_name: The name detected from package.json or directory
// Returns: The confirmed or user-provided app name
fn prompt_app_name(suggested_name: &str) -> Result<String> {
    // Show suggested name and ask for confirmation
    print!("App name: {} [y/n] ", suggested_name);
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();
    
    // If user confirms, use the suggested name
    if input == "y" || input == "yes" || input.is_empty() {
        return Ok(suggested_name.to_string());
    }
    
    // Otherwise, let the user type a custom name
    print!("Enter app name: ");
    io::stdout().flush()?;
    
    let mut app_name = String::new();
    io::stdin().read_line(&mut app_name)?;
    Ok(app_name.trim().to_string())
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

// Simple yes/no confirmation prompt
// Returns true if user answers yes, false if no
fn confirm(prompt: &str) -> Result<bool> {
    print!("{} [y/n] ", prompt);
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();
    
    // Accept "y" or "yes" as confirmation
    Ok(input == "y" || input == "yes")
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

