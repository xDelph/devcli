//! Process spawning and management
//! 
//! This module handles:
//! - Starting multiple apps in parallel
//! - Spawning individual app processes
//! - Process validation and tracking
//! - Internal app starting for dependencies

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
use super::resolver::{AppToStart, StartCommandArgs};



/// Start all apps in parallel
/// 
/// Returns a vector of successfully started app names
pub async fn start_apps_in_parallel(
    apps_to_start: Vec<AppToStart>,
    environment: &str,
    silent: bool,
    stage_override: Option<String>,
) -> Result<Vec<String>> {
    if !silent {
        println!("\nStarting {} app(s) in parallel...", apps_to_start.len());
    }
    
    let preferences = load_preferences()?;
    let show_output = !silent && !preferences.detached_mode;
    
    let mut tasks = Vec::new();
    
    // Create an async task for each app to start
    for app_info in apps_to_start {
        let app_name = app_info.resolved_app.app_name.clone();
        let environment = environment.to_string();
        let stage = stage_override.clone();
        
        let task = tokio::spawn(async move {
            start_single_app_process(
                app_info.resolved_app,
                app_info.command,
                app_info.default_command,
                environment,
                show_output,
                stage,
            ).await
                .map_err(|e| format!("{}: {}", app_name, e))
        });
        
        tasks.push(task);
    }
    
    // Wait for all apps to start (or fail)
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
    
    // Report results to the user (unless in silent mode)
    if !started_apps.is_empty() && show_output {
        println!("\n✓ Successfully started {} app(s): {}", started_apps.len(), started_apps.join(", "));
    }
    
    if !errors.is_empty() {
        if started_apps.is_empty() {
            // All apps failed - this is a complete failure
            anyhow::bail!("Failed to start all apps:\n  {}", errors.join("\n  "));
        } else {
            // Some apps started, some failed - show warning but continue
            if show_output {
                println!("\n⚠ Some apps failed to start:\n  {}", errors.join("\n  "));
            }
        }
    }
    
    Ok(started_apps)
}

/// Helper function to start a single app process
/// 
/// This is used by the parallel app starting logic.
/// Returns the app name on success for reporting.
async fn start_single_app_process(
    resolved_app: crate::config::resolver::ResolvedApp,
    command: String,
    default_command: String,
    environment: String,
    show_output: bool,
    stage_override: Option<String>,
) -> Result<String> {
    let app_name = resolved_app.app_name.clone();
    
    // Determine which stage to use (override takes precedence over config)
    let effective_stage = stage_override
        .or_else(|| resolved_app.app.stage.clone());
    
    // Validate stage if present
    if let Some(ref stage) = effective_stage {
        use crate::config::models::Stage;
        if Stage::from_string(stage).is_none() {
            anyhow::bail!(
                "Invalid stage '{}'. Must be one of: {}",
                stage,
                Stage::all_names()
            );
        }
    }
    
    // Step 1: Expand the working directory path
    let working_dir = expand_path(&resolved_app.app.path);
    
    // Verify the directory actually exists
    if !working_dir.exists() {
        anyhow::bail!("Working directory does not exist: {}", working_dir.display());
    }
    
    // Step 2: Create a log file for this process
    let log_writer = Arc::new(Mutex::new(FileLogger::new(&app_name).await?));
    
    // Get the log file path for display
    let log_path = {
        let writer = log_writer.lock().await;
        writer.log_path().clone()
    };
    
    // Step 3: Build environment variables - inherit parent environment and set Docker context
    // Start with the current process's environment to inherit PATH, HOME, Docker config, etc.
    let mut env_vars: HashMap<String, String> = std::env::vars().collect();
    
    // Step 3a: Inject --platform flag for docker/orbstack commands
    let preferences = load_preferences()?;
    let mut final_command = match environment.as_str() {
        "docker" | "orbstack" => {
            crate::utils::command::inject_docker_platform(&command, &preferences.docker_platform)
        }
        _ => command.clone()
    };
    
    // Override/add specific variables based on environment
    match environment.as_str() {
        "docker" => {
            env_vars.insert("DOCKER_CONTEXT".to_string(), "default".to_string());
            
            // For Docker, use --env-file flag if .env exists
            // Priority: Stage-specific file > base .env (using dockerfile_path from config)
            if let Ok(Some(env_file_path)) = crate::detection::find_env_file(
                &working_dir,
                resolved_app.app.dockerfile_path.as_deref(),
                effective_stage.as_deref()
            ) {
                final_command = crate::utils::command::inject_docker_env_file(
                    &final_command,
                    &env_file_path
                );
                
                // Log which env file is being used
                if show_output {
                    println!("  Using env file: {}", env_file_path);
                }
            }
            
            // Inject dockerfile path for build commands
            if let Some(ref dockerfile_path) = resolved_app.app.dockerfile_path {
                final_command = crate::utils::command::inject_dockerfile_path(&final_command, dockerfile_path);
            }
        }
        "orbstack" => {
            env_vars.insert("DOCKER_CONTEXT".to_string(), "orbstack".to_string());
            
            // For OrbStack, use --env-file flag (same as Docker)
            // Priority: Stage-specific file > base .env (using dockerfile_path from config)
            if let Ok(Some(env_file_path)) = crate::detection::find_env_file(
                &working_dir,
                resolved_app.app.dockerfile_path.as_deref(),
                effective_stage.as_deref()
            ) {
                final_command = crate::utils::command::inject_docker_env_file(
                    &final_command,
                    &env_file_path
                );
                
                // Log which env file is being used
                if show_output {
                    println!("  Using env file: {}", env_file_path);
                }
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
        app_name: app_name.clone(),
        working_dir: working_dir.clone(),
        command: final_command.clone(),
        env_vars,
        detached: true,
        show_output,
    };
    
    // Step 4: Display info to the user about what we're doing (unless silent)
    if show_output {
        println!("→ Starting '{}' in {} (environment: {})", app_name, working_dir.display(), environment);
        println!("  Command: {}", final_command);
        println!("  Log file: {}", log_path.display());
    }
    
    // Step 5: Actually spawn the process!
    let spawned = spawn_process(options, log_writer.clone(), None).await?;
    
    // Step 6: Wait a moment and verify the process didn't immediately crash
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
    let process_info = ProcessInfo {
        app_name: app_name.clone(),
        pid: spawned.pid,
        command: final_command,
        working_dir: working_dir.to_string_lossy().to_string(),
        start_time: Utc::now(),
        env_vars: HashMap::new(),
        project: Some(resolved_app.project.clone()),
        app_config_name: Some(resolved_app.app_name.clone()),
        environment: Some(environment),
        command_variant: Some(default_command),
        stage: effective_stage,
    };
    
    // Step 8: Save the process info to a PID file
    tracker.register_process(process_info)?;
    
    // Step 9: Success! Return the app name for reporting
    Ok(app_name)
}

/// Helper function for internal use (starting dependencies)
/// 
/// This is a simplified version of start_command that handles exactly one app.
/// Used when starting dependencies in parallel.
pub async fn start_single_app_internal(args: StartCommandArgs, show_output: bool) -> Result<()> {
    // Validate that we have exactly one app name (this is for dependencies)
    if args.app_names.len() != 1 {
        anyhow::bail!("start_single_app_internal expects exactly one app name");
    }
    
    let app_name = &args.app_names[0];
    
    // Step 1: Load configuration files
    let config = load_config()?;
    let preferences = load_preferences()?;
    
    // Step 2: Resolve which app to start
    let resolved_app = resolve_app(&config, app_name, args.project.as_deref())?;
    
    // Step 3: Determine which environment to use
    let environment = args
        .env
        .clone()
        .unwrap_or_else(|| preferences.default_env.clone());
    
    // Step 4: Get the commands HashMap for the selected environment
    let commands = resolved_app.app.commands.get(&environment).ok_or_else(|| {
        use crate::config::models::Environment;
        if Environment::from_string(&environment).is_none() {
            anyhow::anyhow!("Invalid environment '{}'", environment)
        } else {
            anyhow::anyhow!("App '{}' does not have '{}' environment configured", app_name, environment)
        }
    })?;
    
    // Step 5: Get the default command name for this environment
    let default_command = resolved_app.app.defaults.get(&environment).ok_or_else(|| {
        anyhow::anyhow!("App '{}' does not have a default command for '{}' environment", app_name, environment)
    })?.clone();
    
    // Step 6: Look up the actual command string from the commands HashMap
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
    if !args.skip_deps {
        let dependencies = resolve_dependency_chain(&config, &resolved_app)?;
        if !dependencies.is_empty() {
            let missing = check_dependencies_running(&tracker, &dependencies)?;
            if !missing.is_empty() {
                anyhow::bail!("Missing dependencies: {}", missing.join(", "));
            }
        }
    }
    
    // Step 10: Start the single app using our helper function
    start_single_app_process(
        resolved_app,
        command,
        default_command,
        environment,
        show_output,
        args.stage,
    ).await?;
    
    // Step 11: Ensure background monitor is running
    let binary_path = crate::process::monitor::get_rustycli_binary_path()?;
    let _ = crate::process::monitor::spawn_monitor_if_needed(&binary_path);
    
    Ok(())
}