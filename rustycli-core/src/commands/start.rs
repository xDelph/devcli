// Start command - Launches apps using configuration
// This is the main command for starting processes in detached mode
//
// Features:
// - Can start multiple apps at once: rustycli start redis.local traefik.local api.local
// - In non-detached mode, keeps main process alive to show logs from all apps
// - Handles dependencies for all apps collectively
//
// Examples: 
//   rustycli start api-private                    (single app)
//   rustycli start redis.local traefik.local     (multiple apps)
//   rustycli start api --env docker               (with environment override)
//
// Flow: Load config → Resolve all apps → Check deps → Start apps in parallel → Show logs (if not detached)

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
    pub app_names: Vec<String>,     // Names of the apps from config (e.g., ["api-private", "redis.local"])
    pub project: Option<String>,    // Optional: specify project if name is ambiguous
    pub env: Option<String>,        // Optional: "local" or "docker" (overrides preference)
    pub skip_deps: bool,            // If true, don't check/start dependencies
}

// Main implementation of the start command
// This is async because we do I/O operations (files, processes)
// Handles multiple apps and keeps process alive for log viewing in non-detached mode
pub async fn start_command(args: StartCommandArgs) -> Result<()> {
    // Validate input: we need at least one app name
    if args.app_names.is_empty() {
        anyhow::bail!("At least one app name must be provided");
    }

    // Step 1: Load the main configuration file and user preferences
    // This contains all projects, apps, commands, and dependencies
    let config = load_config()?;
    // This contains settings like default environment (local/docker) and detached_mode
    let preferences = load_preferences()?;
    
    // Step 2: Determine which environment to use for ALL apps
    // Priority: --env flag > preferences.default_env > "local"
    // .clone() creates copies because we need to own the String
    // .unwrap_or_else() provides a default if args.env is None
    let environment = args
        .env
        .clone()
        .unwrap_or_else(|| preferences.default_env.clone());
    
    // Step 3: Initialize process tracker and clean up any dead processes
    // ProcessTracker manages PID files in ~/.rustycli/pids/
    let tracker = ProcessTracker::new()?;
    // This removes PID files for processes that have crashed or been killed
    tracker.cleanup_dead()?;
    
    // Step 4: Collect all apps to start and validate them
    // We process multiple apps in a loop and collect their info
    let mut apps_to_start = Vec::new(); // Will store (ResolvedApp, command, default_command) tuples
    let mut all_dependencies = Vec::new(); // Will collect dependencies from ALL apps
    
    // Process each app name provided by the user
    for app_name in &args.app_names {
        // Find the app in the config based on name and optional project
        // resolve_app searches all projects for the app name
        // If found in multiple projects, it will error unless --project is specified
        let resolved_app = resolve_app(&config, app_name, args.project.as_deref())?;
        
        // Check if this app is already running
        // .get_process() returns Option<ProcessInfo>
        if let Some(existing) = tracker.get_process(app_name)? {
            // Check if the process is actually still running
            // The PID file might exist but the process could have died
            if tracker.is_running(existing.pid) {
                // Process is running - skip it and continue with other apps
                println!("⚠ Process '{}' is already running with PID {}, skipping", app_name, existing.pid);
                continue;
            } else {
                // PID file exists but process is dead - clean it up
                println!("Cleaning up stale process '{}' (PID {} is no longer running)", app_name, existing.pid);
                tracker.remove_process(app_name)?;
            }
        }
        
        // Validate that the chosen environment exists for this app
        // Each app can have ANY combination of environments configured
        // We check if the requested environment exists, otherwise give a helpful error
        let commands = match environment.as_str() {
            "local" => resolved_app.app.commands.local.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "App '{}' does not have 'local' environment configured. Available: {}",
                    app_name,
                    get_available_environments(&resolved_app.app)
                )
            })?,
            "docker" => resolved_app.app.commands.docker.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "App '{}' does not have 'docker' environment configured. Available: {}",
                    app_name,
                    get_available_environments(&resolved_app.app)
                )
            })?,
            "k8s" => resolved_app.app.commands.k8s.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "App '{}' does not have 'k8s' environment configured. Available: {}",
                    app_name,
                    get_available_environments(&resolved_app.app)
                )
            })?,
            _ => anyhow::bail!(
                "Invalid environment '{}'. Must be 'local', 'docker', or 'k8s'.",
                environment
            ),
        };
        
        // Get the default command name for this environment
        // The config specifies which command to run by default
        // Example: defaults.local = "start" means use the "start" command
        // .clone() is needed because we need to own the String (Rust ownership rules)
        let default_command = match environment.as_str() {
            "local" => resolved_app.app.defaults.local.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "App '{}' does not have a default command for 'local' environment",
                    app_name
                )
            })?.clone(),
            "docker" => resolved_app.app.defaults.docker.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "App '{}' does not have a default command for 'docker' environment",
                    app_name
                )
            })?.clone(),
            "k8s" => resolved_app.app.defaults.k8s.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "App '{}' does not have a default command for 'k8s' environment",
                    app_name
                )
            })?.clone(),
            _ => unreachable!(), // We already validated environment above
        };
        
        // Look up the actual command string from the commands HashMap
        // commands is a HashMap<String, String> like { "start": "npm start", ... }
        // We get the command string corresponding to the default command name
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
        
        // Collect dependencies for this app (unless --skip-deps is set)
        // We collect dependencies from ALL apps and will handle them collectively
        if !args.skip_deps {
            // Resolve the full dependency chain for this app
            // This returns a list of all apps this one depends on
            // Example: Worker → API → Redis gives us [Redis, API]
            let dependencies = resolve_dependency_chain(&config, &resolved_app)?;
            all_dependencies.extend(dependencies);
        }
        
        // Store this app's info for later processing
        // Tuple contains: (resolved app info, command to run, command name)
        apps_to_start.push((resolved_app, command, default_command));
    }
    
    // Step 5: Handle dependencies for all apps collectively
    // Instead of handling dependencies per app, we collect them all and handle once
    if !args.skip_deps && !all_dependencies.is_empty() {
        // Remove duplicates by app name (since ResolvedApp doesn't implement Ord/PartialEq)
        // We sort first, then remove consecutive duplicates
        all_dependencies.sort_by(|a, b| a.app_name.cmp(&b.app_name));
        all_dependencies.dedup_by(|a, b| a.app_name == b.app_name);
        
        println!("Checking dependencies for all apps...");
        
        // Check which dependencies are NOT running
        // Returns a Vec of missing dependency names in "project/app" format
        let missing = check_dependencies_running(&tracker, &all_dependencies)?;
        
        // If any dependencies are missing, handle based on preferences
        if !missing.is_empty() {
            // Check if user wants to auto-start dependencies
            if preferences.auto_start_deps {
                println!("⚠ Missing dependencies: {}", missing.join(", "));
                println!("Starting dependencies in parallel...\n");
                
                // Collect all dependency start tasks to run in parallel
                // This is faster than starting them sequentially
                let mut tasks = Vec::new();
                
                for dep_name in &missing {
                    // Parse "project/app" format
                    let parts: Vec<&str> = dep_name.split('/').collect();
                    if parts.len() == 2 {
                        let dep_project = parts[0].to_string();
                        let dep_app = parts[1].to_string();
                        // Dependencies inherit the parent's resolved environment
                        let env = environment.clone();
                        
                        println!("→ Starting: {}/{}", dep_project, dep_app);
                        
                        // Show output if user preference is to see logs in terminal
                        let show_output = !preferences.detached_mode;
                        
                        // Spawn async task for this dependency
                        // Each runs independently and in parallel
                        let task = tokio::spawn(async move {
                            let dep_args = StartCommandArgs {
                                app_names: vec![dep_app.clone()],
                                project: Some(dep_project.clone()),
                                env: Some(env),
                                skip_deps: false,
                            };
                            
                            start_single_app_internal(dep_args, show_output).await
                                .map_err(|e| format!("{}/{}: {}", dep_project, dep_app, e))
                        });
                        
                        tasks.push(task);
                    }
                }
                
                // Wait for all dependency tasks to complete
                let results = futures::future::join_all(tasks).await;
                
                // Collect any errors that occurred
                let mut errors = Vec::new();
                for result in results {
                    match result {
                        Ok(Ok(_)) => {}, // Success - do nothing
                        Ok(Err(e)) => errors.push(e), // App failed to start
                        Err(e) => errors.push(format!("Task failed: {}", e)), // Task itself failed
                    }
                }
                
                // If any dependencies failed to start, abort
                if !errors.is_empty() {
                    anyhow::bail!(
                        "Failed to start some dependencies:\n  {}",
                        errors.join("\n  ")
                    );
                }
                
                println!("\n✓ All dependencies started successfully");
            } else {
                // If auto_start_deps is false, abort with an error
                anyhow::bail!(
                    "Missing dependencies: {}. Start them first or use --skip-deps",
                    missing.join(", ")
                );
            }
        } else {
            // All dependencies are running!
            println!("✓ All dependencies are running");
        }
    }
    
    // Step 6: Start all apps in parallel
    // This is where the magic happens - we start multiple apps concurrently
    println!("\nStarting {} app(s) in parallel...", apps_to_start.len());
    
    let mut tasks = Vec::new();
    // Determine if we should show output based on detached mode preference
    // If detached_mode is false, we want to see logs in the terminal
    let show_output = !preferences.detached_mode;
    
    // Create an async task for each app to start
    for (resolved_app, command, default_command) in apps_to_start {
        let app_name = resolved_app.app_name.clone();
        let environment = environment.clone();
        
        // tokio::spawn creates a new concurrent task
        // 'async move' captures variables and runs asynchronously
        let task = tokio::spawn(async move {
            start_single_app_process(resolved_app, command, default_command, environment, show_output).await
                .map_err(|e| format!("{}: {}", app_name, e))
        });
        
        tasks.push(task);
    }
    
    // Wait for all apps to start (or fail)
    // futures::future::join_all waits for ALL tasks to complete
    let results = futures::future::join_all(tasks).await;
    let mut errors = Vec::new();
    let mut started_apps = Vec::new();
    
    // Process the results from all tasks
    for result in results {
        match result {
            Ok(Ok(app_name)) => started_apps.push(app_name), // App started successfully
            Ok(Err(e)) => errors.push(e), // App failed to start
            Err(e) => errors.push(format!("Task failed: {}", e)), // Task itself failed
        }
    }
    
    // Report results to the user
    if !started_apps.is_empty() {
        println!("\n✓ Successfully started {} app(s): {}", started_apps.len(), started_apps.join(", "));
    }
    
    if !errors.is_empty() {
        if started_apps.is_empty() {
            // All apps failed - this is a complete failure
            anyhow::bail!("Failed to start all apps:\n  {}", errors.join("\n  "));
        } else {
            // Some apps started, some failed - show warning but continue
            println!("\n⚠ Some apps failed to start:\n  {}", errors.join("\n  "));
        }
    }
    
    // Step 7: Handle different modes and keep process alive for log viewing
    // Display mode information based on user preferences
    if preferences.detached_mode {
        // In detached mode, we exit immediately after starting processes
        println!("\nRunning in background (detached mode, no terminal output)");
    } else {
        // In non-detached mode, keep the main process alive to show logs
        println!("\nRunning in background with output streaming (use Ctrl+C to stop viewing)");
        
        // Explain to user what's happening and how to exit
        println!("Press Ctrl+C to stop viewing logs (processes will continue running)...\n");
        
        // Set up Ctrl+C signal handler using tokio's async signal handling
        // This allows us to gracefully handle the interrupt signal
        let ctrl_c = tokio::signal::ctrl_c();
        
        // Wait for Ctrl+C signal (this blocks until user presses Ctrl+C)
        // The processes are already running and streaming their output to terminal
        match ctrl_c.await {
            Ok(()) => {
                // User pressed Ctrl+C - explain what happened
                println!("\n\nStopped viewing logs. Processes are still running in the background.");
                println!("Use 'rustycli status' to check process status.");
            }
            Err(err) => {
                // Something went wrong with signal handling
                println!("Unable to listen for shutdown signal: {}", err);
            }
        }
    }
    
    // Step 8: Ensure background monitor is running
    // The monitor keeps process status up-to-date and cleans up dead processes
    let binary_path = crate::process::monitor::get_rustycli_binary_path()?;
    let _ = crate::process::monitor::spawn_monitor_if_needed(&binary_path);
    
    Ok(())
}

// Helper function to start a single app process
// This is used by the parallel app starting logic
// Returns the app name on success for reporting
// Args:
//   - resolved_app: App configuration and metadata
//   - command: The actual command string to execute (e.g., "npm start")
//   - default_command: The command name (e.g., "start") for tracking
//   - environment: Environment name (local/docker/k8s) for tracking
//   - show_output: Whether to stream process output to terminal (with colored app name)
async fn start_single_app_process(
    resolved_app: crate::config::resolver::ResolvedApp,
    command: String,
    default_command: String,
    environment: String,
    show_output: bool,
) -> Result<String> {
    let app_name = resolved_app.app_name.clone();
    
    // Step 1: Expand the working directory path
    // This handles ~ and environment variables
    // Example: "~/Projects/app" → "/Users/username/Projects/app"
    let working_dir = expand_path(&resolved_app.app.path);
    
    // Verify the directory actually exists
    if !working_dir.exists() {
        anyhow::bail!("Working directory does not exist: {}", working_dir.display());
    }
    
    // Step 2: Create a log file for this process
    // Arc<Mutex<FileLogger>> allows sharing the logger across async tasks:
    //   Arc: Multiple tasks can have a reference to it
    //   Mutex: Only one task can write to it at a time
    let log_writer = Arc::new(Mutex::new(FileLogger::new(&app_name).await?));
    
    // Get the log file path for display
    // Scope with {} ensures we release the lock quickly
    let log_path = {
        // Lock the mutex to access the logger
        let writer = log_writer.lock().await;
        // Get and clone the path (clone creates a copy)
        writer.log_path().clone()
    }; // Lock is automatically released here
    
    // Step 3: Build the options struct for spawning the process
    // We need .clone() because Rust's ownership system doesn't allow
    // moving the same value multiple times
    let options = ProcessOptions {
        app_name: app_name.clone(),
        working_dir: working_dir.clone(),
        command: command.clone(),
        env_vars: HashMap::new(), // We don't set custom env vars (yet)
        detached: true,            // Always run in detached mode (with setsid)
        show_output,               // Show output based on detached_mode preference
    };
    
    // Step 4: Display info to the user about what we're doing
    println!("→ Starting '{}' in {} (environment: {})", app_name, working_dir.display(), environment);
    println!("  Command: {}", command);
    println!("  Log file: {}", log_path.display());
    
    // Step 5: Actually spawn the process!
    // This returns info about the spawned process (PID, etc.)
    let spawned = spawn_process(options, log_writer.clone()).await?;
    
    // Step 6: Wait a moment and verify the process didn't immediately crash
    // In detached mode, the process might start but immediately fail (e.g., port conflict)
    // We wait 2 seconds to give it time to fail if something is wrong
    // This catches most startup failures like missing files, port conflicts, etc.
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    
    // Check if the process is still running
    let tracker = ProcessTracker::new()?;
    if !tracker.is_running(spawned.pid) {
        anyhow::bail!(
            "Process '{}' started but immediately crashed (PID: {}). Check the log file: {}",
            app_name,
            spawned.pid,
            log_path.display()
        );
    }
    
    // Step 7: Create metadata about this process for tracking
    // This will be saved to ~/.rustycli/pids/<app-name>.json
    let process_info = ProcessInfo {
        app_name: app_name.clone(),
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
        command_variant: Some(default_command),
    };
    
    // Step 8: Save the process info to a PID file
    // This allows us to track and manage the process later
    tracker.register_process(process_info)?;
    
    // Step 9: Success! Return the app name for reporting
    Ok(app_name)
}

// Helper function for internal use (starting dependencies)
// This is a simplified version of start_command that handles exactly one app
// Used when starting dependencies in parallel
// Args:
//   - args: Standard start command arguments (but app_names must have exactly 1 item)
//   - show_output: Whether to stream process output to terminal (with colored app name)
// Returns: Result indicating success or failure
async fn start_single_app_internal(args: StartCommandArgs, show_output: bool) -> Result<()> {
    // Validate that we have exactly one app name (this is for dependencies)
    if args.app_names.len() != 1 {
        anyhow::bail!("start_single_app_internal expects exactly one app name");
    }
    
    let app_name = &args.app_names[0];
    
    // Step 1: Load configuration files (same as main start_command)
    let config = load_config()?;
    let preferences = load_preferences()?;
    
    // Step 2: Resolve which app to start
    // This finds the app in the config based on name and optional project
    let resolved_app = resolve_app(&config, app_name, args.project.as_deref())?;
    
    // Step 3: Determine which environment to use
    // Priority: args.env > preferences.default_env
    let environment = args
        .env
        .clone()
        .unwrap_or_else(|| preferences.default_env.clone());
    
    // Step 4: Get the commands HashMap for the selected environment
    // Fail if the environment doesn't exist for this app
    let commands = match environment.as_str() {
        "local" => resolved_app.app.commands.local.as_ref().ok_or_else(|| {
            anyhow::anyhow!("App '{}' does not have 'local' environment configured", app_name)
        })?,
        "docker" => resolved_app.app.commands.docker.as_ref().ok_or_else(|| {
            anyhow::anyhow!("App '{}' does not have 'docker' environment configured", app_name)
        })?,
        "k8s" => resolved_app.app.commands.k8s.as_ref().ok_or_else(|| {
            anyhow::anyhow!("App '{}' does not have 'k8s' environment configured", app_name)
        })?,
        _ => anyhow::bail!("Invalid environment '{}'", environment),
    };
    
    // Step 5: Get the default command name for this environment
    // Example: "start" or "serve" for local, "run" for docker, "apply" for k8s
    let default_command = match environment.as_str() {
        "local" => resolved_app.app.defaults.local.as_ref().ok_or_else(|| {
            anyhow::anyhow!("App '{}' does not have a default command for 'local' environment", app_name)
        })?,
        "docker" => resolved_app.app.defaults.docker.as_ref().ok_or_else(|| {
            anyhow::anyhow!("App '{}' does not have a default command for 'docker' environment", app_name)
        })?,
        "k8s" => resolved_app.app.defaults.k8s.as_ref().ok_or_else(|| {
            anyhow::anyhow!("App '{}' does not have a default command for 'k8s' environment", app_name)
        })?,
        _ => unreachable!(), // We already validated environment above
    }.clone();
    
    // Step 6: Look up the actual command string from the commands HashMap
    // Example: "start" → "npm run start"
    let command = commands
        .get(&default_command)
        .ok_or_else(|| {
            anyhow::anyhow!("Default command '{}' not found for app '{}'", default_command, app_name)
        })?
        .clone();
    
    // Step 7: Initialize process tracker and clean up stale PIDs
    let tracker = ProcessTracker::new()?;
    tracker.cleanup_dead()?;
    
    // Step 8: Check if this dependency is already running
    if let Some(existing) = tracker.get_process(app_name)? {
        if tracker.is_running(existing.pid) {
            // Already running - no need to start again
            return Ok(());
        } else {
            // Stale PID file - clean it up
            tracker.remove_process(app_name)?;
        }
    }
    
    // Step 9: Check if this dependency has its own dependencies
    // (dependencies can depend on other apps too)
    if !args.skip_deps {
        let dependencies = resolve_dependency_chain(&config, &resolved_app)?;
        if !dependencies.is_empty() {
            let missing = check_dependencies_running(&tracker, &dependencies)?;
            if !missing.is_empty() {
                // This dependency has missing dependencies - fail
                anyhow::bail!("Missing dependencies: {}", missing.join(", "));
            }
        }
    }
    
    // Step 10: Start the single app using our helper function
    start_single_app_process(resolved_app, command, default_command, environment, show_output).await?;
    
    // Step 11: Ensure background monitor is running
    // The monitor keeps process status up-to-date and cleans up dead processes
    let binary_path = crate::process::monitor::get_rustycli_binary_path()?;
    let _ = crate::process::monitor::spawn_monitor_if_needed(&binary_path);
    
    // Step 12: Success!
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
    if app.commands.k8s.is_some() {
        envs.push("k8s");
    }
    
    // Return as comma-separated string, or "none" if empty
    if envs.is_empty() {
        "none".to_string()
    } else {
        envs.join(", ")
    }
}
