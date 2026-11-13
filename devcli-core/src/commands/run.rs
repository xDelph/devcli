// Run command - Execute specific command variants for apps
// Similar to start, but lets you choose which command to run
//
// Example: devcli run api-private build:production
// This runs the "build:production" command instead of the default

use crate::config::{load_config, load_preferences};
use crate::process_manager_support::{
    emit_line, find_process, log_file_path, process_id, state_store, stream_output,
    to_pm_restart_policy, OutputChannel,
};
use crate::utils::path::expand_path;
use crate::Result;
use process_manager::state::SpawnOutcome;
use process_manager::{engine, Task};
use std::collections::HashMap;

// Arguments for the run command
pub struct RunCommandArgs {
    pub app_name: String,                 // Name of the app from config
    pub command_variant: String,          // Which command to run (e.g., "build:production", "test")
    pub project: Option<String>,          // Optional: specify project if ambiguous
    pub env: Option<String>,              // Optional: environment override
    pub skip_deps: bool,                  // If true, skip dependency checks
    pub silent: bool,                     // If true, don't print to terminal (for TUI mode)
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
    let show_output = crate::output::human(silent) && !preferences.detached_mode;

    // Resolve the app in the config (or the git worktree the command runs in)
    let resolved_app = crate::worktrees::resolve(
        &config,
        &args.app_name,
        args.project.as_deref(),
        crate::worktrees::Mode::FromCwd,
    )?;

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

    // Check if this app/environment is already running (checked again under
    // the entry lock when the process is spawned). A dead entry left is
    // replaced when the new process is recorded.
    if let Some(process) = find_process(
        &store,
        &resolved_app.project,
        &resolved_app.app_name,
        Some(&environment),
    )? && store.is_running(&process)
    {
        anyhow::bail!(
            "Process '{}:{}' is already running with PID {}",
            args.app_name,
            args.command_variant,
            process.pid
        );
    }

    // Dependency checking
    if !args.skip_deps {
        use crate::commands::start::handle_dependencies;
        // `run` returns right away, so nothing owns the dependencies it starts.
        handle_dependencies(&[&resolved_app], &environment, silent, None).await?;
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
    // The real app key, not the alias the user typed: `logs` and the TUI look
    // log files up by app key.
    let log_path = log_file_path(&resolved_app.project, &resolved_app.app_name, &environment)?;

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

    let health_check = prepared.health_check(&resolved_app);
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
        health_check,
        restart_policy: to_pm_restart_policy(resolved_app.app.restart_policy.as_ref()),
    };
    let log_file = task.log_file.clone();

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
    crate::worktrees::record(&mut metadata, &resolved_app);

    // Listen for Ctrl+C before anything runs: the process is detached (its
    // own session), so an interrupt with the default action killed devcli
    // and left the process behind, untracked until the save below.
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    // Spawn and record in one step: the process is tracked from the start
    // (it used to be saved only after the 2s wait below), and a concurrent
    // `run`/`start` of the same app cannot spawn a second copy.
    let (pid, output_rx) = match store.spawn_unless_running(task, metadata).await? {
        SpawnOutcome::Started { process, output_rx } => (process.pid, output_rx),
        SpawnOutcome::AlreadyRunning(existing) => anyhow::bail!(
            "Process '{}:{}' is already running with PID {}",
            args.app_name,
            args.command_variant,
            existing.pid
        ),
    };

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

    let json_result = |running: bool| -> Result<()> {
        if crate::output::json_enabled() && !silent {
            crate::output::print_json(&serde_json::json!({
                "project": resolved_app.project,
                "app": resolved_app.app_name,
                "environment": environment,
                "command_variant": args.command_variant,
                "pid": pid,
                "running": running,
                "log_file": log_file,
            }))?;
        }
        Ok(())
    };

    // Wait a moment to check if the process completed or crashed
    let interrupted = tokio::select! {
        _ = tokio::time::sleep(tokio::time::Duration::from_millis(2000)) => false,
        _ = interrupt.recv() => true,
    };

    if !engine::pid_alive(pid) {
        // A one-shot command: nothing left to track.
        store.forget(&id, pid)?;
        emit_line(
            silent,
            args.output_tx.as_ref(),
            format!(
                "✓ Process '{}:{}' completed (PID: {}). Check log for details.",
                args.app_name, args.command_variant, pid
            ),
        );
        json_result(false)?;
        return Ok(());
    }

    store.ensure_daemon_running()?;

    emit_line(
        silent,
        args.output_tx.as_ref(),
        format!(
            "✓ Process '{}:{}' started successfully with PID {}",
            args.app_name, args.command_variant, pid
        ),
    );

    json_result(true)?;

    if interrupted {
        emit_line(
            silent,
            args.output_tx.as_ref(),
            "Process is running in the background. Use 'devcli status' to check process status."
                .to_string(),
        );
        return Ok(());
    }

    // Non-detached mode: keep viewing until Ctrl+C or process exits. `--json`
    // implies detached, like `start`: an agent waiting on this would hang.
    if !preferences.detached_mode
        && !crate::output::json_enabled()
        && crate::utils::agent::detect().is_none()
    {
        emit_line(
            silent,
            args.output_tx.as_ref(),
            "\nRunning in background with output streaming (use Ctrl+C to stop viewing)"
                .to_string(),
        );
        emit_line(
            silent,
            args.output_tx.as_ref(),
            "Press Ctrl+C to stop viewing logs (process will continue running)...\n".to_string(),
        );

        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        loop {
            tokio::select! {
                _ = interrupt.recv() => {
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
                    if let Some(latest) = store.load(&id)? {
                        if !store.is_running(&latest) {
                            // pm-daemon restarts it shortly: keep its entry.
                            let restarting = store.awaiting_restart(&latest);
                            emit_line(
                                silent,
                                args.output_tx.as_ref(),
                                if restarting {
                                    "\n\nProcess exited; pm-daemon will restart it.".to_string()
                                } else {
                                    "\n\nProcess exited.".to_string()
                                },
                            );
                            if !restarting {
                                let _ = store.forget(&id, latest.pid);
                            }
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
