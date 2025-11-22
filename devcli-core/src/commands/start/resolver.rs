//! App resolution and validation logic
//! 
//! This module handles:
//! - Parsing and validating start command arguments
//! - Resolving app names to configuration entries
//! - Validating that requested environments exist for each app
//! - Checking for already running processes

use crate::config::{load_config, load_preferences, resolve_app};
use crate::process::ProcessTracker;
use crate::Result;



/// Arguments passed to the start command
/// These come from the CLI parser (clap)
#[derive(Clone)]
pub struct StartCommandArgs {
    pub app_names: Vec<String>,     // Names of the apps from config (e.g., ["api-private", "redis.local"])
    pub project: Option<String>,    // Optional: specify project if name is ambiguous
    pub env: Option<String>,        // Optional: "local" or "docker" (overrides preference)
    pub skip_deps: bool,            // If true, don't check/start dependencies
    pub silent: bool,               // If true, don't show output to terminal (for TUI mode)
    pub stage: Option<String>,      // Optional: deployment stage override (dev, qa, preprod, prod)
}

/// Information about an app that's ready to start
pub struct AppToStart {
    pub resolved_app: crate::config::resolver::ResolvedApp,
    pub command: String,
    pub default_command: String,
}

/// Resolve all apps to start and validate them
/// 
/// Returns a tuple of (apps_to_start, environment) where:
/// - apps_to_start: Vector of validated apps ready to start
/// - environment: The resolved environment name (local/docker/k8s)
pub async fn resolve_apps_to_start(args: StartCommandArgs) -> Result<(Vec<AppToStart>, String)> {
    // Step 1: Load the main configuration file and user preferences
    let config = load_config()?;
    let preferences = load_preferences()?;
    
    let silent = args.silent;
    
    // Step 2: Determine which environment to use for ALL apps
    // Priority: --env flag > preferences.default_env > "local"
    let environment = args
        .env
        .clone()
        .unwrap_or_else(|| preferences.default_env.clone());
    
    // Step 3: Initialize process tracker and clean up any dead processes
    let tracker = ProcessTracker::new()?;
    tracker.cleanup_dead()?;
    
    // Step 4: Collect all apps to start and validate them
    let mut apps_to_start = Vec::new();
    
    // Process each app name provided by the user
    for app_name in &args.app_names {
        // Find the app in the config based on name and optional project
        let resolved_app = resolve_app(&config, app_name, args.project.as_deref())?;
        
        // Check if this app is already running
        if let Some(existing) = tracker.get_process(app_name)? {
            if tracker.is_running(existing.pid) {
                if !silent {
                    println!("⚠ Process '{}' is already running with PID {}, skipping", app_name, existing.pid);
                }
                continue;
            } else {
                if !silent {
                    println!("Cleaning up stale process '{}' (PID {} is no longer running)", app_name, existing.pid);
                }
                tracker.remove_process(app_name)?;
            }
        }
        
        // Validate that the chosen environment exists for this app and get commands
        let (command, default_command) = validate_and_get_command(&resolved_app, &environment, app_name)?;
        
        // Store this app's info for later processing
        apps_to_start.push(AppToStart {
            resolved_app,
            command,
            default_command,
        });
    }
    
    Ok((apps_to_start, environment))
}

/// Validate that the chosen environment exists for an app and get the command to run
/// 
/// Returns a tuple of (command_string, default_command_name)
pub fn validate_and_get_command(
    resolved_app: &crate::config::resolver::ResolvedApp,
    environment: &str,
    app_name: &str,
) -> Result<(String, String)> {
    // Validate that the chosen environment exists for this app
    let commands = resolved_app.app.commands.get(environment).ok_or_else(|| {
        use crate::config::models::Environment;
        if Environment::from_string(environment).is_none() {
            anyhow::anyhow!(
                "Invalid environment '{}'. Must be one of: {}.",
                environment,
                Environment::all_names()
            )
        } else {
            anyhow::anyhow!(
                "App '{}' does not have '{}' environment configured. Available: {}",
                app_name,
                environment,
                get_available_environments(&resolved_app.app)
            )
        }
    })?;
    
    // Get the default command name for this environment
    let default_command = resolved_app.app.defaults.get(environment).ok_or_else(|| {
        anyhow::anyhow!(
            "App '{}' does not have a default command for '{}' environment",
            app_name,
            environment
        )
    })?.clone();
    
    // Look up the actual command string from the commands HashMap
    let command = commands
        .get(&default_command)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Default command '{}' not found in {} commands for app '{}'",
                default_command,
                environment,
                app_name
            )
        })?
        .clone();
    
    Ok((command, default_command))
}

/// Helper function to show which environments are configured for an app
/// Used in error messages to help users understand what's available
pub fn get_available_environments(app: &crate::config::models::App) -> String {
    // Use the available_envs method from Commands which dynamically checks all environments
    let envs = app.commands.available_envs();
    
    // Return as comma-separated string, or "none" if empty
    if envs.is_empty() {
        "none".to_string()
    } else {
        envs.join(", ")
    }
}