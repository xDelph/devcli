// Start command - Launches apps using configuration
// This is the main command for starting processes in detached mode
//
// Example: rustycli start api-private
// Flow: Load config → Resolve app → Check deps → Start process

use crate::config::{
    dependencies::{check_dependencies_running, resolve_dependency_chain},
    load_config, load_preferences, resolve_app,
};
use crate::logging::FileLogger;
use crate::process::{spawn_process, ProcessInfo, ProcessOptions, ProcessTracker};
use crate::utils::path::expand_path;
use crate::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Arguments passed to the start command
// These come from the CLI parser (clap)
pub struct StartCommandArgs {
    pub app_name: String,           // Name of the app from config (e.g., "api-private")
    pub project: Option<String>,    // Optional: specify project if name is ambiguous
    pub env: Option<String>,        // Optional: "local" or "docker" (overrides preference)
    pub skip_deps: bool,            // If true, don't check/start dependencies
}

// Main implementation of the start command
// This is async because we do I/O operations (files, processes)
pub async fn start_command(args: StartCommandArgs) -> Result<()> {
    // Step 1: Load the main configuration file
    // This contains all projects, apps, commands, and dependencies
    let config = load_config()?;
    
    // Step 2: Load user preferences
    // This contains settings like default environment (local/docker)
    let preferences = load_preferences()?;
    
    // Step 3: Find the app in the config
    // resolve_app searches all projects for the app name
    // If found in multiple projects, it will error unless --project is specified
    let resolved_app = resolve_app(&config, &args.app_name, args.project.as_deref())?;
    
    // Step 4: Determine which environment to use
    // Priority: --env flag > preferences > default to "local"
    // .clone() creates copies because we need to own the String
    // .unwrap_or_else() provides a default if args.env is None
    let environment = args
        .env
        .clone()
        .unwrap_or_else(|| preferences.default_env.clone());
    
    // Step 5: Get the commands HashMap for the chosen environment
    // Each app can have ANY combination of environments configured
    // We check if the requested environment exists, otherwise give a helpful error
    let commands = match environment.as_str() {
        "local" => resolved_app.app.commands.local.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' does not have 'local' environment configured. Available: {}",
                args.app_name,
                get_available_environments(&resolved_app.app)
            )
        })?,
        "docker" => resolved_app.app.commands.docker.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' does not have 'docker' environment configured. Available: {}",
                args.app_name,
                get_available_environments(&resolved_app.app)
            )
        })?,
        _ => anyhow::bail!(
            "Invalid environment '{}'. Must be 'local' or 'docker'.",
            environment
        ),
    };
    
    // Step 6: Get the default command name for this environment
    // The config specifies which command to run by default
    // Example: defaults.local = "start" means use the "start" command
    let default_command = match environment.as_str() {
        "local" => resolved_app.app.defaults.local.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' does not have a default command for 'local' environment",
                args.app_name
            )
        })?,
        "docker" => resolved_app.app.defaults.docker.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' does not have a default command for 'docker' environment",
                args.app_name
            )
        })?,
        _ => unreachable!(), // We already validated environment above
    };
    
    // Step 7: Look up the actual command string
    // commands is a HashMap<String, String> like { "start": "npm start", ... }
    // We get the command string corresponding to the default command name
    let command = commands
        .get(default_command)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Default command '{}' not found in {} commands for app '{}'",
                default_command,
                environment,
                args.app_name
            )
        })?
        .clone();
    
    // Step 8: Check if this app is already running
    // ProcessTracker manages PID files in ~/.rustycli/pids/
    let tracker = ProcessTracker::new()?;
    
    // First, clean up any dead processes to keep state accurate
    // This removes PID files for processes that have crashed or been killed
    tracker.cleanup_dead()?;
    
    // Try to get existing process info for this app
    // .get_process() returns Option<ProcessInfo>
    if let Some(existing) = tracker.get_process(&args.app_name)? {
        // Check if the process is actually still running
        // The PID file might exist but the process could have died
        if tracker.is_running(existing.pid) {
            // Process is running - can't start it again
            anyhow::bail!(
                "Process '{}' is already running with PID {}",
                args.app_name,
                existing.pid
            );
        } else {
            // PID file exists but process is dead - clean it up
            println!(
                "Cleaning up stale process '{}' (PID {} is no longer running)",
                args.app_name, existing.pid
            );
            tracker.remove_process(&args.app_name)?;
        }
    }
    
    // Step 9: Handle dependencies (unless --skip-deps is set)
    if !args.skip_deps {
        // Resolve the full dependency chain
        // This returns a list of all apps this one depends on
        // Example: Worker → API → Redis gives us [Redis, API]
        let dependencies = resolve_dependency_chain(&config, &resolved_app)?;
        
        // Only check if there are actually dependencies
        if !dependencies.is_empty() {
            println!("Checking dependencies...");
            
            // Check which dependencies are NOT running
            // Returns a Vec of missing dependency names
            let missing = check_dependencies_running(&tracker, &dependencies)?;
            
            // If any dependencies are missing, abort
            if !missing.is_empty() {
                anyhow::bail!(
                    "Missing dependencies: {}. Start them first or use --skip-deps",
                    missing.join(", ")
                );
            }
            
            // All dependencies are running!
            println!("✓ All dependencies are running");
        }
    }
    
    // Step 10: Expand the working directory path
    // This handles ~ and environment variables
    // Example: "~/Projects/app" → "/Users/username/Projects/app"
    let working_dir = expand_path(&resolved_app.app.path);
    
    // Verify the directory actually exists
    if !working_dir.exists() {
        anyhow::bail!(
            "Working directory does not exist: {}",
            working_dir.display()
        );
    }
    
    // Step 11: Create a log file for this process
    // Arc<Mutex<FileLogger>> allows sharing the logger across async tasks:
    //   Arc: Multiple tasks can have a reference to it
    //   Mutex: Only one task can write to it at a time
    let log_writer = Arc::new(Mutex::new(FileLogger::new(&args.app_name).await?));
    
    // Get the log file path for display
    // Scope with {} ensures we release the lock quickly
    let log_path = {
        // Lock the mutex to access the logger
        let writer = log_writer.lock().await;
        // Get and clone the path (clone creates a copy)
        writer.log_path().clone()
    }; // Lock is automatically released here
    
    // Step 12: Build the options struct for spawning the process
    // We need .clone() because Rust's ownership system doesn't allow
    // moving the same value multiple times
    let options = ProcessOptions {
        app_name: args.app_name.clone(),
        working_dir: working_dir.clone(),
        command: command.clone(),
        env_vars: HashMap::new(), // We don't set custom env vars (yet)
        detached: true,            // Always run in detached mode for start command
    };
    
    // Step 13: Display info to the user about what we're doing
    println!(
        "Starting process '{}' in {} (environment: {})",
        args.app_name,
        working_dir.display(), // .display() formats PathBuf nicely
        environment
    );
    println!("Command: {}", command);
    println!("Log file: {}", log_path.display());
    
    // Step 14: Actually spawn the process!
    // This returns info about the spawned process (PID, etc.)
    let spawned = spawn_process(options, log_writer.clone()).await?;
    
    // Step 14.5: Wait a moment and verify the process didn't immediately crash
    // In detached mode, the process might start but immediately fail (e.g., port conflict)
    // We wait 2 seconds to give it time to fail if something is wrong
    // This catches most startup failures like missing files, port conflicts, etc.
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    
    // Check if the process is still running
    if !tracker.is_running(spawned.pid) {
        anyhow::bail!(
            "Process '{}' started but immediately crashed (PID: {}). Check the log file: {}",
            args.app_name,
            spawned.pid,
            log_path.display()
        );
    }
    
    // Step 15: Create metadata about this process for tracking
    // This will be saved to ~/.rustycli/pids/<app-name>.json
    let process_info = ProcessInfo {
        app_name: args.app_name.clone(),
        pid: spawned.pid,           // Process ID from the OS
        command,
        // .to_string_lossy() converts PathBuf to String
        // "lossy" means invalid UTF-8 characters become �
        working_dir: working_dir.to_string_lossy().to_string(),
        start_time: Utc::now(),     // Current time in UTC
        env_vars: HashMap::new(),
        // New fields for config system
        project: Some(resolved_app.project.clone()),
        app_config_name: Some(resolved_app.app_name.clone()),
        environment: Some(environment),
        command_variant: Some(default_command.clone()),
    };
    
    // Step 16: Save the process info to a PID file
    // This allows us to track and manage the process later
    tracker.register_process(process_info)?;
    
    // Step 17: Show success message
    println!(
        "✓ Process '{}' started successfully with PID {}",
        args.app_name, spawned.pid
    );
    println!("Process is running in detached mode");
    
    // Return Ok(()) to indicate success
    Ok(())
}

// Helper function to show which environments are configured for an app
// Used in error messages to help users understand what's available
fn get_available_environments(app: &crate::config::models::App) -> String {
    let mut envs = Vec::new();
    
    // Check which environments have commands defined
    if app.commands.local.is_some() {
        envs.push("local");
    }
    if app.commands.docker.is_some() {
        envs.push("docker");
    }
    
    // Return as comma-separated string, or "none" if empty
    if envs.is_empty() {
        "none".to_string()
    } else {
        envs.join(", ")
    }
}
