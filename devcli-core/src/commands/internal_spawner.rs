use crate::process::{ProcessInfo, ProcessTracker};
use crate::Result;
use anyhow::Context;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnerPayload {
    pub app_name: String,
    pub alternative_name: Option<String>,
    pub command: String,
    pub working_dir: PathBuf,
    pub env_vars: HashMap<String, String>,
    pub project: Option<String>,
    pub environment: Option<String>,
    pub command_variant: Option<String>,
    pub stage: Option<String>,
    pub log_file_path: PathBuf,
}

#[tracing::instrument(skip(payload_base64), name = "internal_spawner")]
pub async fn internal_spawner_command(payload_base64: String) -> Result<()> {
    // 1. Decode payload
    let payload_bytes = general_purpose::STANDARD
        .decode(&payload_base64)
        .context("Failed to decode payload")?;
    let payload: SpawnerPayload =
        serde_json::from_slice(&payload_bytes).context("Failed to parse payload JSON")?;

    tracing::info!(
        app = %payload.app_name,
        project = ?payload.project,
        environment = ?payload.environment,
        "Internal spawner starting"
    );

    let log_path = &payload.log_file_path;

    // 2. Open log file (append mode)
    // We use the same file logger mechanism or just raw file append
    // Since we are the only writer (conceptually), we can just open it.
    // But let's use the FileLogger to be consistent if possible, or just simple file I/O.
    // Simple file I/O is easier here since we don't need the rotation logic of FileLogger right now (or maybe we do?)
    // Let's use simple tokio file append for now.
    let log_file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .await
        .context("Failed to open log file")?;

    let log_writer = Arc::new(Mutex::new(log_file));

    // 3. Prepare Command
    let command_parts: Vec<&str> = payload.command.split_whitespace().collect();
    if command_parts.is_empty() {
        anyhow::bail!("Command cannot be empty");
    }
    let program = command_parts[0];
    let args = &command_parts[1..];

    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.current_dir(&payload.working_dir);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::null()); // Background process doesn't need stdin

    for (k, v) in &payload.env_vars {
        cmd.env(k, v);
    }

    // 4. Spawn Child
    let mut child = cmd.spawn().context("Failed to spawn application process")?;
    let child_pid = child
        .id()
        .ok_or_else(|| anyhow::anyhow!("Failed to get child PID"))?;

    tracing::debug!(
        app = %payload.app_name,
        child_pid = child_pid,
        command = %payload.command,
        "Child process spawned"
    );

    // 4.1. Verify child process actually started successfully
    // Give it a moment to fail if it's going to fail immediately
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Check if child is still running (hasn't exited immediately due to spawn failure)
    // Note: Some processes (like redis-server, nginx, etc.) fork/daemonize themselves,
    // so the parent may exit immediately while the child continues running. We only
    // treat this as a failure if the exit code indicates a real error (e.g., command not found).
    match child.try_wait() {
        Ok(Some(exit_status)) => {
            let exit_code = exit_status.code().unwrap_or(-1);

            // Exit codes that indicate actual failures (not just daemon forking):
            // - 127: command not found (shell)
            // - 126: command found but not executable
            // - negative codes: killed by signal
            let is_fatal_error = exit_code == 127 || exit_code == 126 || exit_code < 0;

            if is_fatal_error {
                tracing::error!(
                    app = %payload.app_name,
                    child_pid = child_pid,
                    exit_code = exit_code,
                    "Child process failed to start"
                );
                anyhow::bail!(
                    "Application process '{}' failed to start (exit code: {}). Check if the command exists and is executable.",
                    payload.app_name,
                    exit_code
                );
            } else {
                // Process exited with non-fatal code - likely daemonized or short-lived
                tracing::info!(
                    app = %payload.app_name,
                    child_pid = child_pid,
                    exit_code = exit_code,
                    "Process exited early (likely daemonized or completed quickly)"
                );
            }
        }
        Ok(None) => {
            // Child is still running - good!
            tracing::info!(
                app = %payload.app_name,
                child_pid = child_pid,
                "Child process verified running"
            );
        }
        Err(e) => {
            tracing::error!(
                app = %payload.app_name,
                error = %e,
                "Failed to check child process status"
            );
            anyhow::bail!("Failed to check child process status: {}", e);
        }
    }

    // 5. Register Process (Spawner PID)
    // We register OURSELVES as the process, so `devcli status` tracks US.
    // But we store the child PID in metadata if we want (ProcessInfo doesn't have a field for it yet, maybe add it later?)
    // For now, we just track us.
    let my_pid = std::process::id();

    let process_info = ProcessInfo {
        app_name: payload.app_name.clone(),
        pid: my_pid,
        command: payload.command.clone(),
        working_dir: payload.working_dir.to_string_lossy().to_string(),
        start_time: Utc::now(),
        env_vars: payload.env_vars.clone(),
        project: payload.project.clone(),
        app_config_name: Some(payload.app_name.clone()),
        environment: payload.environment.clone(),
        command_variant: payload.command_variant.clone(),
        stage: payload.stage.clone(),
        restart_count: 0,
        restart_history: Vec::new(),
        last_exit_code: None,
        last_exit_time: None,
        health_check_failures: 0,
        last_health_check: None,
    };

    let tracker = ProcessTracker::new()?;
    tracker.register_process(process_info)?;

    tracing::info!(
        app = %payload.app_name,
        spawner_pid = my_pid,
        child_pid = child_pid,
        "Process registered in tracker"
    );

    // 6. Handle Output Streaming
    let stdout = child.stdout.take().context("Failed to capture stdout")?;
    let stderr = child.stderr.take().context("Failed to capture stderr")?;

    let writer_stdout = log_writer.clone();
    let writer_stderr = log_writer.clone();

    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            // Try to print to stdout for parent process to capture
            // Ignore errors if stdout is closed (parent exited)
            let _ =
                std::io::Write::write_all(&mut std::io::stdout(), format!("{}\n", line).as_bytes());
            let _ = std::io::Write::flush(&mut std::io::stdout());

            // Write to log file (this is the important part)
            let mut w = writer_stdout.lock().await;
            use tokio::io::AsyncWriteExt;
            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S.%3f");
            let _ = w
                .write_all(format!("[{}] {}\n", timestamp, line).as_bytes())
                .await;
            let _ = w.flush().await;
        }
    });

    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            // Try to print to stderr for parent process to capture
            // Ignore errors if stderr is closed (parent exited)
            let _ =
                std::io::Write::write_all(&mut std::io::stderr(), format!("{}\n", line).as_bytes());
            let _ = std::io::Write::flush(&mut std::io::stderr());

            // Write to log file (this is the important part)
            let mut w = writer_stderr.lock().await;
            use tokio::io::AsyncWriteExt;
            let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S.%3f");
            let _ = w
                .write_all(format!("[{}] [STDERR] {}\n", timestamp, line).as_bytes())
                .await;
            let _ = w.flush().await;
        }
    });

    // 7. Handle Signals (Forwarding)
    // We need to listen for SIGINT/SIGTERM and forward to child
    // SIGHUP should be ignored (parent terminal closed, but we continue running)
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?;

    // Capture exit code for restart logic
    let exit_code = loop {
        tokio::select! {
            result = child.wait() => {
                // Child exited naturally
                break match result {
                    Ok(status) => status.code().unwrap_or(-1),
                    Err(_) => -1,
                };
            }
            _ = sigint.recv() => {
                // Received SIGINT, kill child
                // We use libc::kill to send signal to child PID
                tracing::info!(app = %payload.app_name, "Received SIGINT, forwarding to child");
                unsafe { libc::kill(child_pid as i32, libc::SIGINT) };
                break match child.wait().await {
                    Ok(status) => status.code().unwrap_or(-1),
                    Err(_) => -1,
                };
            }
            _ = sigterm.recv() => {
                // Received SIGTERM, kill child
                tracing::info!(app = %payload.app_name, "Received SIGTERM, forwarding to child");
                unsafe { libc::kill(child_pid as i32, libc::SIGTERM) };
                break match child.wait().await {
                    Ok(status) => status.code().unwrap_or(-1),
                    Err(_) => -1,
                };
            }
            _ = sighup.recv() => {
                // Received SIGHUP (parent terminal closed)
                // Ignore it - we want to keep running as a daemon
                tracing::debug!(app = %payload.app_name, "Received SIGHUP, ignoring (continuing as daemon)");
                // Don't break, continue the loop
            }
        }
    };

    // Record exit code before cleanup
    // This allows the monitor to detect crashes and trigger restarts
    tracing::info!(
        app = %payload.app_name,
        exit_code = exit_code,
        child_pid = child_pid,
        "Child process exited"
    );

    let _ = tracker.update_exit_info(
        payload.project.as_deref().unwrap_or("unknown"),
        &payload.app_name,
        payload.environment.as_deref(),
        exit_code,
    );

    // Wait for IO tasks
    let _ = tokio::join!(stdout_task, stderr_task);

    tracing::debug!(
        app = %payload.app_name,
        "Cleaning up process registration"
    );

    // Cleanup PID file
    // We need to re-instantiate tracker because it might have been dropped?
    // Actually tracker is just a struct, but we need to remove the file.
    // But wait, `devcli stop` removes the PID file BEFORE sending signal usually?
    // Or does it?
    // `devcli stop` calls `tracker.remove_process`.
    // If we remove it here again, it might fail or be redundant.
    // But if the app exits naturally (crashes), we MUST remove it.
    // So we should try to remove it.
    let _ = tracker.remove_process(
        payload.project.as_deref().unwrap_or("unknown"),
        &payload.app_name,
        payload.environment.as_deref(),
    );

    Ok(())
}
