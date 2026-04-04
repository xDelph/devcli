//! Process spawning and management
//!
//! This module handles:
//! - Starting multiple apps in parallel
//! - Spawning individual app processes through process-manager
//! - Persisting process metadata and ensuring monitor daemon is running
//! - Internal app starting for dependencies

use super::resolver::{AppToStart, StartCommandArgs};
use crate::config::{
    dependencies::{check_dependencies_running, resolve_dependency_chain},
    load_config, load_preferences, resolve_app,
};
use crate::process_manager_support::{
    emit_line, find_process, log_file_path, process_id, state_store, stream_output, OutputChannel,
};
use crate::utils::path::expand_path;
use crate::Result;
use process_manager::state::{ManagedProcess, ProcessRuntime};
use process_manager::{engine, HealthCheck, RestartPolicy, Task};
use std::collections::HashMap;

/// Start all apps in parallel
///
/// Returns a vector of successfully started app names
#[tracing::instrument(skip(apps_to_start, output_tx), fields(app_count = apps_to_start.len(), environment = %environment))]
pub async fn start_apps_in_parallel(
    apps_to_start: Vec<AppToStart>,
    environment: &str,
    silent: bool,
    stage_override: Option<String>,
    output_tx: Option<OutputChannel>,
) -> Result<Vec<String>> {
    tracing::info!(
        app_count = apps_to_start.len(),
        environment = %environment,
        "Starting apps in parallel"
    );

    emit_line(
        silent,
        output_tx.as_ref(),
        format!("\nStarting {} app(s) in parallel...", apps_to_start.len()),
    );

    let preferences = load_preferences()?;
    let show_output = !silent && !preferences.detached_mode;

    let mut tasks = Vec::new();

    // Create an async task for each app to start
    for app_info in apps_to_start {
        let app_name = app_info.resolved_app.app_name.clone();
        let environment = environment.to_string();
        let stage = stage_override.clone();
        let output_tx = output_tx.clone();

        let task = tokio::spawn(async move {
            start_single_app_process(
                app_info.resolved_app,
                app_info.command,
                app_info.default_command,
                environment,
                show_output,
                stage,
                output_tx,
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

        emit_line(
            !show_output,
            output_tx.as_ref(),
            format!(
                "\n✓ Successfully started {} app(s): {}",
                started_apps.len(),
                started_apps.join(", ")
            ),
        );
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

            emit_line(
                !show_output,
                output_tx.as_ref(),
                format!("\n⚠ Some apps failed to start:\n  {}", errors.join("\n  ")),
            );
        }
    }

    Ok(started_apps)
}

/// Helper function to start a single app process
///
/// This is used by the parallel app starting logic.
/// Returns the app name on success for reporting.
#[tracing::instrument(
    skip(resolved_app, command, default_command, output_tx),
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
    output_tx: Option<OutputChannel>,
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

    // Expand the working directory path
    let working_dir = expand_path(&resolved_app.app.path);

    // Verify the directory actually exists
    if !working_dir.exists() {
        anyhow::bail!(
            "Working directory does not exist: {}",
            working_dir.display()
        );
    }

    // Create log file path for this process
    let log_path = log_file_path(&resolved_app.project, &app_name, &environment)?;

    // Prepare command and environment variables
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

    tracing::info!(
        app = %app_name,
        working_dir = %working_dir.display(),
        environment = %environment,
        command = %final_command,
        log_file = %log_path.display(),
        stage = ?effective_stage,
        "Starting app process"
    );

    emit_line(
        !show_output,
        output_tx.as_ref(),
        format!(
            "→ Starting '{}' in {} (environment: {})",
            app_name,
            working_dir.display(),
            environment
        ),
    );
    emit_line(
        !show_output,
        output_tx.as_ref(),
        format!("  Command: {}", final_command),
    );
    emit_line(
        !show_output,
        output_tx.as_ref(),
        format!("  Log file: {}", log_path.display()),
    );

    let id = process_id(&resolved_app.project, &app_name, &environment);
    let task = Task {
        id: id.clone(),
        command: final_command,
        args: vec![],
        working_dir: working_dir.clone(),
        env: env_vars,
        is_detached: true,
        log_file: Some(log_path),
        health_check: to_pm_health_check(resolved_app.app.health_check.as_ref()),
        restart_policy: to_pm_restart_policy(resolved_app.app.restart_policy.as_ref()),
    };

    // Spawn through process-manager
    let running = engine::spawn(&task).await?;
    if !running.is_alive() {
        anyhow::bail!("Failed to start process '{}'", app_name);
    }

    let process_manager::engine::RunningProcess {
        pid,
        pgid,
        child: _,
        output_rx,
    } = running;

    // Persist managed process metadata
    let mut metadata = HashMap::new();
    metadata.insert("project".to_string(), resolved_app.project.clone());
    metadata.insert("app_config_name".to_string(), app_name.clone());
    metadata.insert("environment".to_string(), environment.clone());
    metadata.insert("command_variant".to_string(), default_command);
    if let Some(stage) = &effective_stage {
        metadata.insert("stage".to_string(), stage.clone());
    }
    if let Some(alt) = &resolved_app.app.alternative_name {
        metadata.insert("alternative_name".to_string(), alt.clone());
    }

    let managed = ManagedProcess {
        id,
        pid,
        pgid,
        task,
        start_time: chrono::Utc::now(),
        metadata,
        runtime: ProcessRuntime::default(),
    };

    let store = state_store()?;
    store.save(&managed)?;
    store.ensure_daemon_running()?;

    // Stream output to terminal and/or TUI if requested
    if show_output || output_tx.is_some() {
        let display_name = resolved_app
            .app
            .alternative_name
            .clone()
            .unwrap_or_else(|| app_name.clone());
        let output_tx_clone = output_tx.clone();
        tokio::spawn(async move {
            stream_output(output_rx, show_output, &display_name, output_tx_clone.as_ref()).await;
        });
    } else {
        drop(output_rx);
    }

    Ok(app_name)
}

fn to_pm_health_check(health: Option<&crate::config::models::HealthCheck>) -> HealthCheck {
    match health {
        Some(crate::config::models::HealthCheck::Http {
            url,
            timeout_secs,
            expected_status,
        }) => HealthCheck::Http {
            url: url.clone(),
            timeout_secs: *timeout_secs,
            expected_status: *expected_status,
        },
        Some(crate::config::models::HealthCheck::Tcp {
            host,
            port,
            timeout_secs,
        }) => HealthCheck::Tcp {
            host: host.clone(),
            port: *port,
            timeout_secs: *timeout_secs,
        },
        Some(crate::config::models::HealthCheck::Command {
            command,
            timeout_secs,
            expected_exit_code,
        }) => HealthCheck::Command {
            command: command.clone(),
            timeout_secs: *timeout_secs,
            expected_exit_code: *expected_exit_code,
        },
        Some(crate::config::models::HealthCheck::Process {}) | None => HealthCheck::Process {},
    }
}

fn to_pm_restart_policy(policy: Option<&crate::config::models::RestartPolicy>) -> RestartPolicy {
    match policy {
        Some(policy) => RestartPolicy {
            enabled: policy.enabled,
            max_restarts: policy.max_restarts,
            restart_window_secs: policy.restart_window_secs,
            initial_backoff_secs: policy.initial_backoff_secs,
            max_backoff_secs: policy.max_backoff_secs,
            backoff_multiplier: policy.backoff_multiplier,
            restart_on_exit_codes: policy.restart_on_exit_codes.clone(),
        },
        None => RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    }
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

    // Load configuration files
    let config = load_config()?;
    let preferences = load_preferences()?;

    // Resolve which app to start
    let resolved_app = resolve_app(&config, app_name, args.project.as_deref())?;

    // Determine which environment to use
    let environment = args
        .env
        .clone()
        .unwrap_or_else(|| preferences.default_env.clone());

    // Get the commands HashMap for the selected environment
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

    // Get the default command name for this environment
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

    // Look up the actual command string from the commands HashMap
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

    // Initialize state store and clean stale entries
    let store = state_store()?;
    store.cleanup_dead()?;

    // Check if this dependency is already running
    if let Some(existing) = find_process(&store, &resolved_app.project, app_name, Some(&environment))? {
        if store.is_running(&existing) {
            // Already running - no need to start again
            return Ok(());
        } else {
            // Stale state file - clean it up
            store.delete(&existing.id)?;
        }
    }

    // Check if this dependency has its own dependencies
    if !args.skip_deps {
        let dependencies = resolve_dependency_chain(&config, &resolved_app)?;
        if !dependencies.is_empty() {
            let missing = check_dependencies_running(&store, &dependencies)?;
            if !missing.is_empty() {
                anyhow::bail!("Missing dependencies: {}", missing.join(", "));
            }
        }
    }

    // Start the single app using our helper function
    start_single_app_process(
        resolved_app,
        command,
        default_command,
        environment,
        show_output,
        args.stage,
        args.output_tx,
    )
    .await?;

    Ok(())
}

