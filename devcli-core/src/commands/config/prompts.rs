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
pub(crate) fn prompt_for_app(
    config: &crate::config::Config,
    project_filter: Option<&str>,
) -> Result<(String, String)> {
    let project_name = if let Some(project) = project_filter {
        // Project already specified, use it
        project.to_string()
    } else {
        // Prompt user to select a project first
        prompt_for_project(config)?
    };

    // Get apps for the selected project
    let project = config
        .projects
        .get(&project_name)
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
/// The selected environment name (local, docker, orbstack, or k8s)
pub(crate) fn prompt_for_environment() -> Result<String> {
    use crate::config::models::Environment;

    let environments: Vec<&str> = Environment::all().iter().map(|e| e.as_str()).collect();
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
    use crate::config::models::Environment;

    // Every environment: this used to handle local/docker/orbstack/k8s only,
    // so `config set-default api docker-compose` (or ci) failed with "not
    // supported in this context", and the "(default)" marker ignored orbstack.
    let env = Environment::parse(environment).map_err(|e| anyhow::anyhow!(e))?;
    let names = command_choices(app, env)?;
    let selection = Select::new("Select command:", names).prompt()?;

    // Remove the " (default)" suffix if present
    Ok(selection
        .strip_suffix(DEFAULT_MARKER)
        .map(str::to_string)
        .unwrap_or(selection))
}

/// Suffix marking the environment's default command in [`command_choices`].
const DEFAULT_MARKER: &str = " (default)";

/// The sorted command names of `env`, the default one marked.
pub(crate) fn command_choices(
    app: &crate::config::App,
    env: crate::config::models::Environment,
) -> Result<Vec<String>> {
    let environment = env.as_str();
    let commands = app
        .commands
        .get(environment)
        .ok_or_else(|| anyhow::anyhow!("No commands configured for {} environment", environment))?;

    if commands.is_empty() {
        return Err(anyhow::anyhow!(
            "No commands available in {} environment",
            environment
        ));
    }

    let default_command = app.defaults.get(environment);
    let mut command_names: Vec<String> = commands.keys().cloned().collect();
    command_names.sort();

    Ok(command_names
        .into_iter()
        .map(|name| {
            if Some(&name) == default_command {
                format!("{name}{DEFAULT_MARKER}")
            } else {
                name
            }
        })
        .collect())
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
}
#[cfg(test)]
#[path = "prompts_test.rs"]
mod tests;
