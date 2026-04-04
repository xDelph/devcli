use anyhow::Result;
use tokio::sync::mpsc;

use crate::tui::app::{CommandRequest, CommandResult, CommandType};

/// Executes a command asynchronously in a background task
/// This runs in a separate tokio task to avoid blocking the UI
/// Uses shared core commands (same code path as CLI)
pub(crate) async fn execute_command_async(
    request: CommandRequest,
    output_tx: mpsc::UnboundedSender<CommandResult>,
) -> Result<String> {
    crate::debug!("execute_command_async: Starting command");

    // Execute based on command type
    match request.command_type {
        CommandType::Start => execute_start_command(request, output_tx).await,
        CommandType::Run => execute_run_command(request, output_tx).await,
        CommandType::Stop => execute_stop_command(request, output_tx).await,
        CommandType::Restart => execute_restart_command(request, output_tx).await,
    }
}

async fn execute_start_command(
    request: CommandRequest,
    output_tx: mpsc::UnboundedSender<CommandResult>,
) -> Result<String> {
    let core_output = spawn_output_bridge(output_tx.clone());
    let args = crate::commands::start::StartCommandArgs {
        app_names: vec![request.app_name.clone()],
        project: Some(request.project),
        env: Some(request.environment),
        skip_deps: false,
        silent: true,
        stage: None,
        output_tx: Some(core_output),
    };
    crate::commands::start::start_command(args).await?;

    crate::debug!("execute_command_async: Start command finished");
    Ok(format!("Successfully started {}", request.app_name))
}

async fn execute_run_command(
    request: CommandRequest,
    output_tx: mpsc::UnboundedSender<CommandResult>,
) -> Result<String> {
    // Parse environment:command format
    // The environment field contains "env:command" (e.g., "orbstack:run")
    let (env, command_name) = if let Some((e, c)) = request.environment.split_once(':') {
        (e, c)
    } else {
        // Fallback if format is wrong - treat whole string as command
        ("local", request.environment.as_str())
    };

    let core_output = spawn_output_bridge(output_tx.clone());
    let args = crate::commands::run::RunCommandArgs {
        app_name: request.app_name.clone(),
        command_variant: command_name.to_string(),
        project: Some(request.project),
        env: Some(env.to_string()),
        skip_deps: false,
        silent: true,
        output_tx: Some(core_output),
    };
    crate::commands::run::run_command(args).await?;

    crate::debug!("execute_command_async: Run command finished");
    Ok(format!("Successfully ran command for {}", request.app_name))
}

async fn execute_stop_command(
    request: CommandRequest,
    output_tx: mpsc::UnboundedSender<CommandResult>,
) -> Result<String> {
    let core_output = spawn_output_bridge(output_tx.clone());
    let args = crate::commands::stop::StopCommandArgs {
        app_name: Some(request.app_name.clone()),
        project: Some(request.project),
        all: false,
        force: false,
        silent: true,
        output_tx: Some(core_output),
    };
    crate::commands::stop::stop_command(args).await?;

    crate::debug!("execute_command_async: Stop command finished");
    Ok(format!("Successfully stopped {}", request.app_name))
}

async fn execute_restart_command(
    request: CommandRequest,
    output_tx: mpsc::UnboundedSender<CommandResult>,
) -> Result<String> {
    let core_output = spawn_output_bridge(output_tx.clone());
    let args = crate::commands::restart::RestartCommandArgs {
        app_name: request.app_name.clone(),
        project: Some(request.project),
        env: if request.environment.is_empty() {
            None
        } else {
            Some(request.environment)
        },
        skip_deps: false,
        silent: true,
        output_tx: Some(core_output),
    };
    crate::commands::restart::restart_command(args).await?;

    crate::debug!("execute_command_async: Restart command finished");
    Ok(format!("Successfully restarted {}", request.app_name))
}

fn spawn_output_bridge(
    output_tx: mpsc::UnboundedSender<CommandResult>,
) -> crate::process_manager_support::OutputChannel {
    let (core_tx, mut core_rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(line) = core_rx.recv().await {
            let _ = output_tx.send(CommandResult::LogLine(line));
        }
    });
    core_tx
}
