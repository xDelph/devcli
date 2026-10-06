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

impl RunningProcess {
    /// Returns `true` if the spawned process is still alive.
    ///
    /// Uses a signal-0 probe on Unix (no signal delivered, just existence check).
    /// Callers should not need to touch `self.pid` directly for liveness checks.
    pub fn is_alive(&self) -> bool {
        #[cfg(unix)]
        {
            use std::process::Command;
            Command::new("kill")
                .arg("-0")
                .arg(self.pid.to_string())
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            true
        }
    }
}

pub async fn spawn(task: &Task) -> Result<RunningProcess> {
    // Parse command using shell-words to handle quoted arguments correctly
    let parts = shell_words::split(&task.command).context("Failed to parse command string")?;

    if parts.is_empty() {
        anyhow::bail!("Command cannot be empty");
    }

    let program = &parts[0];
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
        use std::time::Duration;

        let alive = Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !alive {
            return Ok(false);
        }

        let signal = if force { "-9" } else { "-15" };

        // Signal the whole group first so children of a wrapper die with it, but
        // never rely on it: `kill -9 -PGID` can exit 0 without terminating
        // anything (observed on CI runners), which would make us report success
        // on a process that is still running.
        if let Some(g) = pgid {
            let _ = Command::new("kill")
                .arg(signal)
                .arg(format!("-{}", g))
                .status();
        }

        // The PID itself is the reliable target. A non-zero status here is not an
        // error: the group signal above may already have killed it.
        let pid_status = Command::new("kill")
            .arg(signal)
            .arg(pid.to_string())
            .status()
            .context("Failed to execute kill command")?;

        // Reap our own child so the zombie stops answering `kill -0`.
        reap_child(pid);

        // Confirm the process is actually gone instead of assuming it.
        for _ in 0..40 {
            if !process_exists(pid) {
                return Ok(true);
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        if !pid_status.success() && force {
            anyhow::bail!("Failed to kill process {}", pid);
        }

        Ok(false)
    }
    #[cfg(not(unix))]
    {
        anyhow::bail!("Termination not yet implemented for this platform");
    }
}

/// Non-blocking reap of our own child; a no-op if it was already reaped.
fn reap_child(pid: u32) {
    let mut status = 0;
    // SAFETY: waitpid only inspects/reaps our own child; WNOHANG never blocks.
    unsafe {
        libc::waitpid(pid as i32, &mut status, libc::WNOHANG);
    }
}

/// Whether the OS still knows about this PID.
fn process_exists(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HealthCheck, RestartPolicy, Task};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tokio::sync::mpsc;

    fn make_running_process(pid: u32) -> RunningProcess {
        let (_tx, rx) = mpsc::unbounded_channel();
        RunningProcess {
            pid,
            pgid: None,
            child: None,
            output_rx: rx,
        }
    }

    /// A RunningProcess whose PID is the current test process must be alive.
    #[test]
    fn test_is_alive_current_process() {
        let current_pid = std::process::id();
        let rp = make_running_process(current_pid);
        assert!(rp.is_alive(), "current process must report itself as alive");
    }

    /// PID 0 is never a valid user process and must report as not alive.
    /// On Linux/macOS, `kill -0 0` sends to the process *group* which could
    /// succeed, so we use a clearly invalid large PID instead.
    #[test]
    fn test_is_alive_dead_process() {
        // PID 4_194_304 is above the kernel max on Linux (default 4_194_304 is
        // the ceiling; we use one above that). On macOS the max is 99_998.
        // Using a very large number that is guaranteed not to be running.
        let dead_pid = 2_000_000_000_u32;
        let rp = make_running_process(dead_pid);
        assert!(
            !rp.is_alive(),
            "a PID that cannot exist must report as dead"
        );
    }

    /// is_alive() on the same PID called twice must be consistent (no side
    /// effects from the signal-0 probe).
    #[test]
    fn test_is_alive_is_idempotent() {
        let current_pid = std::process::id();
        let rp = make_running_process(current_pid);
        assert_eq!(rp.is_alive(), rp.is_alive());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn test_spawn_rejects_empty_command() {
        let task = Task {
            id: "empty-cmd".to_string(),
            command: "   ".to_string(),
            args: vec![],
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
            env: HashMap::new(),
            is_detached: false,
            log_file: None,
            health_check: HealthCheck::Process {},
            restart_policy: RestartPolicy::default(),
        };

        let err = spawn(&task).await.unwrap_err();
        assert!(
            err.to_string().contains("empty") || err.to_string().contains("Failed to parse"),
            "unexpected error: {err}"
        );
    }
}
