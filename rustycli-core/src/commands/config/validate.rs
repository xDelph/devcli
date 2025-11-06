//! Configuration validation functionality
//! 
//! This module handles comprehensive validation of the configuration file,
//! including path validation, dependency checking, and circular dependency detection.
//! It provides the `rustycli config validate` command functionality.

use crate::config::{
    dependencies::resolve_dependency_chain, list_all_apps, load_config, resolve_app,
};
use crate::Result;
use std::collections::HashSet;



/// Validate the config file
/// 
/// Example: `rustycli config validate`
///
/// Performs comprehensive validation:
/// - Are all paths valid?
/// - Do default commands exist?
/// - Are dependencies valid?
/// - Any circular dependencies?
/// - Any duplicate app names across projects?
/// 
/// # Errors
/// 
/// Returns an error if validation fails with any critical issues.
/// Warnings are displayed but don't cause failure.
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

#[cfg(test)]
#[path = "validate_test.rs"]
mod tests;