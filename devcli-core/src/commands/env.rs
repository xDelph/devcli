// Environment files management commands
// Allows users to add, remove, and configure env files for apps

use crate::config::{load_config, save_config};
use crate::Result;
use anyhow::anyhow;
use std::collections::HashMap;

/// Add an environment file to an app's configuration
///
/// # Arguments
/// * `app_name` - Name of the app to configure
/// * `stage` - Stage name (e.g., "qa", "prod", "dev")
/// * `context` - Context name (e.g., "local", "docker", "orbstack")
/// * `file_path` - Path to the env file relative to app root
pub fn add_env_file(app_name: &str, stage: &str, context: &str, file_path: &str) -> Result<()> {
    let mut config = load_config()?;

    // Find the app in the config
    let (_project_name, app) = config
        .projects
        .iter_mut()
        .find_map(|(proj_name, project)| {
            project
                .apps
                .get_mut(app_name)
                .map(|app| (proj_name.clone(), app))
        })
        .ok_or_else(|| anyhow!("App '{}' not found", app_name))?;

    // Initialize env_files if it doesn't exist
    if app.env_files.is_none() {
        app.env_files = Some(HashMap::new());
    }

    let env_files = app
        .env_files
        .as_mut()
        .expect("env_files should be Some after initialization");

    // Initialize stage if it doesn't exist
    if !env_files.contains_key(stage) {
        env_files.insert(stage.to_string(), HashMap::new());
    }

    let stage_map = env_files
        .get_mut(stage)
        .expect("stage should exist after insertion");

    // Add the file for this context
    stage_map.insert(context.to_string(), file_path.to_string());

    // If context is docker or orbstack, also set the other one
    if context == "docker" {
        stage_map.insert("orbstack".to_string(), file_path.to_string());
        stage_map.insert("docker-compose".to_string(), file_path.to_string());
    } else if context == "orbstack" {
        stage_map.insert("docker".to_string(), file_path.to_string());
        stage_map.insert("docker-compose".to_string(), file_path.to_string());
    } else if context == "docker-compose" {
        stage_map.insert("docker".to_string(), file_path.to_string());
        stage_map.insert("orbstack".to_string(), file_path.to_string());
    }

    save_config(&config)?;

    Ok(())
}

/// Remove an environment file or entire stage from an app's configuration
///
/// # Arguments
/// * `app_name` - Name of the app to configure
/// * `stage` - Stage name to remove
/// * `context` - Optional context name. If None, removes entire stage
pub fn remove_env_file(app_name: &str, stage: &str, context: Option<&str>) -> Result<()> {
    let mut config = load_config()?;

    // Find the app in the config
    let (_project_name, app) = config
        .projects
        .iter_mut()
        .find_map(|(proj_name, project)| {
            project
                .apps
                .get_mut(app_name)
                .map(|app| (proj_name.clone(), app))
        })
        .ok_or_else(|| anyhow!("App '{}' not found", app_name))?;

    let env_files = app
        .env_files
        .as_mut()
        .ok_or_else(|| anyhow!("No env files configured for app '{}'", app_name))?;

    if let Some(ctx) = context {
        // Remove specific context
        if let Some(stage_map) = env_files.get_mut(stage) {
            stage_map.remove(ctx);

            // If stage is now empty, remove it
            if stage_map.is_empty() {
                env_files.remove(stage);
            }
        } else {
            return Err(anyhow!("Stage '{}' not found", stage));
        }
    } else {
        // Remove entire stage
        env_files.remove(stage);
    }

    // If env_files is now empty, remove it
    if env_files.is_empty() {
        app.env_files = None;
    }

    save_config(&config)?;

    Ok(())
}

/// List all environment files configured for an app
///
/// # Arguments
/// * `app_name` - Name of the app to list env files for
pub fn list_env_files(app_name: &str) -> Result<()> {
    let config = load_config()?;

    // Find the app in the config
    let (project_name, app) = config
        .projects
        .iter()
        .find_map(|(proj_name, project)| project.apps.get(app_name).map(|app| (proj_name, app)))
        .ok_or_else(|| anyhow!("App '{}' not found", app_name))?;

    println!("Environment files for {}/{}:", project_name, app_name);
    println!();

    if let Some(env_files) = &app.env_files {
        // Sort stages for consistent display
        let mut sorted_stages: Vec<_> = env_files.iter().collect();
        sorted_stages.sort_by_key(|(stage, _)| stage.as_str());

        for (stage, contexts) in sorted_stages {
            println!("  {}:", stage);

            // Sort contexts for consistent display
            let mut sorted_contexts: Vec<_> = contexts.iter().collect();
            sorted_contexts.sort_by_key(|(context, _)| context.as_str());

            for (context, file_path) in sorted_contexts {
                println!("    • {}: {}", context, file_path);
            }
            println!();
        }
    } else {
        println!("  No env files configured");
    }

    // Show default stages if configured
    if let Some(default_stages) = &app.default_stages {
        println!("Default stages:");

        // Sort contexts for consistent display
        let mut sorted_defaults: Vec<_> = default_stages.iter().collect();
        sorted_defaults.sort_by_key(|(context, _)| context.as_str());

        for (context, stage) in sorted_defaults {
            println!("  • {}: {}", context, stage);
        }
    }

    Ok(())
}

/// Set default stage for an app's context
///
/// # Arguments
/// * `app_name` - Name of the app to configure
/// * `context` - Context name (e.g., "local", "docker", "orbstack", "k8s")
/// * `stage` - Stage name to set as default
pub fn set_default_stage(app_name: &str, context: &str, stage: &str) -> Result<()> {
    let mut config = load_config()?;

    // Find the app in the config
    let (project_name, app) = config
        .projects
        .iter_mut()
        .find_map(|(proj_name, project)| {
            project
                .apps
                .get_mut(app_name)
                .map(|app| (proj_name.clone(), app))
        })
        .ok_or_else(|| anyhow!("App '{}' not found", app_name))?;

    // Initialize default_stages if it doesn't exist
    if app.default_stages.is_none() {
        app.default_stages = Some(HashMap::new());
    }

    let default_stages = app
        .default_stages
        .as_mut()
        .expect("default_stages should be Some after initialization");

    // Set the default stage for the context
    if let Some(context) = crate::config::Environment::from_string(context) {
        default_stages.insert(context.as_str().to_string(), stage.to_string());
    } else {
        return Err(anyhow!("Unknown context: {}", context));
    }

    save_config(&config)?;

    println!("✓ Set default stage for {}/{}", project_name, app_name);
    println!("  Context: {}", context);
    println!("  Default stage: {}", stage);

    Ok(())
}
