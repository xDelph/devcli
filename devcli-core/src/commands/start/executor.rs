//! Process spawning and management
//!
//! This module handles:
//! - Starting multiple apps in parallel
//! - Spawning individual app processes
//! - Process validation and tracking
//! - Internal app starting for dependencies

#![allow(deprecated)]

use super::resolver::{AppToStart, StartCommandArgs};
use crate::config::{
    dependencies::{check_dependencies_running, resolve_dependency_chain},
    load_config, load_preferences, resolve_app,
};
use crate::logging::FileLogger;
use crate::process::ProcessTracker;
use crate::utils::path::expand_path;
use crate::Result;
use anyhow::Context;

use std::sync::Arc;
use tokio::sync::Mutex;

/// Start all apps in parallel
///
/// Returns a vector of successfully started app names
#[tracing::instrument(skip(apps_to_start), fields(app_count = apps_to_start.len(), environment = %environment))]
pub async fn start_apps_in_parallel(
    apps_to_start: Vec<AppToStart>,
    environment: &str,
    silent: bool,
    stage_override: Option<String>,
) -> Result<Vec<String>> {
    tracing::info!(
        app_count = apps_to_start.len(),
        environment = %environment,
        "Starting apps in parallel"
    );

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
            )
            .await
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
            Ok(Err(e)) => errors.push(e),                    // App failed to start
            Err(e) => errors.push(format!("Task failed: {}", e)), // Task itself failed
        }
    }

    // Report results to the user (unless in silent mode)
    if !started_apps.is_empty() {
        tracing::info!(
            started_count = started_apps.len(),
            apps = ?started_apps,
            "Apps started successfully"
        );

        if show_output {
            println!(
                "\n✓ Successfully started {} app(s): {}",
                started_apps.len(),
                started_apps.join(", ")
            );
        }
    }

    if !errors.is_empty() {
        if started_apps.is_empty() {
            // All apps failed - this is a complete failure
            tracing::error!(
                error_count = errors.len(),
                errors = ?errors,
                "Failed to start all apps"
            );
            anyhow::bail!("Failed to start all apps:\n  {}", errors.join("\n  "));
        } else {
            // Some apps started, some failed - show warning but continue
            tracing::warn!(
                started_count = started_apps.len(),
                failed_count = errors.len(),
                errors = ?errors,
                "Some apps failed to start"
            );

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
#[tracing::instrument(
    skip(resolved_app, command, default_command),
    fields(
        app = %resolved_app.app_name,
        project = %resolved_app.project,
        environment = %environment,
        stage = ?stage_override
    )
)]
async fn start_single_app_process(
    resolved_app: crate::config::resolver::ResolvedApp,
    command: String,
    default_command: String,
    environment: String,
    show_output: bool,
    stage_override: Option<String>,
) -> Result<String> {
    let app_name = resolved_app.app_name.clone();

    tracing::debug!(
        app = %app_name,
        command = %command,
        "Starting single app process"
    );

    // Load preferences to get default_stage
    let preferences = load_preferences()?;

    // Determine which stage to use (override takes precedence over default_stages)
    let effective_stage = stage_override.or_else(|| {
        resolved_app
            .app
            .get_default_stage(&environment, preferences.default_stage.as_deref())
    });

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
        anyhow::bail!(
            "Working directory does not exist: {}",
            working_dir.display()
        );
    }

    // Step 2: Create a log file for this process
    let log_writer = Arc::new(Mutex::new(
        FileLogger::new(&resolved_app.project, &app_name, &environment, true).await?,
    ));

    // Get the log file path for display
    let log_path = {
        let writer = log_writer.lock().await;
        writer.log_path().clone()
    };

    // Step 3: Prepare command and environment variables
    let prepared = crate::commands::prepare::prepare_command(
        &command,
        &environment,
        &resolved_app,
        effective_stage.as_deref(),
        &working_dir,
        &preferences,
        show_output,
    )?;

    let final_command = prepared.final_command;
    let env_vars = prepared.env_vars;

    // Step 4: Display info to the user about what we're doing (unless silent)
    tracing::info!(
        app = %app_name,
        working_dir = %working_dir.display(),
        environment = %environment,
        command = %final_command,
        log_file = %log_path.display(),
        stage = ?effective_stage,
        "Starting app process"
    );

    if show_output {
        println!(
            "→ Starting '{}' in {} (environment: {})",
            app_name,
            working_dir.display(),
            environment
        );
        println!("  Command: {}", final_command);
        println!("  Log file: {}", log_path.display());
    }

    // Step 5: Prepare payload for internal-spawner
    use crate::commands::internal_spawner::SpawnerPayload;
    let payload = SpawnerPayload {
        app_name: app_name.clone(),
        command: final_command.clone(),
        working_dir: working_dir.clone(),
        env_vars,
        project: Some(resolved_app.project.clone()),
        environment: Some(environment.clone()),
        command_variant: Some(default_command.clone()),
        stage: effective_stage.clone(),
        log_file_path: log_path.clone(),
    };

    // Serialize and encode payload
    let payload_json = serde_json::to_vec(&payload)?;
    use base64::{engine::general_purpose, Engine as _};
    let payload_base64 = general_purpose::STANDARD.encode(&payload_json);

    // Get path to devcli binary
    let devcli_binary = crate::process::monitor::get_devcli_binary_path()?;

    // Spawn internal-spawner as a detached process
    let mut cmd = tokio::process::Command::new(&devcli_binary);
    cmd.arg("internal-spawner");
    cmd.arg("--payload");
    cmd.arg(&payload_base64);

    // Configure stdio based on show_output
    cmd.stdin(std::process::Stdio::null());
    if show_output {
        // Pipe stdout/stderr so we can capture and stream logs
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
    } else {
        // Detached mode: discard output
        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(std::process::Stdio::null());
    }

    #[cfg(unix)]
    {
        #[allow(unused_imports)]
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    // Step 6: Spawn the spawner process
    let mut spawner_child = cmd.spawn().context("Failed to spawn internal-spawner")?;
    let spawner_pid = spawner_child
        .id()
        .ok_or_else(|| anyhow::anyhow!("Failed to get spawner PID"))?;

    // Verify spawner is running
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let tracker = ProcessTracker::new()?;
    if !tracker.is_running(spawner_pid) {
        tracing::error!(
            app = %app_name,
            spawner_pid = spawner_pid,
            log_file = %log_path.display(),
            "Spawner failed to start"
        );
        anyhow::bail!(
            "Spawner for '{}' failed to start (PID: {}). Check the log file: {}",
            app_name,
            spawner_pid,
            log_path.display()
        );
    }

    tracing::info!(
        app = %app_name,
        spawner_pid = spawner_pid,
        "Spawner verified running"
    );

    // Step 7: If show_output is true, stream stdout/stderr from spawner
    // This keeps the command running so the TUI can capture logs
    if show_output {
        use tokio::io::{AsyncBufReadExt, BufReader};

        // Capture stdout
        if let Some(stdout) = spawner_child.stdout.take() {
            let app_name_clone = app_name.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    use colored::Colorize;
                    println!("[{}] {}", app_name_clone.cyan().bold(), line);
                }
            });
        }

        // Capture stderr
        if let Some(stderr) = spawner_child.stderr.take() {
            let app_name_clone = app_name.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    use colored::Colorize;
                    eprintln!("[{}] {}", app_name_clone.cyan().bold(), line);
                }
            });
        }

        // Wait for spawner to exit (which happens when app exits or is killed)
        // We spawn a task to wait for it so we don't block the main thread
        // This ensures the process is properly reaped (avoiding zombies)
        tokio::spawn(async move {
            let _ = spawner_child.wait().await;
        });
    }

    // Step 8: Success! Return the app name for reporting
    Ok(app_name)
}

/// Helper function for internal use (starting dependencies)
///
/// This is a simplified version of start_command that handles exactly one app.
/// Used when starting dependencies in parallel.
#[tracing::instrument(skip(args), fields(app = ?args.app_names.first(), project = ?args.project))]
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
            anyhow::anyhow!(
                "App '{}' does not have '{}' environment configured",
                app_name,
                environment
            )
        }
    })?;

    // Step 5: Get the default command name for this environment
    let default_command = resolved_app
        .app
        .defaults
        .get(&environment)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "App '{}' does not have a default command for '{}' environment",
                app_name,
                environment
            )
        })?
        .clone();

    // Step 6: Look up the actual command string from the commands HashMap
    let command = commands
        .get(&default_command)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Default command '{}' not found for app '{}'",
                default_command,
                app_name
            )
        })?
        .clone();

    // Step 7: Initialize process tracker and clean up stale PIDs
    let tracker = ProcessTracker::new()?;
    tracker.cleanup_dead()?;

    // Step 8: Check if this dependency is already running
    if let Some(existing) =
        tracker.get_process(&resolved_app.project, app_name, Some(&environment))?
    {
        if tracker.is_running(existing.pid) {
            // Already running - no need to start again
            return Ok(());
        } else {
            // Stale PID file - clean it up
            tracker.remove_process(&resolved_app.project, app_name, Some(&environment))?;
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
    )
    .await?;

    // Step 11: Ensure background monitor is running
    let binary_path = crate::process::monitor::get_devcli_binary_path()?;
    let _ = crate::process::monitor::spawn_monitor_if_needed(&binary_path);

    Ok(())
}
