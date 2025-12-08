use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::tui::app::{CommandRequest, CommandResult, CommandType};

/// Executes a command asynchronously in a background task
/// This runs in a separate tokio task to avoid blocking the UI
/// Captures stdout/stderr directly from the spawned process
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
    // Get the CLI binary path
    let binary_path = std::env::current_exe()?;

    // Build the command: devcli start <app> --project <project> --env <env>
    let mut cmd = Command::new(binary_path);
    cmd.arg("start")
        .arg(&request.app_name)
        .arg("--project")
        .arg(&request.project)
        .arg("--env")
        .arg(&request.environment)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;

    // Capture stdout
    if let Some(stdout) = child.stdout.take() {
        let output_tx_clone = output_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = output_tx_clone.send(CommandResult::LogLine(line));
            }
        });
    }

    // Capture stderr
    // Note: Docker/OrbStack write normal output to stderr, so don't prefix for those
    if let Some(stderr) = child.stderr.take() {
        let output_tx_clone = output_tx.clone();
        let env = request.environment.clone();
        let is_docker_like = env == "docker" || env == "orbstack";
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let formatted = if is_docker_like {
                    line // Don't prefix for docker/orbstack
                } else {
                    format!("[stderr] {}", line)
                };
                let _ = output_tx_clone.send(CommandResult::LogLine(formatted));
            }
        });
    }

    // Wait for the command to complete
    let status = child.wait().await?;

    if !status.success() {
        anyhow::bail!("Start command failed with status: {}", status);
    }

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

    // Get the CLI binary path
    let binary_path = std::env::current_exe()?;

    // Build the command: devcli run <app> <command> --project <project> --env <env>
    let mut cmd = Command::new(binary_path);
    cmd.arg("run")
        .arg(&request.app_name)
        .arg(command_name)
        .arg("--project")
        .arg(&request.project)
        .arg("--env")
        .arg(env)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;

    // Capture stdout
    if let Some(stdout) = child.stdout.take() {
        let output_tx_clone = output_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = output_tx_clone.send(CommandResult::LogLine(line));
            }
        });
    }

    // Capture stderr
    // Note: Docker/OrbStack write normal output to stderr, so don't prefix for those
    if let Some(stderr) = child.stderr.take() {
        let output_tx_clone = output_tx.clone();
        let is_docker_like = env == "docker" || env == "orbstack";
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let formatted = if is_docker_like {
                    line // Don't prefix for docker/orbstack
                } else {
                    format!("[stderr] {}", line)
                };
                let _ = output_tx_clone.send(CommandResult::LogLine(formatted));
            }
        });
    }

    // Wait for the command to complete
    let status = child.wait().await?;

    if !status.success() {
        anyhow::bail!("Run command failed with status: {}", status);
    }

    crate::debug!("execute_command_async: Run command finished");
    Ok(format!("Successfully ran command for {}", request.app_name))
}

async fn execute_stop_command(
    request: CommandRequest,
    output_tx: mpsc::UnboundedSender<CommandResult>,
) -> Result<String> {
    // Get the CLI binary path
    let binary_path = std::env::current_exe()?;

    // Build the command: devcli stop <app> --project <project>
    let mut cmd = Command::new(binary_path);
    cmd.arg("stop")
        .arg(&request.app_name)
        .arg("--project")
        .arg(&request.project)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;

    // Capture stdout
    if let Some(stdout) = child.stdout.take() {
        let output_tx_clone = output_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = output_tx_clone.send(CommandResult::LogLine(line));
            }
        });
    }

    // Capture stderr
    if let Some(stderr) = child.stderr.take() {
        let output_tx_clone = output_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = output_tx_clone.send(CommandResult::LogLine(format!("[stderr] {}", line)));
            }
        });
    }

    // Wait for the command to complete
    let status = child.wait().await?;

    if !status.success() {
        anyhow::bail!("Stop command failed with status: {}", status);
    }

    crate::debug!("execute_command_async: Stop command finished");
    Ok(format!("Successfully stopped {}", request.app_name))
}

async fn execute_restart_command(
    request: CommandRequest,
    output_tx: mpsc::UnboundedSender<CommandResult>,
) -> Result<String> {
    // Get the CLI binary path
    let binary_path = std::env::current_exe()?;

    // Build the command: devcli restart <app> --project <project> --env <env>
    let mut cmd = Command::new(binary_path);
    cmd.arg("restart")
        .arg(&request.app_name)
        .arg("--project")
        .arg(&request.project)
        .arg("--env")
        .arg(&request.environment)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;

    // Capture stdout
    if let Some(stdout) = child.stdout.take() {
        let output_tx_clone = output_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = output_tx_clone.send(CommandResult::LogLine(line));
            }
        });
    }

    // Capture stderr
    if let Some(stderr) = child.stderr.take() {
        let output_tx_clone = output_tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = output_tx_clone.send(CommandResult::LogLine(format!("[stderr] {}", line)));
            }
        });
    }

    // Wait for the command to complete
    let status = child.wait().await?;

    if !status.success() {
        anyhow::bail!("Restart command failed with status: {}", status);
    }

    crate::debug!("execute_command_async: Restart command finished");
    Ok(format!("Successfully restarted {}", request.app_name))
}
