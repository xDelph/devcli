//! Configuration editing functionality
//! 
//! This module handles interactive configuration editing, including opening the config
//! file in an editor and managing individual commands. It provides the `rustycli config edit`,
//! `config add-command`, `config remove-command`, `config set-default`, and 
//! `config edit-command` command functionality.

use crate::config::{load_config, resolve_app, save_config};
use crate::Result;
use std::collections::HashMap;
use std::env;
use std::process::Command;



/// Edit the config file in the user's editor
/// 
/// Example: `rustycli config edit`
///
/// Opens the config file in $EDITOR (or vim if not set).
/// After editing, suggests running validate to check changes.
/// 
/// # Errors
/// 
/// Returns an error if:
/// - Config file doesn't exist
/// - Editor command fails
/// - Cannot determine HOME directory
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

/// Add a command to an app
/// 
/// Example: `rustycli config add-command api-private local test "npm test"`
/// Example: `rustycli config add-command api --project qm docker build "docker build -t api ."`
/// Example: `rustycli config add-command` (interactive mode)
///
/// Adds a new command to the specified environment for an app.
/// 
/// # Arguments
/// 
/// * `app_name` - Optional app name (prompts if not provided)
/// * `project` - Optional project name to resolve ambiguous app names
/// * `environment` - Optional environment (local, docker, k8s) (prompts if not provided)
/// * `command_name` - Optional command name (prompts if not provided)
/// * `command_value` - Optional command value (prompts if not provided)
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
        crate::commands::config::prompt_for_app(&config, project.as_deref())?
    };
    
    let environment = if let Some(env) = environment {
        // Validate provided environment
        if !["local", "docker", "k8s"].contains(&env.as_str()) {
            anyhow::bail!("Invalid environment '{}'. Must be one of: local, docker, k8s", env);
        }
        env
    } else {
        // Prompt for environment
        crate::commands::config::prompt_for_environment()?
    };
    
    let command_name = if let Some(name) = command_name {
        name
    } else {
        crate::commands::config::prompt_for_text("Enter command name:", None)?
    };
    
    let command_value = if let Some(value) = command_value {
        value
    } else {
        crate::commands::config::prompt_for_text("Enter command value:", None)?
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

/// Remove a command from an app
/// 
/// Example: `rustycli config remove-command api-private local test`
/// Example: `rustycli config remove-command api --project qm docker build`
/// Example: `rustycli config remove-command` (interactive mode)
///
/// Removes a command from the specified environment for an app.
/// 
/// # Arguments
/// 
/// * `app_name` - Optional app name (prompts if not provided)
/// * `project` - Optional project name to resolve ambiguous app names
/// * `environment` - Optional environment (local, docker, k8s) (prompts if not provided)
/// * `command_name` - Optional command name (prompts if not provided)
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
        crate::commands::config::prompt_for_app(&config, project.as_deref())?
    };
    
    let environment = if let Some(env) = environment {
        if !["local", "docker", "k8s"].contains(&env.as_str()) {
            anyhow::bail!("Invalid environment '{}'. Must be one of: local, docker, k8s", env);
        }
        env
    } else {
        crate::commands::config::prompt_for_environment()?
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
        crate::commands::config::prompt_for_command(app, &environment)?
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

/// Set the default command for an environment
/// 
/// Example: `rustycli config set-default api-private local start`
/// Example: `rustycli config set-default api --project qm docker run`
/// Example: `rustycli config set-default` (interactive mode)
///
/// Sets which command should be used by default when starting an app in the specified environment.
/// 
/// # Arguments
/// 
/// * `app_name` - Optional app name (prompts if not provided)
/// * `project` - Optional project name to resolve ambiguous app names
/// * `environment` - Optional environment (local, docker, k8s) (prompts if not provided)
/// * `command_name` - Optional command name (prompts if not provided)
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
        crate::commands::config::prompt_for_app(&config, project.as_deref())?
    };
    
    let environment = if let Some(env) = environment {
        if !["local", "docker", "k8s"].contains(&env.as_str()) {
            anyhow::bail!("Invalid environment '{}'. Must be one of: local, docker, k8s", env);
        }
        env
    } else {
        crate::commands::config::prompt_for_environment()?
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
        crate::commands::config::prompt_for_command(app, &environment)?
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

/// Edit a specific command for an app
/// 
/// Example: `rustycli config edit-command api-private local start`
/// Example: `rustycli config edit-command api --project qm docker build`
/// Example: `rustycli config edit-command` (interactive mode)
///
/// Allows interactive editing of a command's value.
/// 
/// # Arguments
/// 
/// * `app_name` - Optional app name (prompts if not provided)
/// * `project` - Optional project name to resolve ambiguous app names
/// * `environment` - Optional environment (local, docker, k8s) (prompts if not provided)
/// * `command_name` - Optional command name (prompts if not provided)
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
        crate::commands::config::prompt_for_app(&config, project.as_deref())?
    };
    
    let environment = if let Some(env) = environment {
        if !["local", "docker", "k8s"].contains(&env.as_str()) {
            anyhow::bail!("Invalid environment '{}'. Must be one of: local, docker, k8s", env);
        }
        env
    } else {
        crate::commands::config::prompt_for_environment()?
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
        crate::commands::config::prompt_for_command(app, &environment)?
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
}#[cfg(test)]
#[path = "edit_test.rs"]
mod tests;