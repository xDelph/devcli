// Run command - Execute specific command variants for apps
// Similar to start, but lets you choose which command to run
//
// Example: devcli run api-private build:production
// This runs the "build:production" command instead of the default

use crate::config::{load_config, load_preferences, resolve_app};
use crate::process_manager_support::{
    emit_line, find_process, log_file_path, process_id, state_store, stream_output, OutputChannel,
};
use crate::utils::path::expand_path;
use crate::Result;
use process_manager::state::{ManagedProcess, ProcessRuntime};
use process_manager::{engine, HealthCheck, RestartPolicy, Task};
use std::collections::HashMap;

// Arguments for the run command
pub struct RunCommandArgs {
    pub app_name: String,        // Name of the app from config
    pub command_variant: String, // Which command to run (e.g., "build:production", "test")
    pub project: Option<String>, // Optional: specify project if ambiguous
    pub env: Option<String>,     // Optional: environment override
    pub skip_deps: bool,         // If true, skip dependency checks
    pub silent: bool,            // If true, don't print to terminal (for TUI mode)
    pub output_tx: Option<OutputChannel>, // Optional output stream (for TUI popup)
}

// Main implementation of the run command
// Very similar to start_command, but uses a specific command variant instead of default
#[tracing::instrument(skip(args), fields(app_name = %args.app_name, command_variant = %args.command_variant, project = ?args.project, env = ?args.env))]
pub async fn run_command(args: RunCommandArgs) -> Result<()> {
    tracing::info!(
        app_name = %args.app_name,
        command_variant = %args.command_variant,
        project = ?args.project,
        "Run command initiated"
    );

    // Load config and preferences (same as start command)
    let config = load_config()?;
    let preferences = load_preferences()?;
    let silent = args.silent;
    let show_output = !silent && !preferences.detached_mode;

    // Resolve the app in the config
    let resolved_app = resolve_app(&config, &args.app_name, args.project.as_deref())?;

    // Determine environment (same priority as start)
    let environment = args
        .env
        .clone()
        .unwrap_or_else(|| preferences.default_env.clone());

    // Get the commands for the chosen environment
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

    // Look up the specific command variant
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

    let store = state_store()?;
    store.cleanup_dead()?;

    // Check if this app/environment is already running
    if let Some(process) = find_process(
        &store,
        &resolved_app.project,
        &resolved_app.app_name,
        Some(&environment),
    )? {
        if store.is_running(&process) {
            anyhow::bail!(
                "Process '{}:{}' is already running with PID {}",
                args.app_name,
                args.command_variant,
                process.pid
            );
        } else {
            store.delete(&process.id)?;
        }
    }

    // Dependency checking
    if !args.skip_deps {
        use crate::commands::start::handle_dependencies;
        handle_dependencies(&[&resolved_app], &environment, silent).await?;
    }

    // Expand path and verify it exists
    let working_dir = expand_path(&resolved_app.app.path);
    if !working_dir.exists() {
        anyhow::bail!(
            "Working directory does not exist: {}",
            working_dir.display()
        );
    }

    // Create log file path
    let log_path = log_file_path(&resolved_app.project, &args.app_name, &environment)?;

    // Determine which stage to use for env file resolution
    let stage = resolved_app
        .app
        .get_default_stage(&environment, preferences.default_stage.as_deref());

    // Prepare command and environment variables
    let prepared = crate::commands::prepare::prepare_command(
        &command,
        &environment,
        &resolved_app,
        stage.as_deref(),
        &working_dir,
        &preferences,
        show_output,
    )?;

    let final_command = prepared.final_command;
    let env_vars = prepared.env_vars;

    tracing::info!(
        app_name = %args.app_name,
        command_variant = %args.command_variant,
        environment = %environment,
        working_dir = %working_dir.display(),
        command = %final_command,
        log_file = %log_path.display(),
        "Running command"
    );

    emit_line(
        silent,
        args.output_tx.as_ref(),
        format!(
            "Running command '{}' for app '{}' (environment: {})",
            args.command_variant, args.app_name, environment
        ),
    );
    emit_line(
        silent,
        args.output_tx.as_ref(),
        format!("Working directory: {}", working_dir.display()),
    );
    emit_line(
        silent,
        args.output_tx.as_ref(),
        format!("Command: {}", final_command),
    );
    emit_line(
        silent,
        args.output_tx.as_ref(),
        format!("Log file: {}", log_path.display()),
    );

    // Build process-manager task
    let id = process_id(&resolved_app.project, &resolved_app.app_name, &environment);
    let task = Task {
        id: id.clone(),
        command: final_command.clone(),
        args: vec![],
        working_dir: working_dir.clone(),
        env: env_vars,
        is_detached: true,
        log_file: Some(log_path),
        health_check: to_pm_health_check(resolved_app.app.health_check.as_ref()),
        restart_policy: to_pm_restart_policy(resolved_app.app.restart_policy.as_ref()),
    };

    // Spawn process via process-manager
    let running = engine::spawn(&task).await?;
    let pid = running.pid;
    let pgid = running.pgid;
    let output_rx = running.output_rx;

    if show_output || args.output_tx.is_some() {
        let display_name = resolved_app
            .app
            .alternative_name
            .clone()
            .unwrap_or_else(|| args.app_name.clone());
        let output_tx = args.output_tx.clone();
        tokio::spawn(async move {
            stream_output(output_rx, show_output, &display_name, output_tx.as_ref()).await;
        });
    } else {
        drop(output_rx);
    }

    // Wait a moment to check if the process completed or crashed
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

    if !is_pid_running(pid) {
        emit_line(
            silent,
            args.output_tx.as_ref(),
            format!(
                "✓ Process '{}:{}' completed (PID: {}). Check log for details.",
                args.app_name, args.command_variant, pid
            ),
        );
        return Ok(());
    }

    // Save process metadata
    let mut metadata = HashMap::new();
    metadata.insert("project".to_string(), resolved_app.project.clone());
    metadata.insert("app_config_name".to_string(), resolved_app.app_name.clone());
    metadata.insert("environment".to_string(), environment.clone());
    metadata.insert("command_variant".to_string(), args.command_variant.clone());
    if let Some(stage_name) = &stage {
        metadata.insert("stage".to_string(), stage_name.clone());
    }
    if let Some(alt) = &resolved_app.app.alternative_name {
        metadata.insert("alternative_name".to_string(), alt.clone());
    }

    let process_info = ManagedProcess {
        id,
        pid,
        pgid,
        task,
        start_time: chrono::Utc::now(),
        metadata,
        runtime: ProcessRuntime::default(),
    };

    store.save(&process_info)?;
    store.ensure_daemon_running()?;

    emit_line(
        silent,
        args.output_tx.as_ref(),
        format!(
            "✓ Process '{}:{}' started successfully with PID {}",
            args.app_name, args.command_variant, pid
        ),
    );

    // Non-detached mode: keep viewing until Ctrl+C or process exits
    if !preferences.detached_mode {
        emit_line(
            silent,
            args.output_tx.as_ref(),
            "\nRunning in background with output streaming (use Ctrl+C to stop viewing)".to_string(),
        );
        emit_line(
            silent,
            args.output_tx.as_ref(),
            "Press Ctrl+C to stop viewing logs (process will continue running)...\n".to_string(),
        );

        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    emit_line(
                        silent,
                        args.output_tx.as_ref(),
                        "\n\nStopped viewing logs. Process is still running in the background.".to_string(),
                    );
                    emit_line(
                        silent,
                        args.output_tx.as_ref(),
                        "Use 'devcli status' to check process status.".to_string(),
                    );
                    break;
                }
                _ = interval.tick() => {
                    if let Some(latest) = store.load(&process_info.id)? {
                        if !store.is_running(&latest) {
                            emit_line(
                                silent,
                                args.output_tx.as_ref(),
                                "\n\nProcess exited.".to_string(),
                            );
                            let _ = store.delete(&process_info.id);
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    } else {
        emit_line(
            silent,
            args.output_tx.as_ref(),
            "\nRunning in background (detached mode, no terminal output)".to_string(),
        );
    }

    Ok(())
}

fn is_pid_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        false
    }
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

