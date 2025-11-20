//! Configuration listing and display functionality
//! 
//! This module handles displaying configuration information in various formats.
//! It provides the `rustycli config list`, `config show`, and `config list-commands` 
//! command functionality.

use crate::config::{list_all_apps, load_config, resolve_app};
use crate::Result;
use std::collections::HashMap;



/// List all projects and apps
/// 
/// Example: `rustycli config list`
/// Example: `rustycli config list --project qm`
/// Example: `rustycli config list --apps-only`
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

/// Show details of a specific app
/// 
/// Example: `rustycli config show api-private`
/// Example: `rustycli config show api --project qm`
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

/// List all commands for an app
/// 
/// Example: `rustycli config list-commands api-private`
/// Example: `rustycli config list-commands api --project qm`
/// Example: `rustycli config list-commands api-private --env local`
/// Example: `rustycli config list-commands` (interactive mode)
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
    
    // Validate environment filter if provided
    if let Some(ref env) = environment {
        use crate::config::models::Environment;
        if Environment::from_string(env.as_str()).is_none() {
            anyhow::bail!("Invalid environment '{}'. Must be one of: {}.", env, Environment::all_names());
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

#[cfg(test)]
#[path = "list_test.rs"]
mod tests;