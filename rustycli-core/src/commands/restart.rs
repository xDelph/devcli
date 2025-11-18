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
        "orbstack" => resolved_app.app.commands.orbstack.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' does not have 'orbstack' environment configured. Available: {}",
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
            "Invalid environment '{}'. Must be 'local', 'docker', 'orbstack', or 'k8s'.",
            environment
        ),
    };

    // Step 9: Get the default command name for this environment
    // Use the same command that was running before (from the existing process)
    let default_command = if let Some(existing_variant) = &process.command_variant {
        // Use the same command variant that was running
        existing_variant.clone()
    } else {
        // Fallback to default for this environment
        match environment.as_str() {
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
            "orbstack" => resolved_app.app.defaults.orbstack.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "App '{}' does not have a default command for 'orbstack' environment",
                    args.app_name
                )
            })?,
            "k8s" => resolved_app.app.defaults.k8s.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "App '{}' does not have a default command for 'k8s' environment",
                    args.app_name
                )
            })?,
            _ => unreachable!(),
        }
        .clone()
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

    // Step 14: Build the options struct for spawning the process
    // Use the same configuration as before
    let options = ProcessOptions {
        app_name: args.app_name.clone(),
        working_dir: working_dir.clone(),
        command: command.clone(),
        env_vars: process.env_vars.clone(), // Use the same environment variables
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
        println!("Command: {}", command);
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
        command,
        working_dir: working_dir.to_string_lossy().to_string(),
        start_time: Utc::now(),
        env_vars: process.env_vars.clone(),
        project: Some(resolved_app.project.clone()),
        app_config_name: Some(resolved_app.app_name.clone()),
        environment: Some(environment),
        command_variant: Some(default_command),
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
    use crate::config::models::Environment;
    
    let mut envs = Vec::new();

    for env_type in Environment::all() {
        if app.commands.get(env_type.as_str()).is_some() {
            envs.push(env_type.as_str());
        }
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
