use crate::model::{OutputMessage, OutputSource, Task};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::mpsc;

pub type OutputReceiver = mpsc::UnboundedReceiver<OutputMessage>;
pub type OutputSender = mpsc::UnboundedSender<OutputMessage>;

#[derive(Debug)]
pub struct RunningProcess {
    pub pid: u32,
    pub pgid: Option<i32>,
    pub child: Option<Child>,
    pub output_rx: OutputReceiver,
}

pub async fn spawn(task: &Task) -> Result<RunningProcess> {
    let parts: Vec<&str> = task.command.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Command cannot be empty");
    }
    let program = parts[0];
    let args = &parts[1..];

    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args);
    cmd.args(&task.args);
    cmd.current_dir(&task.working_dir);
    cmd.envs(&task.env);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    if task.is_detached {
        cmd.stdin(Stdio::null());

        #[cfg(unix)]
        {
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
    }

    let mut child = cmd.spawn().context("Failed to spawn process")?;
    let pid = child.id().ok_or_else(|| anyhow::anyhow!("No PID"))?;

    let pgid = if task.is_detached {
        Some(pid as i32)
    } else {
        None
    };

    let (tx, rx) = mpsc::unbounded_channel();

    if let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) {
        spawn_output_handler(
            stdout,
            OutputSource::Stdout,
            task.log_file.clone(),
            tx.clone(),
        );
        spawn_output_handler(stderr, OutputSource::Stderr, task.log_file.clone(), tx);
    }

    Ok(RunningProcess {
        pid,
        pgid,
        child: if task.is_detached { None } else { Some(child) },
        output_rx: rx,
    })
}

/// Terminate a process or its entire group.
pub async fn terminate(pid: u32, pgid: Option<i32>, force: bool) -> Result<bool> {
    #[cfg(unix)]
    {
        use std::process::Command;

        let target = if let Some(id) = pgid {
            format!("-{}", id)
        } else {
            pid.to_string()
        };

        let check = Command::new("kill")
            .arg("-0")
            .arg(&target)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !check {
            return Ok(false);
        }

        let signal = if force { "-9" } else { "-15" };

        let status = Command::new("kill")
            .arg(signal)
            .arg(&target)
            .status()
            .context("Failed to execute kill command")?;

        if !status.success() && force {
            anyhow::bail!("Failed to kill process {}", target);
        }

        Ok(true)
    }
    #[cfg(not(unix))]
    {
        anyhow::bail!("Termination not yet implemented for this platform");
    }
}

/// Restart a managed process.
pub async fn restart(
    task: &Task,
    old_pid: u32,
    old_pgid: Option<i32>,
    backoff: std::time::Duration,
) -> Result<RunningProcess> {
    let _ = terminate(old_pid, old_pgid, true).await;

    if !backoff.is_zero() {
        tokio::time::sleep(backoff).await;
    }

    spawn(task).await
}

fn spawn_output_handler<R>(
    stream: R,
    source: OutputSource,
    log_file: Option<PathBuf>,
    tx: OutputSender,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let file = if let Some(path) = log_file {
            tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .await
                .ok()
                .map(|f| std::sync::Arc::new(tokio::sync::Mutex::new(f)))
        } else {
            None
        };

        let mut reader = BufReader::new(stream).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let msg = OutputMessage {
                source: source.clone(),
                content: line.clone(),
                timestamp: chrono::Utc::now(),
            };

            let _ = tx.send(msg);

            if let Some(ref f) = file {
                let mut guard = f.lock().await;
                let ts = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S.%3f");
                let tag = match source {
                    OutputSource::Stdout => "STDOUT",
                    OutputSource::Stderr => "STDERR",
                };
                let _ = guard
                    .write_all(format!("[{}] [{}] {}\n", ts, tag, line).as_bytes())
                    .await;
            }
        }
    });
}
