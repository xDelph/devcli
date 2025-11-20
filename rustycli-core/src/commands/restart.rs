// Restart command - Restart running applications
// Allows restarting applications in exactly the same state they were running
//
// Example: rustycli restart api
// Example: rustycli restart api --project qm
// Flow: Find running process → Stop it → Start same process with same config

use crate::config::{
    dependencies::{check_dependencies_running, resolve_dependency_chain},
    load_config, load_preferences, resolve_app,
};
use crate::logging::FileLogger;
use crate::process::{spawn_process, ProcessInfo, ProcessOptions, ProcessTracker};
use crate::utils::path::expand_path;
use crate::Result;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;

// Arguments for the restart command
pub struct RestartCommandArgs {
    pub app_name: String,        // Name of the app to restart
    pub project: Option<String>, // Optional: specify project if name is ambiguous
    pub env: Option<String>,     // Optional: "local" or "docker" (overrides existing config)
    pub skip_deps: bool,         // If true, don't check/start dependencies
    pub silent: bool,            // If true, don't print to terminal (for TUI mode)
}

// Main implementation of the restart command
// Restart means: stop existing process → start same process with same config
pub async fn restart_command(args: RestartCommandArgs) -> Result<()> {
    let silent = args.silent;
    
    // Step 1: Load the main configuration file
    let config = load_config()?;

    // Step 2: Load user preferences
    let preferences = load_preferences()?;

    // Step 3: Get the process tracker and clean up dead processes
    let tracker = ProcessTracker::new()?;
    tracker.cleanup_dead()?;

    // Step 4: Check if the process is currently running
    let existing_process = if let Some(process) = tracker.get_process(&args.app_name)? {
        // Check if process is still actually running
        if tracker.is_running(process.pid) {
            Some(process)
        } else {
            // Process died but we have PID file - clean it up
            tracker.remove_process(&args.app_name)?;
            if !silent {
                println!(
                    "Process '{}' was not running (cleaning up stale PID file)",
                    args.app_name
                );
            }
            None
        }
    } else {
        None
    };

    // Step 5: If process is not running, we can't restart it - suggest starting instead
    if existing_process.is_none() {
        anyhow::bail!(
            "Process '{}' is not currently running. Use 'start' command instead.",
            args.app_name
        );
    }

    let process = existing_process.unwrap();

    // Step 6: Stop the existing process
    if !silent {
        println!(
            "Stopping process '{}' (PID: {})...",
            args.app_name, process.pid
        );
    }

    // Platform-specific process termination
    #[cfg(unix)]
    {
        use std::process::Command;
        // Send SIGTERM (graceful shutdown)
        if let Err(e) = Command::new("kill")
            .args(["-TERM", &process.pid.to_string()])
            .output()
        {
            if !silent {
                println!("Warning: Failed to send SIGTERM: {}", e);
            }
        }

        // Wait a bit for graceful shutdown
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // If still running, force kill
        if tracker.is_running(process.pid) {
            if !silent {
                println!("Force killing process '{}'...", args.app_name);
            }
            if let Err(e) = Command::new("kill")
                .args(["-KILL", &process.pid.to_string()])
                .output()
            {
                if !silent {
                    println!("Warning: Failed to send SIGKILL: {}", e);
                }
            }

            // Wait for process to actually die
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    #[cfg(not(unix))]
    {
        // For non-Unix platforms, we don't have process management yet
        anyhow::bail!("Process termination not implemented for this platform");
    }

    // Remove the PID file
    tracker.remove_process(&args.app_name)?;
    if !silent {
        println!("✓ Process '{}' stopped", args.app_name);
    }

    // Step 7: Prepare to start the same process again with identical configuration
    if !silent {
        println!("Restarting process '{}'...", args.app_name);
    }

    // Determine environment: use --env flag if provided, otherwise use existing process's environment
    let environment = args.env.clone().unwrap_or_else(|| {
        process
            .environment
            .clone()
            .unwrap_or_else(|| preferences.default_env.clone())
    });

    // Determine project: use --project flag if provided, otherwise use existing process's project
    let project = args.project.clone().or(process.project.clone());

    // Find the app in the config using the resolved project (if any)
    let resolved_app = resolve_app(&config, &args.app_name, project.as_deref())?;

    // Step 8: Get the commands for the determined environment
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

    // Step 9: Get the default command name for this environment
    // Use the same command that was running before (from the existing process)
    let default_command = if let Some(existing_variant) = &process.command_variant {
        // Use the same command variant that was running
        existing_variant.clone()
    } else {
        // Fallback to default for this environment
        resolved_app.app.defaults.get(&environment).ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' does not have a default command for '{}' environment",
                args.app_name,
                environment
            )
        })?.clone()
    };

    // Step 10: Look up the actual command string
    let command = commands
        .get(&default_command)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Command '{}' not found in {} commands for app '{}'",
                default_command,
                environment,
                args.app_name
            )
        })?
        .clone();

    // Step 11: Check dependencies (unless --skip-deps is set)
    if !args.skip_deps {
        let dependencies = resolve_dependency_chain(&config, &resolved_app)?;

        if !dependencies.is_empty() {
            if !silent {
                println!("Checking dependencies...");
            }

            let missing = check_dependencies_running(&tracker, &dependencies)?;

            if !missing.is_empty() {
                if preferences.auto_start_deps {
                    if !silent {
                        println!("⚠ Missing dependencies: {}", missing.join(", "));
                        println!("Starting dependencies in parallel...\n");
                    }

                    let mut tasks = Vec::new();

                    for dep_name in &missing {
                        let parts: Vec<&str> = dep_name.split('/').collect();
                        if parts.len() == 2 {
                            let dep_project = parts[0].to_string();
                            let dep_app = parts[1].to_string();
                            let env = environment.clone();

                            if !silent {
                                println!("→ Starting: {}/{}", dep_project, dep_app);
                            }

                            let task = tokio::spawn(async move {
                                // Use StartCommandArgs structure with app_names as Vec
                                let dep_args = crate::commands::start::StartCommandArgs {
                                    app_names: vec![dep_app.clone()], // Vec with single dependency
                                    project: Some(dep_project.clone()),
                                    env: Some(env),
                                    skip_deps: false,
                                    silent, // Pass through silent flag
                                };

                                crate::commands::start::start_command(dep_args)
                                    .await
                                    .map_err(|e| format!("{}/{}: {}", dep_project, dep_app, e))
                            });

                            tasks.push(task);
                        }
                    }

                    let results = futures::future::join_all(tasks).await;

                    let mut errors = Vec::new();
                    for result in results {
                        match result {
                            Ok(Ok(_)) => {}
                            Ok(Err(e)) => errors.push(e),
                            Err(e) => errors.push(format!("Task failed: {}", e)),
                        }
                    }

                    if !errors.is_empty() {
                        anyhow::bail!(
                            "Failed to start some dependencies:\n  {}",
                            errors.join("\n  ")
                        );
                    }

                    if !silent {
                        println!("\n✓ All dependencies started successfully");
                    }
                } else {
                    anyhow::bail!(
                        "Missing dependencies: {}. Start them first or use --skip-deps",
                        missing.join(", ")
                    );
                }
            } else if !silent {
                println!("✓ All dependencies are running");
            }
        }
    }

    // Step 12: Expand the working directory path (use the same as before)
    let working_dir = expand_path(&resolved_app.app.path);

    if !working_dir.exists() {
        anyhow::bail!(
            "Working directory does not exist: {}",
            working_dir.display()
        );
    }

    // Step 13: Create a log file for the restarted process
    let log_writer = Arc::new(Mutex::new(FileLogger::new(&args.app_name).await?));

    let log_path = {
        let writer = log_writer.lock().await;
        writer.log_path().clone()
    };

    // Step 14: Inject --platform flag for docker/orbstack commands
    let mut final_command = match environment.as_str() {
        "docker" | "orbstack" => {
            crate::utils::command::inject_docker_platform(&command, &preferences.docker_platform)
        }
        _ => command.clone()
    };
    
    // Step 14a: Build environment variables - inherit parent environment and reload for OrbStack
    // Start with the current process's environment to inherit PATH, HOME, Docker config, etc.
    let mut env_vars: std::collections::HashMap<String, String> = std::env::vars().collect();
    
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
    
    // Build the options struct for spawning the process
    let options = ProcessOptions {
        app_name: args.app_name.clone(),
        working_dir: working_dir.clone(),
        command: final_command.clone(),
        env_vars,
        detached: true,
        show_output: !silent && !preferences.detached_mode,
    };

    // Step 15: Display info about the restart
    if !silent {
        println!(
            "Restarting process '{}' in {} (environment: {})",
            args.app_name,
            working_dir.display(),
            environment
        );
        println!("Command: {}", final_command);
        println!("Log file: {}", log_path.display());
    }

    // Step 16: Spawn the process
    let spawned = spawn_process(options, log_writer.clone(), None).await?;

    // Step 17: Wait and verify the process didn't crash immediately
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

    if !tracker.is_running(spawned.pid) {
        anyhow::bail!(
            "Process '{}' restarted but immediately crashed (PID: {}). Check the log file: {}",
            args.app_name,
            spawned.pid,
            log_path.display()
        );
    }

    // Step 18: Create and save new process info
    let process_info = ProcessInfo {
        app_name: args.app_name.clone(),
        pid: spawned.pid,
        command: final_command,
        working_dir: working_dir.to_string_lossy().to_string(),
        start_time: Utc::now(),
        env_vars: std::collections::HashMap::new(), // Don't store env vars in PID file
        project: Some(resolved_app.project.clone()),
        app_config_name: Some(resolved_app.app_name.clone()),
        environment: Some(environment),
        command_variant: Some(default_command),
        stage: None, // Stage tracking will be added in future task
    };

    tracker.register_process(process_info)?;

    // Step 19: Show success message
    if !silent {
        println!(
            "✓ Process '{}' restarted successfully with PID {}",
            args.app_name, spawned.pid
        );

        if preferences.detached_mode {
            println!("Running in background (detached mode, no terminal output)");
        } else {
            println!("Running in background with output streaming (use Ctrl+C to stop viewing)");
        }
    }

    // Step 20: Ensure background monitor is running
    let binary_path = crate::process::monitor::get_rustycli_binary_path()?;
    let _ = crate::process::monitor::spawn_monitor_if_needed(&binary_path);

    Ok(())
}

// Helper function to show which environments are configured for an app
fn get_available_environments(app: &crate::config::models::App) -> String {
    let envs = app.commands.available_envs();

    if envs.is_empty() {
        "none".to_string()
    } else {
        envs.join(", ")
    }
}
