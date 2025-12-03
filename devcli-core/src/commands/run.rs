// Run command - Execute specific command variants for apps
// Similar to start, but lets you choose which command to run
//
// Example: devcli run api-private build:production
// This runs the "build:production" command instead of the default

use crate::config::{load_config, load_preferences, resolve_app};
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
    pub app_name: String,        // Name of the app from config
    pub command_variant: String, // Which command to run (e.g., "build:production", "test")
    pub project: Option<String>, // Optional: specify project if ambiguous
    pub env: Option<String>,     // Optional: environment override
    pub skip_deps: bool,         // If true, skip dependency checks
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
                crate::utils::app::get_available_environments(&resolved_app.app)
            )
        }
    })?;

    // DIFFERENCE FROM START: Look up the SPECIFIC command variant
    // Instead of using the default, we use the variant provided by the user
    // Example: If user runs "devcli run api build:prod"
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
    // The format is "appName:variantName"
    let process_name = format!("{}:{}", args.app_name, args.command_variant);

    // Check if THIS SPECIFIC command is already running for this app
    if let Ok(Some(process)) = tracker.get_process(
        &resolved_app.project,
        &resolved_app.app_name,
        Some(&environment),
    ) {
        if tracker.is_running(process.pid) {
            anyhow::bail!(
                "Process '{}' is already running with PID {}",
                process_name,
                process.pid
            );
        } else {
            // Clean up stale PID file
            tracker.remove_process(
                &resolved_app.project,
                &resolved_app.app_name,
                Some(&environment),
            )?;
        }
    }

    // Dependency checking
    // We reuse the shared `handle_dependencies` function from the start command
    // This ensures consistent behavior: dependencies are checked and optionally auto-started
    if !args.skip_deps {
        use crate::commands::start::handle_dependencies;
        // Pass a slice of references to ResolvedApp, as expected by the generic handler
        handle_dependencies(&[&resolved_app], &environment, !preferences.detached_mode).await?;
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
    let log_writer = Arc::new(Mutex::new(
        FileLogger::new(&resolved_app.project, &args.app_name, &environment, true).await?,
    ));

    let log_path = {
        let writer = log_writer.lock().await;
        writer.log_path().clone()
    };

    // Determine which stage to use for env file resolution
    // Priority: app default_stages > preferences default_stage > None
    let stage = resolved_app
        .app
        .get_default_stage(&environment, preferences.default_stage.as_deref());

    // Step 3: Prepare command and environment variables
    let prepared = crate::commands::prepare::prepare_command(
        &command,
        &environment,
        &resolved_app,
        stage.as_deref(),
        &working_dir,
        &preferences,
        !preferences.detached_mode, // show_output logic for run command
    )?;

    let final_command = prepared.final_command;
    let env_vars = prepared.env_vars;

    // Build process options
    let options = ProcessOptions {
        app_name: process_name.clone(),
        working_dir: working_dir.clone(),
        command: final_command.clone(),
        env_vars,
        detached: true,                          // Always detached (with setsid)
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
        environment: Some(environment.clone()),
        command_variant: Some(args.command_variant), // Store the variant!
        stage: None,                                 // Stage tracking will be added in future task
    };

    tracker.register_process(process_info)?;

    println!(
        "✓ Process '{}' started successfully with PID {}",
        process_name, spawned.pid
    );

    // Ensure background monitor is running
    // The monitor keeps process status up-to-date and cleans up dead processes
    let binary_path = crate::process::monitor::get_devcli_binary_path()?;
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
                    println!("Use 'devcli status' to check process status.");
                    break;
                }
                _ = interval.tick() => {
                    // Check if process is still running
                    if !tracker.is_running(spawned.pid) {
                        println!("\n\nProcess exited.");
                        // Clean up the process from tracker
                     tracker.remove_process(&resolved_app.project, &resolved_app.app_name, Some(&environment))?;
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
