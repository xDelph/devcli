// Nx monorepo detection and multi-app handling functionality
// Handles detection of multiple apps in Nx workspaces
// Provides interactive selection for adding multiple apps at once

use crate::config::{load_config, save_config, App, Commands, Defaults, Project};
use crate::Result;
use std::collections::HashMap;

use super::interactive::{
    prompt_project_selection, prompt_app_name, show_preview, confirm_default_yes,
    interactive_nx_app_selection
};

// Handle detection and addition of apps in an Nx monorepo
// Scans apps/ and packages/ directories for Nx projects
// Allows the user to select which apps to add interactively
pub async fn handle_nx_monorepo(workspace_root: &std::path::Path) -> Result<()> {
    // Use the detection module to find all apps in apps/ and packages/ directories
    let detected_apps = crate::detection::detect_nx_apps(workspace_root)?;
    
    // Display detected apps count
    println!("✓ Detected Nx monorepo with {} apps", detected_apps.len());
    
    // Use interactive multi-select for app selection
    let selected_indices = interactive_nx_app_selection(&detected_apps)?;
    
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
    
    println!("\nProcessing {} selected app(s) for project '{}'...\n", selected_indices.len(), project_name);
    
    // Process each selected app with confirmation
    for &idx in &selected_indices {
        let detected = &detected_apps[idx];
        
        // Skip apps with no commands - they can't be run
        if detected.local_commands.is_none() 
            && detected.docker_commands.is_none() 
            && detected.k8s_commands.is_none() {
            println!("⚠ Skipping {} - no commands detected", detected.app_name);
            continue;
        }
        
        // Prompt for app name (allow user to customize)
        let app_name = prompt_app_name(&detected.app_name)?;
        
        // Show preview of what will be added
        show_preview(&project_name, &app_name, detected);
        
        // Confirm this specific app (default to yes)
        if !confirm_default_yes(&format!("Add {} to config?", app_name))? {
            println!("Skipped {}.\n", app_name);
            continue;
        }
        
        // Re-detect env files with interactive prompts for unspecified contexts
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
        // This creates the project if it doesn't exist, then adds the app
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
    println!("✓ All apps added successfully!");
    
    Ok(())
}

#[cfg(test)]
#[path = "nx_monorepo_test.rs"]
mod nx_monorepo_test;