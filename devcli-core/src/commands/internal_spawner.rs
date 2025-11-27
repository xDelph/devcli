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
    pub command: String,
    pub working_dir: PathBuf,
    pub env_vars: HashMap<String, String>,
    pub project: Option<String>,
    pub environment: Option<String>,
    pub command_variant: Option<String>,
    pub stage: Option<String>,
    pub log_file_path: PathBuf,
}

pub async fn internal_spawner_command(payload_base64: String) -> Result<()> {
    // 1. Decode payload
    let payload_bytes = general_purpose::STANDARD
        .decode(&payload_base64)
        .context("Failed to decode payload")?;
    let payload: SpawnerPayload =
        serde_json::from_slice(&payload_bytes).context("Failed to parse payload JSON")?;

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
    };

    let tracker = ProcessTracker::new()?;
    tracker.register_process(process_info)?;

    // 6. Handle Output Streaming
    let stdout = child.stdout.take().context("Failed to capture stdout")?;
    let stderr = child.stderr.take().context("Failed to capture stderr")?;

    let writer_stdout = log_writer.clone();
    let writer_stderr = log_writer.clone();

    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            // Print to stdout for parent process to capture
            println!("{}", line);

            // Write to log file
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
            // Print to stderr for parent process to capture
            eprintln!("{}", line);

            // Write to log file
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
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    tokio::select! {
        _ = child.wait() => {
            // Child exited
        }
        _ = sigint.recv() => {
            // Received SIGINT, kill child
            // We use libc::kill to send signal to child PID
            unsafe { libc::kill(child_pid as i32, libc::SIGINT) };
            let _ = child.wait().await;
        }
        _ = sigterm.recv() => {
            // Received SIGTERM, kill child
            unsafe { libc::kill(child_pid as i32, libc::SIGTERM) };
            let _ = child.wait().await;
        }
    }

    // Wait for IO tasks
    let _ = tokio::join!(stdout_task, stderr_task);

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
