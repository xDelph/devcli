//! Configuration editing functionality
//!
//! This module handles interactive configuration editing, including opening the config
//! file in an editor and managing individual commands. It provides the `devcli config edit`,
//! `config add-command`, `config remove-command`, `config set-default`, and
//! `config edit-command` command functionality.

use crate::config::models::Environment;
use crate::config::{load_config, resolve_app, save_config};
use crate::Result;
use std::env;
use std::process::Command;

/// Edit the config file in the user's editor
///
/// Example: `devcli config edit`
///
/// Opens the config file in $VISUAL / $EDITOR (or vim if neither is set).
/// After editing, suggests running validate to check changes.
///
/// # Errors
///
/// Returns an error if:
/// - Config file doesn't exist
/// - Editor command fails
pub async fn config_edit() -> Result<()> {
    // The same file every other command reads: honours DEVCLI_CONFIG_DIR and a
    // project-local ./.devcli, unlike a hard-coded ~/.devcli/config.json.
    let config_path = crate::config::loader::get_config_path()?;

    // Make sure config exists
    if !config_path.exists() {
        anyhow::bail!("Config file not found. Run 'devcli config init' to create one.");
    }

    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .ok()
        .filter(|e| !e.trim().is_empty())
        .unwrap_or_else(|| "vim".to_string());

    // `EDITOR="code -w"` / `"subl -n -w"` are common: split like a shell would
    // instead of looking for a program literally named "code -w".
    let parts = shell_words::split(&editor)
        .map_err(|e| anyhow::anyhow!("Cannot parse editor command '{}': {}", editor, e))?;
    let (program, args) = parts
        .split_first()
        .ok_or_else(|| anyhow::anyhow!("Editor command is empty"))?;

    let status = Command::new(program)
        .args(args)
        .arg(&config_path)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to launch editor '{}': {}", editor, e))?;

    // Check if the editor exited successfully
    if !status.success() {
        anyhow::bail!("Editor exited with error");
    }

    if crate::output::json_enabled() {
        return crate::output::print_json(&serde_json::json!({
            "path": config_path.display().to_string(),
            "editor": editor,
        }));
    }
    // Remind user to validate their changes
    println!("Config file updated. Run 'devcli config validate' to check your changes.");

    Ok(())
}

/// Add a command to an app
///
/// Example: `devcli config add-command api-private local test "npm test"`
/// Example: `devcli config add-command api --project qm docker build "docker build -t api ."`
/// Example: `devcli config add-command` (interactive mode)
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

    let env = resolve_environment(environment)?;
    let environment = env.as_str();

    let command_name = if let Some(name) = command_name {
        name
    } else {
        crate::commands::config::prompt_for_text(
            "Enter command name:",
            None,
            "the command name (COMMAND_NAME)",
        )?
    };

    let command_value = if let Some(value) = command_value {
        value
    } else {
        crate::commands::config::prompt_for_text(
            "Enter command value:",
            None,
            "the command (COMMAND_VALUE)",
        )?
    };

    // Get mutable reference to the app
    let app = config
        .projects
        .get_mut(&resolved_project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved_project))?
        .apps
        .get_mut(&resolved_app_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' not found in project '{}'",
                resolved_app_name,
                resolved_project
            )
        })?;

    // Add command to the appropriate environment
    app.commands
        .entry(env)
        .insert(command_name.clone(), command_value.clone());

    // Save the updated config
    save_config(&config)?;

    if crate::output::json_enabled() {
        return crate::output::print_json(&serde_json::json!({
            "project": resolved_project,
            "app": resolved_app_name,
            "environment": environment,
            "command": command_name,
            "value": command_value,
        }));
    }
    println!(
        "✓ Added command '{}' to {}/{} ({} environment)",
        command_name, resolved_project, resolved_app_name, environment
    );
    println!("  Command: {}", command_value);

    Ok(())
}

/// Remove a command from an app
///
/// Example: `devcli config remove-command api-private local test`
/// Example: `devcli config remove-command api --project qm docker build`
/// Example: `devcli config remove-command` (interactive mode)
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

    let env = resolve_environment(environment)?;
    let environment = env.as_str();

    // Get the app to check available commands
    let app = config
        .projects
        .get(&resolved_project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved_project))?
        .apps
        .get(&resolved_app_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' not found in project '{}'",
                resolved_app_name,
                resolved_project
            )
        })?;

    let command_name = if let Some(name) = command_name {
        name
    } else {
        crate::commands::config::prompt_for_command(app, environment)?
    };

    // Get mutable reference to the app
    let app = config
        .projects
        .get_mut(&resolved_project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved_project))?
        .apps
        .get_mut(&resolved_app_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' not found in project '{}'",
                resolved_app_name,
                resolved_project
            )
        })?;

    // Remove command from the appropriate environment
    let removed = app
        .commands
        .slot_mut(env)
        .as_mut()
        .is_some_and(|commands| commands.remove(&command_name).is_some());

    if !removed {
        anyhow::bail!(
            "Command '{}' not found in {} environment for {}/{}",
            command_name,
            environment,
            resolved_project,
            resolved_app_name
        );
    }

    // Check if this was the default command and warn user
    let default = app.defaults.slot_mut(env);
    let was_default = default.as_ref() == Some(&command_name);

    if was_default {
        // Clear the default since the command no longer exists
        *default = None;
    }

    // Save the updated config
    save_config(&config)?;

    if crate::output::json_enabled() {
        return crate::output::print_json(&serde_json::json!({
            "project": resolved_project,
            "app": resolved_app_name,
            "environment": environment,
            "command": command_name,
            "default_cleared": was_default,
        }));
    }
    if was_default {
        println!(
            "⚠ Warning: '{}' was the default {} command. Default cleared.",
            command_name, environment
        );
    }

    println!(
        "✓ Removed command '{}' from {}/{} ({} environment)",
        command_name, resolved_project, resolved_app_name, environment
    );

    Ok(())
}

/// Set the default command for an environment
///
/// Example: `devcli config set-default api-private local start`
/// Example: `devcli config set-default api --project qm docker run`
/// Example: `devcli config set-default` (interactive mode)
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

    let env = resolve_environment(environment)?;
    let environment = env.as_str();

    // Get the app to check available commands
    let app = config
        .projects
        .get(&resolved_project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved_project))?
        .apps
        .get(&resolved_app_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' not found in project '{}'",
                resolved_app_name,
                resolved_project
            )
        })?;

    let command_name = if let Some(name) = command_name {
        name
    } else {
        crate::commands::config::prompt_for_command(app, environment)?
    };

    // Get mutable reference to the app
    let app = config
        .projects
        .get_mut(&resolved_project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved_project))?
        .apps
        .get_mut(&resolved_app_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' not found in project '{}'",
                resolved_app_name,
                resolved_project
            )
        })?;

    // Verify the command exists in the specified environment
    let command_exists = app
        .commands
        .get(environment)
        .is_some_and(|commands| commands.contains_key(&command_name));

    if !command_exists {
        anyhow::bail!("Command '{}' not found in {} environment for {}/{}. Add it first with 'config add-command'.", 
                     command_name, environment, resolved_project, resolved_app_name);
    }

    // Set the default command
    *app.defaults.slot_mut(env) = Some(command_name.clone());

    // Save the updated config
    save_config(&config)?;

    if crate::output::json_enabled() {
        return crate::output::print_json(&serde_json::json!({
            "project": resolved_project,
            "app": resolved_app_name,
            "environment": environment,
            "default": command_name,
        }));
    }

    println!(
        "✓ Set '{}' as default {} command for {}/{}",
        command_name, environment, resolved_project, resolved_app_name
    );

    Ok(())
}

/// Edit a specific command for an app
///
/// Example: `devcli config edit-command api-private local start`
/// Example: `devcli config edit-command api --project qm docker build`
/// Example: `devcli config edit-command` (interactive mode)
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
    // The new value is always typed into a prompt.
    if crate::output::json_enabled() {
        anyhow::bail!(
            "config edit-command prompts for the new value, which --json does not allow; \
             use `config add-command APP ENVIRONMENT COMMAND_NAME VALUE` to replace it"
        );
    }
    let mut config = load_config()?;

    // Interactive prompts for missing parameters
    let (resolved_project, resolved_app_name) = if let Some(app_name) = app_name {
        let resolved = resolve_app(&config, &app_name, project.as_deref())?;
        (resolved.project, resolved.app_name)
    } else {
        crate::commands::config::prompt_for_app(&config, project.as_deref())?
    };

    let env = resolve_environment(environment)?;
    let environment = env.as_str();

    // Get the app to check available commands
    let app = config
        .projects
        .get(&resolved_project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved_project))?
        .apps
        .get(&resolved_app_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' not found in project '{}'",
                resolved_app_name,
                resolved_project
            )
        })?;

    let command_name = if let Some(name) = command_name {
        name
    } else {
        crate::commands::config::prompt_for_command(app, environment)?
    };

    // Get current command value
    let current_value = app
        .commands
        .get(environment)
        .and_then(|commands| commands.get(&command_name))
        .cloned();

    let current_value = current_value.ok_or_else(|| {
        anyhow::anyhow!("Command '{}' not found in {} environment for {}/{}. Add it first with 'config add-command'.", 
                        command_name, environment, resolved_project, resolved_app_name)
    })?;

    println!(
        "Editing command: {}/{} {} {}",
        resolved_project, resolved_app_name, environment, command_name
    );
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
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved_project))?
        .apps
        .get_mut(&resolved_app_name)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' not found in project '{}'",
                resolved_app_name,
                resolved_project
            )
        })?;

    app.commands
        .slot_mut(env)
        .as_mut()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No {} commands defined for {}",
                environment,
                resolved_app_name
            )
        })?
        .insert(command_name.clone(), new_value.to_string());

    // Save the updated config
    save_config(&config)?;

    println!(
        "✓ Updated command '{}' for {}/{} ({} environment)",
        command_name, resolved_project, resolved_app_name, environment
    );
    println!("  New value: {}", new_value);

    Ok(())
}

/// Parse `--env` (aliases such as `compose` / `kubernetes` accepted), or prompt
/// for it when absent.
///
/// The commands used to validate against all six environments but then only
/// handle local/docker/k8s, hitting `unreachable!()` - a panic - for orbstack,
/// docker-compose and ci.
fn resolve_environment(environment: Option<String>) -> Result<Environment> {
    let name = match environment {
        Some(name) => name,
        None => crate::commands::config::prompt_for_environment()?,
    };
    Environment::from_string(&name).ok_or_else(|| {
        anyhow::anyhow!(
            "Invalid environment '{}'. Must be one of: {}.",
            name,
            Environment::all_names()
        )
    })
}

#[cfg(test)]
#[path = "edit_test.rs"]
mod tests;
