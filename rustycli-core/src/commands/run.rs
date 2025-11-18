// Run command - Execute specific command variants for apps
// Similar to start, but lets you choose which command to run
//
// Example: rustycli run api-private build:production
// This runs the "build:production" command instead of the default

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

// Arguments for the run command
pub struct RunCommandArgs {
    pub app_name: String,           // Name of the app from config
    pub command_variant: String,    // Which command to run (e.g., "build:production", "test")
    pub project: Option<String>,    // Optional: specify project if ambiguous
    pub env: Option<String>,        // Optional: environment override
    pub skip_deps: bool,            // If true, skip dependency checks
}

// Main implementation of the run command
// Very similar to start_command, but uses a specific command variant instead of default
pub async fn run_command(args: RunCommandArgs) -> Result<()> {
    // Load config and preferences (same as start command)
    let config = load_config()?;
    let preferences = load_preferences()?;
    
    // Resolve the app in the config
    let resolved_app = resolve_app(&config, &args.app_name, args.project.as_deref())?;
    
    // Determine environment (same priority as start)
    let environment = args
        .env
        .clone()
        .unwrap_or_else(|| preferences.default_env.clone());
    
    // Get the commands for the chosen environment
    // Check if the app has commands defined for this environment
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
        "k8s" => resolved_app.app.commands.k8s.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' does not have 'k8s' environment configured. Available: {}",
                args.app_name,
                get_available_environments(&resolved_app.app)
            )
        })?,
        _ => anyhow::bail!(
            "Invalid environment '{}'. Must be 'local', 'docker', or 'k8s'.",
            environment
        ),
    };
    
    // DIFFERENCE FROM START: Look up the SPECIFIC command variant
    // Instead of using the default, we use the variant provided by the user
    // Example: If user runs "rustycli run api build:prod"
    // We look for "build:prod" in the commands HashMap
    let command = commands
        .get(&args.command_variant)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Command variant '{}' not found in {} commands for app '{}'",
                args.command_variant,
                environment,
                args.app_name
            )
        })?
        .clone();
    
    let tracker = ProcessTracker::new()?;
    
    // Clean up any stale PID files from crashed processes
    tracker.cleanup_dead()?;
    
    // DIFFERENCE FROM START: Process name includes the variant
    // This allows running multiple commands for the same app simultaneously
    // Example: "api-private:build" and "api-private:start" can both run
    let process_name = format!("{}:{}", args.app_name, args.command_variant);
    
    // Check if THIS SPECIFIC command is already running for this app
    if let Some(existing) = tracker.get_process(&process_name)? {
        if tracker.is_running(existing.pid) {
            anyhow::bail!(
                "Process '{}' is already running with PID {}",
                process_name,
                existing.pid
            );
        } else {
            // Clean up stale PID file
            tracker.remove_process(&process_name)?;
        }
    }
    
    // Dependency checking (same as start command)
    if !args.skip_deps {
        let dependencies = resolve_dependency_chain(&config, &resolved_app)?;
        
        if !dependencies.is_empty() {
            println!("Checking dependencies...");
            let missing = check_dependencies_running(&tracker, &dependencies)?;
            
            if !missing.is_empty() {
                anyhow::bail!(
                    "Missing dependencies: {}. Start them first or use --skip-deps",
                    missing.join(", ")
                );
            }
            
            println!("✓ All dependencies are running");
        }
    }
    
    // Expand path and verify it exists
    let working_dir = expand_path(&resolved_app.app.path);
    
    if !working_dir.exists() {
        anyhow::bail!(
            "Working directory does not exist: {}",
            working_dir.display()
        );
    }
    
    // Create log file using the full process name (includes variant)
    let log_writer = Arc::new(Mutex::new(FileLogger::new(&process_name).await?));
    
    let log_path = {
        let writer = log_writer.lock().await;
        writer.log_path().clone()
    };
    
    // Build process options
    let options = ProcessOptions {
        app_name: process_name.clone(),
        working_dir: working_dir.clone(),
        command: command.clone(),
        env_vars: HashMap::new(),
        detached: true, // Always detached (with setsid)
        show_output: !preferences.detached_mode, // Show output based on preference
    };
    
    // Display what we're doing
    // Note: We show the command variant being executed
    println!(
        "Running command '{}' for app '{}' (environment: {})",
        args.command_variant, args.app_name, environment
    );
    println!("Working directory: {}", working_dir.display());
    println!("Command: {}", command);
    println!("Log file: {}", log_path.display());
    
    // Spawn the process
    let spawned = spawn_process(options, log_writer.clone(), None).await?;
    
    // Wait a moment to check if the process completed or crashed
    // For build commands, completing quickly is expected behavior
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    
    // Check if the process is still running
    let process_still_running = tracker.is_running(spawned.pid);
    
    if !process_still_running {
        // Process completed - check if it was successful or crashed
        // For short-running commands like builds, this is normal
        
        println!(
            "✓ Process '{}' completed (PID: {}). Check log for details: {}",
            process_name,
            spawned.pid,
            log_path.display()
        );
        
        // If not in detached mode, we already showed the output during spawn_process
        // Don't register completed processes in the tracker
        return Ok(());
    }
    
    // Save process metadata
    // Note: command_variant field stores the specific variant used
    let process_info = ProcessInfo {
        app_name: process_name.clone(),
        pid: spawned.pid,
        command,
        working_dir: working_dir.to_string_lossy().to_string(),
        start_time: Utc::now(),
        env_vars: HashMap::new(),
        project: Some(resolved_app.project.clone()),
        app_config_name: Some(resolved_app.app_name.clone()),
        environment: Some(environment),
        command_variant: Some(args.command_variant), // Store the variant!
    };
    
    tracker.register_process(process_info)?;
    
    println!(
        "✓ Process '{}' started successfully with PID {}",
        process_name, spawned.pid
    );
    
    // Ensure background monitor is running
    // The monitor keeps process status up-to-date and cleans up dead processes
    let binary_path = crate::process::monitor::get_rustycli_binary_path()?;
    let _ = crate::process::monitor::spawn_monitor_if_needed(&binary_path);
    
    // Setup log monitoring based on detached mode preference
    // If not in detached mode, this will wait for Ctrl+C while streaming logs
    if !preferences.detached_mode {
        println!("\nRunning in background with output streaming (use Ctrl+C to stop viewing)");
        println!("Press Ctrl+C to stop viewing logs (process will continue running)...\n");
        
        // Wait for Ctrl+C signal
        let ctrl_c = tokio::signal::ctrl_c();
        match ctrl_c.await {
            Ok(()) => {
                println!("\n\nStopped viewing logs. Process is still running in the background.");
                println!("Use 'rustycli status' to check process status.");
            }
            Err(err) => {
                println!("Unable to listen for shutdown signal: {}", err);
            }
        }
    } else {
        println!("\nRunning in background (detached mode, no terminal output)");
    }
    
    Ok(())
}

// Helper function to show which environments are configured for an app
// Used in error messages to help users understand what's available
fn get_available_environments(app: &crate::config::models::App) -> String {
    let mut envs = Vec::new();
    
    if app.commands.local.is_some() {
        envs.push("local");
    }
    if app.commands.docker.is_some() {
        envs.push("docker");
    }
    if app.commands.k8s.is_some() {
        envs.push("k8s");
    }
    
    if envs.is_empty() {
        "none".to_string()
    } else {
        envs.join(", ")
    }
}
