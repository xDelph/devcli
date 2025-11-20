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
    let commands = resolved_app.app.commands.get(&environment).ok_or_else(|| {
        use crate::config::models::Environment;
        if Environment::from_string(&environment).is_none() {
            anyhow::anyhow!(
                "Invalid environment '{}'. Must be one of: {}.",
                environment,
                Environment::all_names()
            )
        } else {
            anyhow::anyhow!(
                "App '{}' does not have '{}' environment configured. Available: {}",
                args.app_name,
                environment,
                get_available_environments(&resolved_app.app)
            )
        }
    })?;
    
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
    
    // Inject --platform flag for docker/orbstack commands
    let mut final_command = match environment.as_str() {
        "docker" | "orbstack" => {
            crate::utils::command::inject_docker_platform(&command, &preferences.docker_platform)
        }
        _ => command.clone()
    };
    
    // Build environment variables - inherit parent environment and set Docker context
    // Start with the current process's environment to inherit PATH, HOME, Docker config, etc.
    let mut env_vars: HashMap<String, String> = std::env::vars().collect();
    
    // Override/add specific variables based on environment
    match environment.as_str() {
        "docker" => {
            env_vars.insert("DOCKER_CONTEXT".to_string(), "default".to_string());
            
            // For Docker, use --env-file flag if .env exists
            // Priority: Dockerfile-level .env > root .env (using dockerfile_path from config)
            if let Ok(Some(env_file_path)) = crate::detection::find_env_file(&working_dir, resolved_app.app.dockerfile_path.as_deref()) {
                final_command = crate::utils::command::inject_docker_env_file(
                    &final_command,
                    &env_file_path
                );
            }
            
            // Inject dockerfile path for build commands
            if let Some(ref dockerfile_path) = resolved_app.app.dockerfile_path {
                final_command = crate::utils::command::inject_dockerfile_path(&final_command, dockerfile_path);
            }
        }
        "orbstack" => {
            env_vars.insert("DOCKER_CONTEXT".to_string(), "orbstack".to_string());
            
            // For OrbStack, use --env-file flag (same as Docker)
            // Priority: Dockerfile-level .env > root .env (using dockerfile_path from config)
            if let Ok(Some(env_file_path)) = crate::detection::find_env_file(&working_dir, resolved_app.app.dockerfile_path.as_deref()) {
                final_command = crate::utils::command::inject_docker_env_file(
                    &final_command,
                    &env_file_path
                );
            }
            
            // Inject dockerfile path for build commands
            if let Some(ref dockerfile_path) = resolved_app.app.dockerfile_path {
                final_command = crate::utils::command::inject_dockerfile_path(&final_command, dockerfile_path);
            }
        }
        _ => {}
    }
    
    // Build process options
    let options = ProcessOptions {
        app_name: process_name.clone(),
        working_dir: working_dir.clone(),
        command: final_command.clone(),
        env_vars,
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
    println!("Command: {}", final_command);
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
        command: final_command,
        working_dir: working_dir.to_string_lossy().to_string(),
        start_time: Utc::now(),
        env_vars: HashMap::new(),
        project: Some(resolved_app.project.clone()),
        app_config_name: Some(resolved_app.app_name.clone()),
        environment: Some(environment),
        command_variant: Some(args.command_variant), // Store the variant!
        stage: None, // Stage tracking will be added in future task
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
        
        // Poll for process exit or Ctrl+C
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("\n\nStopped viewing logs. Process is still running in the background.");
                    println!("Use 'rustycli status' to check process status.");
                    break;
                }
                _ = interval.tick() => {
                    // Check if process is still running
                    if !tracker.is_running(spawned.pid) {
                        println!("\n\nProcess exited.");
                        // Clean up the process from tracker
                        tracker.remove_process(&process_name)?;
                        break;
                    }
                }
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
    let envs = app.commands.available_envs();
    
    if envs.is_empty() {
        "none".to_string()
    } else {
        envs.join(", ")
    }
}
