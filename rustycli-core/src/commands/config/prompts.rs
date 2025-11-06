//! User interaction and prompt functionality
//! 
//! This module provides helper functions for interactive user prompts used across
//! the config command functionality. It handles app selection, project selection,
//! environment selection, and text input prompts.

use crate::Result;
use inquire::{Select, Text};



/// Prompt user to select a project first, then an app
/// 
/// # Arguments
/// 
/// * `config` - The loaded configuration
/// * `project_filter` - Optional project name to skip project selection
/// 
/// # Returns
/// 
/// A tuple of (project_name, app_name)
pub(crate) fn prompt_for_app(config: &crate::config::Config, project_filter: Option<&str>) -> Result<(String, String)> {
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
/// 
/// # Arguments
/// 
/// * `config` - The loaded configuration
/// 
/// # Returns
/// 
/// The selected project name
pub(crate) fn prompt_for_project(config: &crate::config::Config) -> Result<String> {
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
/// 
/// # Returns
/// 
/// The selected environment name (local, docker, or k8s)
pub(crate) fn prompt_for_environment() -> Result<String> {
    let environments = vec!["local", "docker", "k8s"];
    let selection = Select::new("Select environment:", environments).prompt()?;
    Ok(selection.to_string())
}

/// Prompt user to select a command from available commands in an environment
/// 
/// # Arguments
/// 
/// * `app` - The app configuration
/// * `environment` - The environment to get commands from
/// 
/// # Returns
/// 
/// The selected command name
pub(crate) fn prompt_for_command(app: &crate::config::App, environment: &str) -> Result<String> {
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
/// 
/// # Arguments
/// 
/// * `message` - The prompt message to display
/// * `default` - Optional default value
/// 
/// # Returns
/// 
/// The user's input
pub(crate) fn prompt_for_text(message: &str, default: Option<&str>) -> Result<String> {
    let mut prompt = Text::new(message);
    if let Some(default_val) = default {
        prompt = prompt.with_default(default_val);
    }
    let input = prompt.prompt()?;
    Ok(input)
}#[cfg(test)]
#[path = "prompts_test.rs"]
mod tests;