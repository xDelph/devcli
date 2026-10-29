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

    // A detached child must outlive the process that spawned it (the CLI exits
    // right after `start`, the TUI when the user quits). If its stdout/stderr
    // were pipes read by *this* process, the next write after we exit would hit
    // a closed pipe: SIGPIPE kills most programs and Node crashes on EPIPE. So a
    // detached child writes straight into its log file and the kernel keeps that
    // descriptor valid for as long as the child lives.
    let detached_log = match (&task.log_file, task.is_detached) {
        (Some(path), true) => Some(open_log_for_child(path)?),
        _ => None,
    };

    match &detached_log {
        Some((file, _)) => {
            cmd.stdout(Stdio::from(
                file.try_clone().context("Failed to share log file")?,
            ))
            .stderr(Stdio::from(
                file.try_clone().context("Failed to share log file")?,
            ));
        }
        None => {
            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        }
    }

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

    if let (Some((_, start_offset)), Some(path)) = (&detached_log, &task.log_file) {
        // Keep `output_rx` working for callers that stream output while they are
        // alive (attached `start`, `run`, the TUI) by following the log file.
        spawn_log_follower(path.clone(), *start_offset, pid, tx);
    } else if let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) {
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

        let alive = process_exists(pid) || pgid.is_some_and(crate::state::group_has_live_member);

        if !alive {
            return Ok(false);
        }

        let signal = if force { "-9" } else { "-15" };

        // Signal the whole group so children of a wrapper die with it, but treat
        // it as best effort: `kill -9 -PGID` can exit 0 without terminating
        // anything (observed on CI runners), which would make us report success on
        // a process that is still running.
        if let Some(g) = pgid {
            let _ = Command::new("kill")
                .arg(signal)
                .arg(format!("-{}", g))
                .status();
        }

        // The PID itself is the reliable target. A non-zero status is not an
        // error: the group signal above may already have killed it.
        let _ = Command::new("kill")
            .arg(signal)
            .arg(pid.to_string())
            .status();

        // Confirm the process is actually gone instead of trusting the exit status.
        // When the leader had already exited we must also wait for the group to
        // empty out, otherwise children of the app keep serving unnoticed.
        for _ in 0..10 {
            let gone =
                !process_exists(pid) && !pgid.is_some_and(crate::state::group_has_live_member);
            if gone {
                return Ok(true);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        Ok(false)
    }
    #[cfg(not(unix))]
    {
        anyhow::bail!("Termination not yet implemented for this platform");
    }
}

/// Signal-0 probe without spawning a process (cheap enough to poll).
fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let Ok(pid) = i32::try_from(pid) else {
            return false;
        };
        // SAFETY: kill(2) with signal 0 performs no action beyond permission and
        // existence checks.
        let rc = unsafe { libc::kill(pid, 0) };
        rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
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

/// Open (creating if needed) a log file in append mode for a child to inherit.
///
/// Returns the file and its length before the child writes anything, which is
/// where a follower starts reading.
fn open_log_for_child(path: &std::path::Path) -> Result<(std::fs::File, u64)> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create log directory {}", parent.display()))?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("Failed to open log file {}", path.display()))?;
    let offset = file.metadata().map(|m| m.len()).unwrap_or(0);
    Ok((file, offset))
}

/// Forward lines appended to `path` after `offset` into `tx`.
///
/// Stops as soon as the receiver is dropped, so a caller that is not interested
/// in output (detached `start`) costs nothing beyond one short-lived task. Once
/// the child is gone and the file is drained the sender is dropped, which closes
/// the channel exactly like the end of a pipe would.
fn spawn_log_follower(path: PathBuf, offset: u64, pid: u32, tx: OutputSender) {
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};

        let Ok(mut file) = tokio::fs::File::open(&path).await else {
            return;
        };
        if file.seek(std::io::SeekFrom::Start(offset)).await.is_err() {
            return;
        }

        let mut pending: Vec<u8> = Vec::new();
        let mut buf = vec![0u8; 8192];
        loop {
            if tx.is_closed() {
                return;
            }
            match file.read(&mut buf).await {
                Ok(0) => {
                    if !pid_alive(pid) {
                        if !pending.is_empty() {
                            let _ = tx.send(OutputMessage {
                                source: OutputSource::Stdout,
                                content: String::from_utf8_lossy(&pending).to_string(),
                                timestamp: chrono::Utc::now(),
                            });
                        }
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
                        let line: Vec<u8> = pending.drain(..=pos).collect();
                        let content = String::from_utf8_lossy(&line[..line.len() - 1])
                            .trim_end_matches('\r')
                            .to_string();
                        let msg = OutputMessage {
                            source: OutputSource::Stdout,
                            content,
                            timestamp: chrono::Utc::now(),
                        };
                        if tx.send(msg).is_err() {
                            return;
                        }
                    }
                }
                Err(_) => return,
            }
        }
    });
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

        // Read raw bytes: `lines()` stops at the first invalid UTF-8 sequence,
        // which closed the pipe and got the child killed by SIGPIPE as soon as it
        // printed binary output or text in a legacy encoding.
        let mut reader = BufReader::new(stream);
        let mut raw = Vec::new();
        loop {
            raw.clear();
            match reader.read_until(b'\n', &mut raw).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let line = String::from_utf8_lossy(&raw)
                .trim_end_matches('\n')
                .trim_end_matches('\r')
                .to_string();
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
