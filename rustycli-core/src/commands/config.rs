// Config management commands
// Handles initializing, validating, listing, and editing the configuration file
//
// Commands:
// - rustycli config init      - Create template config
// - rustycli config validate  - Check config is valid
// - rustycli config list      - List all apps
// - rustycli config show      - Show details of one app
// - rustycli config edit      - Open config in editor

use crate::config::{
    dependencies::resolve_dependency_chain, list_all_apps, load_config, resolve_app, save_config,
};
use crate::Result;
use inquire::{Select, Text};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::process::Command;

// Initialize a new config file with a template
// Example: rustycli config init
//
// Creates ~/.rustycli/config.json with an example app configuration
// The user can then edit this file to add their own projects and apps
pub async fn config_init() -> Result<()> {
    // Get the home directory from environment
    let home = env::var("HOME")?;
    
    // Build paths: ~/.rustycli/ and ~/.rustycli/config.json
    let config_dir = std::path::PathBuf::from(home).join(".rustycli");
    let config_path = config_dir.join("config.json");
    
    // Check if config already exists
    // Don't overwrite existing configs - user might lose their data!
    if config_path.exists() {
        anyhow::bail!(
            "Config file already exists at {}. Delete it first if you want to recreate it.",
            config_path.display()
        );
    }
    
    // Create the ~/.rustycli directory if it doesn't exist
    // create_dir_all is like "mkdir -p" - creates parent directories too
    fs::create_dir_all(&config_dir)?;
    
    // Template config with one example app
    // r#"..."# is a raw string literal - backslashes and quotes don't need escaping
    // This makes it easier to embed JSON
    let template = r#"{
  "projects": {
    "example": {
      "apps": {
        "my-app": {
          "type": "nodejs",
          "path": "~/Projects/my-app",
          "commands": {
            "local": {
              "start": "npm start",
              "test": "npm test",
              "build": "npm run build"
            },
            "docker": {
              "build": "docker build -t my-app .",
              "run": "docker run --name my-app --rm my-app"
            }
          },
          "dependencies": [],
          "defaults": {
            "local": "start",
            "docker": "run"
          }
        }
      }
    }
  }
}
"#;
    
    // Write the template to the config file
    fs::write(&config_path, template)?;
    
    // Show success message with next steps
    println!("✓ Created config file at {}", config_path.display());
    println!("\nEdit this file to add your projects and apps.");
    println!("Then run 'rustycli config validate' to check your configuration.");
    
    Ok(())
}

// Validate the config file
// Example: rustycli config validate
//
// Performs comprehensive validation:
// - Are all paths valid?
// - Do default commands exist?
// - Are dependencies valid?
// - Any circular dependencies?
// - Any duplicate app names across projects?
pub async fn config_validate() -> Result<()> {
    // Load the config (this already validates JSON syntax)
    let config = load_config()?;
    
    println!("Validating configuration...\n");
    
    // Collections to store validation issues
    // Vec = resizable array for collecting errors/warnings
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    
    // Get a flat list of all apps across all projects
    let all_apps = list_all_apps(&config);
    
    // HashSet to track app names and detect duplicates
    // HashSet = no duplicates, O(1) lookup
    let mut app_names: HashSet<String> = HashSet::new();
    
    // Validation pass 1: Check each app individually
    for (project_name, app_name, app) in &all_apps {
        // Check for duplicate app names across projects
        // This isn't an error (you CAN have same name in different projects)
        // But it requires using --project flag, so we warn about it
        if app_names.contains(app_name) {
            warnings.push(format!(
                "App name '{}' in project '{}' is not unique - you'll need to use --project flag",
                app_name, project_name
            ));
        }
        // Add to the set for future duplicate checks
        app_names.insert(app_name.clone());
        
        // Check if the app's path exists
        // Expand ~ and env vars first, then check
        let path = crate::utils::path::expand_path(&app.path);
        if !path.exists() {
            errors.push(format!(
                "[{}/{}] Path does not exist: {}",
                project_name,
                app_name,
                path.display()
            ));
        }
        
        // Check if app has any commands defined
        // An app without commands is useless!
        let has_local = app.commands.local.as_ref().map(|m| !m.is_empty()).unwrap_or(false);
        let has_docker = app.commands.docker.as_ref().map(|m| !m.is_empty()).unwrap_or(false);
        
        if !has_local && !has_docker {
            errors.push(format!(
                "[{}/{}] No commands defined (need at least one environment)",
                project_name, app_name
            ));
        }
        
        // Validate local environment (if configured)
        if let Some(local_commands) = &app.commands.local {
            // If we have local commands, we should have a default
            if let Some(default_local) = &app.defaults.local {
                // Check the default references an actual command
                if !local_commands.contains_key(default_local) {
                    errors.push(format!(
                        "[{}/{}] Default local command '{}' not found in local commands",
                        project_name, app_name, default_local
                    ));
                }
            } else {
                // Local commands exist but no default specified
                errors.push(format!(
                    "[{}/{}] Local commands defined but no defaults.local specified",
                    project_name, app_name
                ));
            }
        }
        
        // Validate docker environment (if configured)
        if let Some(docker_commands) = &app.commands.docker {
            // If we have docker commands, we should have a default
            if let Some(default_docker) = &app.defaults.docker {
                // Check the default references an actual command
                if !docker_commands.contains_key(default_docker) {
                    errors.push(format!(
                        "[{}/{}] Default docker command '{}' not found in docker commands",
                        project_name, app_name, default_docker
                    ));
                }
            } else {
                // Docker commands exist but no default specified
                errors.push(format!(
                    "[{}/{}] Docker commands defined but no defaults.docker specified",
                    project_name, app_name
                ));
            }
        }
        
        // Validate each dependency exists
        // Try to get each dependency from the config
        for dep in &app.dependencies {
            // get_app_by_project returns Result - Err if not found
            if let Err(e) = crate::config::resolver::get_app_by_project(&config, &dep.project, &dep.app) {
                errors.push(format!(
                    "[{}/{}] Invalid dependency {}/{}: {}",
                    project_name, app_name, dep.project, dep.app, e
                ));
            }
        }
    }
    
    // Validation pass 2: Check dependency chains
    // This detects circular dependencies (A→B→A)
    for (project_name, app_name, _app) in &all_apps {
        // Try to resolve the full dependency chain
        if let Ok(resolved) = resolve_app(&config, app_name, Some(project_name)) {
            // resolve_dependency_chain will error if there's a cycle
            if let Err(e) = resolve_dependency_chain(&config, &resolved) {
                errors.push(format!(
                    "[{}/{}] Dependency resolution error: {}",
                    project_name, app_name, e
                ));
            }
        }
    }
    
    // Display validation results
    
    // Show warnings (non-fatal issues)
    if !warnings.is_empty() {
        println!("⚠ Warnings:");
        for warning in &warnings {
            println!("  - {}", warning);
        }
        println!();
    }
    
    // Show errors (fatal issues that prevent usage)
    if !errors.is_empty() {
        println!("✗ Errors:");
        for error in &errors {
            println!("  - {}", error);
        }
        println!("\nValidation failed with {} error(s)", errors.len());
        
        // Return error to indicate validation failed
        anyhow::bail!("Config validation failed");
    }
    
    // Success! No errors found
    println!("✓ Configuration is valid!");
    println!("  - {} projects", config.projects.len());
    println!("  - {} apps", all_apps.len());
    
    Ok(())
}

// List all projects and apps
// Example: rustycli config list
// Example: rustycli config list --project qm
// Example: rustycli config list --apps-only
//
// Two display modes:
// 1. Full mode (default): Shows projects with app details
// 2. Apps-only mode: Just lists app names (useful for scripts)
pub async fn config_list(project_filter: Option<String>, apps_only: bool) -> Result<()> {
    let config = load_config()?;
    
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
            if let Some(ref filter) = project_filter {
                if project_name != filter {
                    continue; // Skip this project
                }
            }
            
            // Display project header
            println!("PROJECT: {}", project_name);
            
            // Display each app in this project
            for (app_name, app) in &project.apps {
                // Show app name and type
                println!("  [{}] ({})", app_name, app.app_type);
                
                // Show path
                println!("    Path: {}", app.path);
                
                // Show default commands (if configured)
                let local_default = app.defaults.local.as_deref().unwrap_or("none");
                let docker_default = app.defaults.docker.as_deref().unwrap_or("none");
                let k8s_default = app.defaults.k8s.as_deref().unwrap_or("none");
                println!(
                    "    Defaults: local='{}', docker='{}', k8s='{}'",
                    local_default, docker_default, k8s_default
                );
                
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

// Show details of a specific app
// Example: rustycli config show api-private
// Example: rustycli config show api --project qm
//
// Displays all configuration for one app:
// - Project and type
// - Path
// - Default commands
// - Dependencies
// - All available commands (local and docker)
pub async fn config_show(app_name: String, project: Option<String>) -> Result<()> {
    let config = load_config()?;
    
    // Resolve the app (handles ambiguous names with --project)
    // .as_deref() converts Option<String> to Option<&str>
    let resolved = resolve_app(&config, &app_name, project.as_deref())?;
    
    // Display basic info
    println!("App: {}", resolved.app_name);
    println!("Project: {}", resolved.project);
    println!("Type: {}", resolved.app.app_type);
    println!("Path: {}", resolved.app.path);
    
    // Display defaults (if configured)
    println!("\nDefaults:");
    if let Some(default_local) = &resolved.app.defaults.local {
        println!("  Local: {}", default_local);
    } else {
        println!("  Local: none");
    }
    if let Some(default_docker) = &resolved.app.defaults.docker {
        println!("  Docker: {}", default_docker);
    } else {
        println!("  Docker: none");
    }
    
    // Display dependencies if any
    if !resolved.app.dependencies.is_empty() {
        println!("\nDependencies:");
        for dep in &resolved.app.dependencies {
            println!("  - {}/{}", dep.project, dep.app);
        }
    }
    
    // Display all local commands (if configured)
    println!("\nLocal Commands:");
    if let Some(local_commands) = &resolved.app.commands.local {
        for (name, cmd) in local_commands {
            println!("  {}: {}", name, cmd);
        }
    } else {
        println!("  (none configured)");
    }
    
    // Display all docker commands (if configured)
    println!("\nDocker Commands:");
    if let Some(docker_commands) = &resolved.app.commands.docker {
        for (name, cmd) in docker_commands {
            println!("  {}: {}", name, cmd);
        }
    } else {
        println!("  (none configured)");
    }
    
    Ok(())
}

// Edit the config file in the user's editor
// Example: rustycli config edit
//
// Opens the config file in $EDITOR (or vim if not set)
// After editing, suggests running validate to check changes
pub async fn config_edit() -> Result<()> {
    // Build path to config file
    let home = env::var("HOME")?;
    let config_path = std::path::PathBuf::from(home)
        .join(".rustycli")
        .join("config.json");
    
    // Make sure config exists
    if !config_path.exists() {
        anyhow::bail!(
            "Config file not found. Run 'rustycli config init' to create one."
        );
    }
    
    // Get editor from environment variable
    // .unwrap_or_else() provides a default if EDITOR is not set
    // The closure (|_| ...) is only called if env::var fails
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    
    // Launch the editor as a subprocess
    // Command::new() creates a new command to run
    // .arg() adds an argument (the file path)
    // .status() runs the command and waits for it to finish
    let status = Command::new(&editor)
        .arg(&config_path)
        .status()?;
    
    // Check if the editor exited successfully
    // .success() returns true if exit code was 0
    if !status.success() {
        anyhow::bail!("Editor exited with error");
    }
    
    // Remind user to validate their changes
    println!("Config file updated. Run 'rustycli config validate' to check your changes.");
    
    Ok(())
}

// Add a command to an app
// Example: rustycli config add-command api-private local test "npm test"
// Example: rustycli config add-command api --project qm docker build "docker build -t api ."
// Example: rustycli config add-command (interactive mode)
//
// Adds a new command to the specified environment for an app
pub async fn config_add_command(
    app_name: Option<String>,
    project: Option<String>,
    environment: Option<String>,
    command_name: Option<String>,
    command_value: Option<String>,
) -> Result<()> {
    let mut config = load_config()?;
    
    // Interactive prompts for missing parameters
    let (resolved_project, resolved_app_name) = if let Some(app_name) = app_name {
        // App name provided, resolve it
        let resolved = resolve_app(&config, &app_name, project.as_deref())?;
        (resolved.project, resolved.app_name)
    } else {
        // No app name provided, prompt user to select
        prompt_for_app(&config, project.as_deref())?
    };
    
    let environment = if let Some(env) = environment {
        // Validate provided environment
        if !["local", "docker", "k8s"].contains(&env.as_str()) {
            anyhow::bail!("Invalid environment '{}'. Must be one of: local, docker, k8s", env);
        }
        env
    } else {
        // Prompt for environment
        prompt_for_environment()?
    };
    
    let command_name = if let Some(name) = command_name {
        name
    } else {
        prompt_for_text("Enter command name:", None)?
    };
    
    let command_value = if let Some(value) = command_value {
        value
    } else {
        prompt_for_text("Enter command value:", None)?
    };
    
    // Get mutable reference to the app
    let app = config
        .projects
        .get_mut(&resolved_project)
        .unwrap()
        .apps
        .get_mut(&resolved_app_name)
        .unwrap();
    
    // Add command to the appropriate environment
    match environment.as_str() {
        "local" => {
            if app.commands.local.is_none() {
                app.commands.local = Some(HashMap::new());
            }
            app.commands.local.as_mut().unwrap().insert(command_name.clone(), command_value.clone());
        }
        "docker" => {
            if app.commands.docker.is_none() {
                app.commands.docker = Some(HashMap::new());
            }
            app.commands.docker.as_mut().unwrap().insert(command_name.clone(), command_value.clone());
        }
        "k8s" => {
            if app.commands.k8s.is_none() {
                app.commands.k8s = Some(HashMap::new());
            }
            app.commands.k8s.as_mut().unwrap().insert(command_name.clone(), command_value.clone());
        }
        _ => unreachable!(),
    }
    
    // Save the updated config
    save_config(&config)?;
    
    println!("✓ Added command '{}' to {}/{} ({} environment)", 
             command_name, resolved_project, resolved_app_name, environment);
    println!("  Command: {}", command_value);
    
    Ok(())
}

// Remove a command from an app
// Example: rustycli config remove-command api-private local test
// Example: rustycli config remove-command api --project qm docker build
// Example: rustycli config remove-command (interactive mode)
//
// Removes a command from the specified environment for an app
pub async fn config_remove_command(
    app_name: Option<String>,
    project: Option<String>,
    environment: Option<String>,
    command_name: Option<String>,
) -> Result<()> {
    let mut config = load_config()?;
    
    // Interactive prompts for missing parameters
    let (resolved_project, resolved_app_name) = if let Some(app_name) = app_name {
        let resolved = resolve_app(&config, &app_name, project.as_deref())?;
        (resolved.project, resolved.app_name)
    } else {
        prompt_for_app(&config, project.as_deref())?
    };
    
    let environment = if let Some(env) = environment {
        if !["local", "docker", "k8s"].contains(&env.as_str()) {
            anyhow::bail!("Invalid environment '{}'. Must be one of: local, docker, k8s", env);
        }
        env
    } else {
        prompt_for_environment()?
    };
    
    // Get the app to check available commands
    let app = config
        .projects
        .get(&resolved_project)
        .unwrap()
        .apps
        .get(&resolved_app_name)
        .unwrap();
    
    let command_name = if let Some(name) = command_name {
        name
    } else {
        prompt_for_command(app, &environment)?
    };
    
    // Get mutable reference to the app
    let app = config
        .projects
        .get_mut(&resolved_project)
        .unwrap()
        .apps
        .get_mut(&resolved_app_name)
        .unwrap();
    
    // Remove command from the appropriate environment
    let removed = match environment.as_str() {
        "local" => {
            if let Some(commands) = &mut app.commands.local {
                commands.remove(&command_name).is_some()
            } else {
                false
            }
        }
        "docker" => {
            if let Some(commands) = &mut app.commands.docker {
                commands.remove(&command_name).is_some()
            } else {
                false
            }
        }
        "k8s" => {
            if let Some(commands) = &mut app.commands.k8s {
                commands.remove(&command_name).is_some()
            } else {
                false
            }
        }
        _ => unreachable!(),
    };
    
    if !removed {
        anyhow::bail!("Command '{}' not found in {} environment for {}/{}", 
                     command_name, environment, resolved_project, resolved_app_name);
    }
    
    // Check if this was the default command and warn user
    let was_default = match environment.as_str() {
        "local" => app.defaults.local.as_ref() == Some(&command_name),
        "docker" => app.defaults.docker.as_ref() == Some(&command_name),
        "k8s" => app.defaults.k8s.as_ref() == Some(&command_name),
        _ => false,
    };
    
    if was_default {
        // Clear the default since the command no longer exists
        match environment.as_str() {
            "local" => app.defaults.local = None,
            "docker" => app.defaults.docker = None,
            "k8s" => app.defaults.k8s = None,
            _ => unreachable!(),
        }
        println!("⚠ Warning: '{}' was the default {} command. Default cleared.", 
                command_name, environment);
    }
    
    // Save the updated config
    save_config(&config)?;
    
    println!("✓ Removed command '{}' from {}/{} ({} environment)", 
             command_name, resolved_project, resolved_app_name, environment);
    
    Ok(())
}

// Set the default command for an environment
// Example: rustycli config set-default api-private local start
// Example: rustycli config set-default api --project qm docker run
// Example: rustycli config set-default (interactive mode)
//
// Sets which command should be used by default when starting an app in the specified environment
pub async fn config_set_default(
    app_name: Option<String>,
    project: Option<String>,
    environment: Option<String>,
    command_name: Option<String>,
) -> Result<()> {
    let mut config = load_config()?;
    
    // Interactive prompts for missing parameters
    let (resolved_project, resolved_app_name) = if let Some(app_name) = app_name {
        let resolved = resolve_app(&config, &app_name, project.as_deref())?;
        (resolved.project, resolved.app_name)
    } else {
        prompt_for_app(&config, project.as_deref())?
    };
    
    let environment = if let Some(env) = environment {
        if !["local", "docker", "k8s"].contains(&env.as_str()) {
            anyhow::bail!("Invalid environment '{}'. Must be one of: local, docker, k8s", env);
        }
        env
    } else {
        prompt_for_environment()?
    };
    
    // Get the app to check available commands
    let app = config
        .projects
        .get(&resolved_project)
        .unwrap()
        .apps
        .get(&resolved_app_name)
        .unwrap();
    
    let command_name = if let Some(name) = command_name {
        name
    } else {
        prompt_for_command(app, &environment)?
    };
    
    // Get mutable reference to the app
    let app = config
        .projects
        .get_mut(&resolved_project)
        .unwrap()
        .apps
        .get_mut(&resolved_app_name)
        .unwrap();
    
    // Verify the command exists in the specified environment
    let command_exists = match environment.as_str() {
        "local" => {
            app.commands.local.as_ref()
                .map(|commands| commands.contains_key(&command_name))
                .unwrap_or(false)
        }
        "docker" => {
            app.commands.docker.as_ref()
                .map(|commands| commands.contains_key(&command_name))
                .unwrap_or(false)
        }
        "k8s" => {
            app.commands.k8s.as_ref()
                .map(|commands| commands.contains_key(&command_name))
                .unwrap_or(false)
        }
        _ => unreachable!(),
    };
    
    if !command_exists {
        anyhow::bail!("Command '{}' not found in {} environment for {}/{}. Add it first with 'config add-command'.", 
                     command_name, environment, resolved_project, resolved_app_name);
    }
    
    // Set the default command
    match environment.as_str() {
        "local" => app.defaults.local = Some(command_name.clone()),
        "docker" => app.defaults.docker = Some(command_name.clone()),
        "k8s" => app.defaults.k8s = Some(command_name.clone()),
        _ => unreachable!(),
    }
    
    // Save the updated config
    save_config(&config)?;
    
    println!("✓ Set '{}' as default {} command for {}/{}", 
             command_name, environment, resolved_project, resolved_app_name);
    
    Ok(())
}

// List all commands for an app
// Example: rustycli config list-commands api-private
// Example: rustycli config list-commands api --project qm
// Example: rustycli config list-commands api-private --env local
// Example: rustycli config list-commands (interactive mode)
//
// Shows all available commands for an app, optionally filtered by environment
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
        prompt_for_app(&config, project.as_deref())?
    };
    
    // Validate environment filter if provided
    if let Some(ref env) = environment {
        if !["local", "docker", "k8s"].contains(&env.as_str()) {
            anyhow::bail!("Invalid environment '{}'. Must be one of: local, docker, k8s", env);
        }
    }
    
    // Get the app
    let app = config
        .projects
        .get(&resolved_project)
        .unwrap()
        .apps
        .get(&resolved_app_name)
        .unwrap();
    
    println!("Commands for {}/{}", resolved_project, resolved_app_name);
    
    // Helper function to display commands for an environment
    let display_env_commands = |env_name: &str, commands: &Option<HashMap<String, String>>, default: &Option<String>| {
        if let Some(ref filter) = environment {
            if filter != env_name {
                return; // Skip this environment if filtering
            }
        }
        
        println!("\n{} environment:", env_name.to_uppercase());
        
        if let Some(commands) = commands {
            if commands.is_empty() {
                println!("  (no commands defined)");
            } else {
                for (name, cmd) in commands {
                    let is_default = default.as_ref() == Some(name);
                    let marker = if is_default { " (default)" } else { "" };
                    println!("  {}: {}{}", name, cmd, marker);
                }
            }
        } else {
            println!("  (not configured)");
        }
    };
    
    // Display commands for each environment
    display_env_commands("local", &app.commands.local, &app.defaults.local);
    display_env_commands("docker", &app.commands.docker, &app.defaults.docker);
    display_env_commands("k8s", &app.commands.k8s, &app.defaults.k8s);
    
    Ok(())
}

// Edit a specific command for an app
// Example: rustycli config edit-command api-private local start
// Example: rustycli config edit-command api --project qm docker build
// Example: rustycli config edit-command (interactive mode)
//
// Allows interactive editing of a command's value
pub async fn config_edit_command(
    app_name: Option<String>,
    project: Option<String>,
    environment: Option<String>,
    command_name: Option<String>,
) -> Result<()> {
    let mut config = load_config()?;
    
    // Interactive prompts for missing parameters
    let (resolved_project, resolved_app_name) = if let Some(app_name) = app_name {
        let resolved = resolve_app(&config, &app_name, project.as_deref())?;
        (resolved.project, resolved.app_name)
    } else {
        prompt_for_app(&config, project.as_deref())?
    };
    
    let environment = if let Some(env) = environment {
        if !["local", "docker", "k8s"].contains(&env.as_str()) {
            anyhow::bail!("Invalid environment '{}'. Must be one of: local, docker, k8s", env);
        }
        env
    } else {
        prompt_for_environment()?
    };
    
    // Get the app to check available commands
    let app = config
        .projects
        .get(&resolved_project)
        .unwrap()
        .apps
        .get(&resolved_app_name)
        .unwrap();
    
    let command_name = if let Some(name) = command_name {
        name
    } else {
        prompt_for_command(app, &environment)?
    };
    
    // Get current command value
    let current_value = {
        
        match environment.as_str() {
            "local" => {
                app.commands.local.as_ref()
                    .and_then(|commands| commands.get(&command_name))
                    .cloned()
            }
            "docker" => {
                app.commands.docker.as_ref()
                    .and_then(|commands| commands.get(&command_name))
                    .cloned()
            }
            "k8s" => {
                app.commands.k8s.as_ref()
                    .and_then(|commands| commands.get(&command_name))
                    .cloned()
            }
            _ => unreachable!(),
        }
    };
    
    let current_value = current_value.ok_or_else(|| {
        anyhow::anyhow!("Command '{}' not found in {} environment for {}/{}. Add it first with 'config add-command'.", 
                        command_name, environment, resolved_project, resolved_app_name)
    })?;
    
    println!("Editing command: {}/{} {} {}", 
             resolved_project, resolved_app_name, environment, command_name);
    println!("Current value: {}", current_value);
    println!();
    
    // Prompt for new value using inquire
    use inquire::Text;
    
    let new_value = Text::new("Enter new command:")
        .with_default(&current_value)
        .with_help_message("Press Enter to keep current value, or type a new command")
        .prompt()?;
    
    if new_value == current_value {
        println!("No changes made.");
        return Ok(());
    }
    
    // Update the command
    let app = config
        .projects
        .get_mut(&resolved_project)
        .unwrap()
        .apps
        .get_mut(&resolved_app_name)
        .unwrap();
    
    match environment.as_str() {
        "local" => {
            app.commands.local.as_mut().unwrap().insert(command_name.clone(), new_value.to_string());
        }
        "docker" => {
            app.commands.docker.as_mut().unwrap().insert(command_name.clone(), new_value.to_string());
        }
        "k8s" => {
            app.commands.k8s.as_mut().unwrap().insert(command_name.clone(), new_value.to_string());
        }
        _ => unreachable!(),
    }
    
    // Save the updated config
    save_config(&config)?;
    
    println!("✓ Updated command '{}' for {}/{} ({} environment)", 
             command_name, resolved_project, resolved_app_name, environment);
    println!("  New value: {}", new_value);
    
    Ok(())
}

// Helper functions for interactive prompts

/// Prompt user to select a project first, then an app
fn prompt_for_app(config: &crate::config::Config, project_filter: Option<&str>) -> Result<(String, String)> {
    let project_name = if let Some(project) = project_filter {
        // Project already specified, use it
        project.to_string()
    } else {
        // Prompt user to select a project first
        prompt_for_project(config)?
    };
    
    // Get apps for the selected project
    let project = config.projects.get(&project_name)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", project_name))?;
    
    if project.apps.is_empty() {
        anyhow::bail!("No apps found in project '{}'", project_name);
    }
    
    // Create sorted list of app names
    let mut app_names: Vec<String> = project.apps.keys().cloned().collect();
    app_names.sort();
    
    let selection = Select::new("Select an app:", app_names).prompt()?;
    
    Ok((project_name, selection))
}

/// Prompt user to select a project
fn prompt_for_project(config: &crate::config::Config) -> Result<String> {
    if config.projects.is_empty() {
        anyhow::bail!("No projects found in config");
    }
    
    // Create sorted list of project names
    let mut project_names: Vec<String> = config.projects.keys().cloned().collect();
    project_names.sort();
    
    let selection = Select::new("Select a project:", project_names).prompt()?;
    Ok(selection)
}

/// Prompt user to select an environment
fn prompt_for_environment() -> Result<String> {
    let environments = vec!["local", "docker", "k8s"];
    let selection = Select::new("Select environment:", environments).prompt()?;
    Ok(selection.to_string())
}

/// Prompt user to select a command from available commands in an environment
fn prompt_for_command(app: &crate::config::App, environment: &str) -> Result<String> {
    let commands = match environment {
        "local" => app.commands.local.as_ref(),
        "docker" => app.commands.docker.as_ref(),
        "k8s" => app.commands.k8s.as_ref(),
        _ => return Err(anyhow::anyhow!("Invalid environment: {}", environment)),
    };
    
    let commands = commands.ok_or_else(|| {
        anyhow::anyhow!("No commands configured for {} environment", environment)
    })?;
    
    if commands.is_empty() {
        return Err(anyhow::anyhow!("No commands available in {} environment", environment));
    }
    
    // Create sorted list of command names with default indicator
    let default_command = match environment {
        "local" => app.defaults.local.as_ref(),
        "docker" => app.defaults.docker.as_ref(),
        "k8s" => app.defaults.k8s.as_ref(),
        _ => None,
    };
    
    let mut command_names: Vec<String> = commands.keys().cloned().collect();
    command_names.sort();
    
    // Add default indicator to the display
    let display_names: Vec<String> = command_names.iter()
        .map(|name| {
            if Some(name) == default_command {
                format!("{} (default)", name)
            } else {
                name.clone()
            }
        })
        .collect();
    
    let selection = Select::new("Select command:", display_names).prompt()?;
    
    // Remove the " (default)" suffix if present
    let clean_selection = if selection.ends_with(" (default)") {
        selection.trim_end_matches(" (default)").to_string()
    } else {
        selection
    };
    
    Ok(clean_selection)
}

/// Prompt user for a text input
fn prompt_for_text(message: &str, default: Option<&str>) -> Result<String> {
    let mut prompt = Text::new(message);
    if let Some(default_val) = default {
        prompt = prompt.with_default(default_val);
    }
    let input = prompt.prompt()?;
    Ok(input)
}