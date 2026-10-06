use crate::model::{RestartPolicy, Task};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedProcess {
    pub id: String,
    pub pid: u32,
    /// Process Group ID - used to track child processes if the parent exits
    pub pgid: Option<i32>,
    pub task: Task,
    pub start_time: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub runtime: ProcessRuntime,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessRuntime {
    pub restart_count: u32,
    pub history: Vec<RestartEvent>,
    pub last_exit_code: Option<i32>,
    pub last_exit_time: Option<DateTime<Utc>>,
    pub health_failures: u32,
    pub last_health_check: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartEvent {
    pub timestamp: DateTime<Utc>,
    pub reason: String,
}

pub struct StateStore {
    base_dir: PathBuf,
}

impl StateStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    pub fn save(&self, process: &ManagedProcess) -> Result<()> {
        let path = self.base_dir.join(format!("{}.json", process.id));
        let json = serde_json::to_string_pretty(process)?;
        fs::write(path, json)?;
        self.notify_change()?;
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<Option<ManagedProcess>> {
        let path = self.base_dir.join(format!("{}.json", id));
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(path)?;
        let process = serde_json::from_str(&content)?;
        Ok(Some(process))
    }

    pub fn list(&self) -> Result<Vec<ManagedProcess>> {
        let mut processes = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(p) = serde_json::from_str(&content) {
                        processes.push(p);
                    }
                }
            }
        }
        Ok(processes)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.base_dir.join(format!("{}.json", id));
        if path.exists() {
            fs::remove_file(path)?;
            self.notify_change()?;
        }
        Ok(())
    }

    /// Check if a process or its group is still running
    pub fn is_running(&self, process: &ManagedProcess) -> bool {
        #[cfg(unix)]
        {
            use std::process::Command;

            // Liveness is decided by the tracked PID, never by the process group.
            // A group can linger after its last member exited, so `kill -0 -PGID`
            // keeps succeeding for processes that no longer exist.
            let alive = Command::new("kill")
                .arg("-0")
                .arg(process.pid.to_string())
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if !alive {
                return false;
            }

            // `kill -0` also succeeds for a zombie — an exited child nobody has
            // reaped yet. When we are the parent we can settle it non-blockingly:
            // a successful waitpid means the process is already gone, and
            // reporting it as alive would stop the monitor from ever restarting a
            // crashed process. ECHILD means it is not our child, so trust kill -0.
            let mut status = 0;
            // SAFETY: waitpid(WNOHANG) only inspects/reaps a child of this process.
            match unsafe { libc::waitpid(process.pid as i32, &mut status, libc::WNOHANG) } {
                0 => true,
                pid if pid == process.pid as i32 => false,
                _ => true,
            }
        }
        #[cfg(not(unix))]
        {
            false // TODO: Windows support
        }
    }

    /// Path to the mtime-based notification file used by TUI status polling.
    pub fn status_changed_path(&self) -> PathBuf {
        self.base_dir.join(".status_changed")
    }

    /// Returns the last modification time of `.status_changed`, if the file exists.
    pub fn last_status_change(&self) -> Result<Option<SystemTime>> {
        let path = self.status_changed_path();
        if !path.exists() {
            return Ok(None);
        }
        Ok(fs::metadata(&path)?.modified().ok())
    }

    fn notify_change(&self) -> Result<()> {
        let path = self.status_changed_path();
        if path.exists() {
            let now = std::time::SystemTime::now();
            filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(now))?;
        } else {
            fs::write(&path, "")?;
        }
        Ok(())
    }

    pub fn cleanup_dead(&self) -> Result<Vec<String>> {
        let mut cleaned = Vec::new();
        for proc in self.list()? {
            if !self.is_running(&proc) {
                self.delete(&proc.id)?;
                cleaned.push(proc.id);
            }
        }
        Ok(cleaned)
    }

    /// Find processes by metadata key-value pair
    /// Returns all processes where metadata[key] == value
    pub fn find_by_metadata(&self, key: &str, value: &str) -> Result<Vec<ManagedProcess>> {
        let all_processes = self.list()?;
        Ok(all_processes
            .into_iter()
            .filter(|proc| proc.metadata.get(key).map(|v| v.as_str()) == Some(value))
            .collect())
    }

    /// Find a single process by metadata key-value pair
    /// Returns the first matching process, or None if no match found
    pub fn find_one_by_metadata(&self, key: &str, value: &str) -> Result<Option<ManagedProcess>> {
        let all_processes = self.list()?;
        Ok(all_processes
            .into_iter()
            .find(|proc| proc.metadata.get(key).map(|v| v.as_str()) == Some(value)))
    }

    /// Ensure the monitor daemon is running for this state store.
    ///
    /// Uses an exclusive file lock on `state_dir/.daemon.lock` to detect
    /// whether a daemon is already alive (the daemon holds the lock for its
    /// entire lifetime and releases it on exit).
    ///
    /// If no daemon is running, spawns `pm-daemon --state-dir <base_dir>`
    /// from the same directory as the current executable. Silently skips
    /// if the binary cannot be found (monitoring is optional infrastructure).
    pub fn ensure_daemon_running(&self) -> Result<()> {
        let lock_path = self.base_dir.join(".daemon.lock");

        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .context("Failed to open daemon lock file")?;

        match file.try_lock_exclusive() {
            Ok(_) => {
                // Lock acquired → daemon is NOT running.
                // Release now so pm-daemon can acquire it on startup.
                file.unlock().context("Failed to release daemon lock")?;
                drop(file);
                self.spawn_daemon()?;
            }
            Err(_) => {
                // Lock is held by the running daemon — nothing to do.
            }
        }

        Ok(())
    }

    fn spawn_daemon(&self) -> Result<()> {
        let Some(daemon_bin) = Self::find_daemon_binary() else {
            tracing::debug!(
                "pm-daemon not found alongside executable or in PATH — process monitoring disabled"
            );
            return Ok(());
        };

        std::process::Command::new(&daemon_bin)
            .arg("--state-dir")
            .arg(&self.base_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to spawn pm-daemon at {}", daemon_bin.display()))?;

        tracing::debug!(
            path = %daemon_bin.display(),
            state_dir = %self.base_dir.display(),
            "pm-daemon spawned"
        );
        Ok(())
    }

    /// Locate the `pm-daemon` binary.
    ///
    /// Search order:
    /// 1. Same directory as the current executable (e.g. target/debug/ in dev,
    ///    or ~/.cargo/bin/ when installed via `cargo install`)
    /// 2. Each directory in `$PATH`
    pub(crate) fn find_daemon_binary() -> Option<PathBuf> {
        // 1. Alongside current executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let candidate = dir.join("pm-daemon");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        // 2. Search PATH
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join("pm-daemon");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        None
    }
}

impl ManagedProcess {
    pub fn should_restart(&self, policy: &RestartPolicy) -> bool {
        if !policy.enabled {
            return false;
        }

        if let Some(code) = self.runtime.last_exit_code {
            if let Some(codes) = &policy.restart_on_exit_codes {
                if !codes.contains(&code) {
                    return false;
                }
            } else if code == 0 {
                return false;
            }
        }

        if policy.max_restarts > 0 {
            let window_start =
                Utc::now() - chrono::Duration::seconds(policy.restart_window_secs as i64);
            let recent = self
                .runtime
                .history
                .iter()
                .filter(|e| e.timestamp > window_start)
                .count() as u32;
            if recent >= policy.max_restarts {
                return false;
            }
        }
        true
    }

    pub fn calculate_backoff(&self, policy: &RestartPolicy) -> std::time::Duration {
        let attempts = self.runtime.restart_count.max(1);
        let backoff = policy.initial_backoff_secs as f64
            * policy.backoff_multiplier.powi((attempts - 1) as i32);
        let capped = backoff.min(policy.max_backoff_secs as f64);
        std::time::Duration::from_secs(capped as u64)
    }
}
