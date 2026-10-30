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

/// Metadata key set by `stop` before it signals a process. pm-daemon leaves such
/// an entry alone: a process that dies because the user stopped it is not a
/// crash to recover from.
pub const STOPPING_KEY: &str = "stopping";

/// Exclusive hold on one state entry; released on drop.
///
/// Serialises the read-modify-write sequences of `stop` and of pm-daemon's
/// automatic restart, which otherwise interleave: the daemon could respawn an
/// app the user had just stopped and leave the new process running untracked.
pub struct EntryLock {
    file: fs::File,
}

/// The monitor singleton lock; released when dropped (or the process exits).
pub struct DaemonLock {
    _file: fs::File,
}

impl Drop for EntryLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

impl StateStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)?;
        // Entries hold commands and env-file values: keep them private to the
        // user, also for directories created by older versions.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&base_dir, fs::Permissions::from_mode(0o700))
                .with_context(|| format!("Failed to restrict {}", base_dir.display()))?;
        }
        Ok(Self { base_dir })
    }

    /// Directory holding the state entries.
    pub fn base_dir(&self) -> &std::path::Path {
        &self.base_dir
    }

    pub fn save(&self, process: &ManagedProcess) -> Result<()> {
        let path = self.base_dir.join(format!("{}.json", process.id));
        let json = serde_json::to_string_pretty(process)?;
        self.write_atomic(&path, json.as_bytes())?;
        self.notify_change()?;
        Ok(())
    }

    /// Replace `path` in one step: write a sibling temp file, then rename it.
    ///
    /// The daemon, the CLI and the TUI read these files concurrently. A plain
    /// `fs::write` truncates first, so a reader could see an empty or half-written
    /// file, which `list()` silently skips: the monitor then believed nothing was
    /// tracked and exited. A rename is atomic, so readers see the old or the new
    /// content, never a mix.
    fn write_atomic(&self, path: &std::path::Path, bytes: &[u8]) -> Result<()> {
        use std::io::Write;

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("state");
        // Leading dot and non-.json suffix: `list()` never picks up a temp file.
        let tmp = self
            .base_dir
            .join(format!(".{}.{}.tmp", file_name, uuid::Uuid::new_v4()));

        let result = (|| -> Result<()> {
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&tmp)
                .with_context(|| format!("Failed to create {}", tmp.display()))?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::rename(&tmp, path)
                .with_context(|| format!("Failed to replace {}", path.display()))?;
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    pub fn load(&self, id: &str) -> Result<Option<ManagedProcess>> {
        let path = self.base_dir.join(format!("{}.json", id));
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            // Deleted between our call and the read (another stop/cleanup).
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let process = serde_json::from_str(&content)?;
        Ok(Some(process))
    }

    pub fn list(&self) -> Result<Vec<ManagedProcess>> {
        let mut processes = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Ok(content) = fs::read_to_string(&path)
                && let Ok(p) = serde_json::from_str(&content)
            {
                processes.push(p);
            }
        }
        Ok(processes)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.base_dir.join(format!("{}.json", id));
        match fs::remove_file(path) {
            Ok(()) => self.notify_change()?,
            // Two concurrent `cleanup_dead()` calls (daemon tick + a CLI command)
            // race to delete the same entry; losing that race is not an error, and
            // used to abort the whole `start`/`status` with ENOENT.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }
}

/// Whether the process group still contains a live, non-zombie member.
///
/// `kill -0 -PGID` cannot answer this: a process group keeps resolving after its
/// last member exited, so a long-dead process would look alive forever (which
/// stops the monitor from ever restarting a crash). Enumerating the group and
/// filtering out zombies gives the answer the caller actually wants.
pub fn group_has_live_member(pgid: i32) -> bool {
    let output = match std::process::Command::new("ps")
        .args(["-eo", "pid=,pgid=,stat="])
        .output()
    {
        Ok(o) => o,
        Err(_) => return false,
    };

    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        let mut fields = line.split_whitespace();
        let group = fields.nth(1);
        let state = fields.next();
        matches!((group, state), (Some(g), Some(s))
                if g.parse::<i32>() == Ok(pgid) && !s.starts_with('Z'))
    })
}

impl StateStore {
    /// Check if a process or its group is still running
    pub fn is_running(&self, process: &ManagedProcess) -> bool {
        #[cfg(unix)]
        {
            // The tracked PID answers for itself - as long as it is still ours.
            if crate::engine::pid_alive(process.pid) {
                return !process.pid_was_reused();
            }

            // The leader is gone, but the app may still be alive as a child of its
            // group: `npm run dev` records npm's pid while node keeps serving.
            // Only trust the group if it still holds a live, non-zombie member.
            process.pgid.is_some_and(group_has_live_member)
        }
        #[cfg(not(unix))]
        {
            false // TODO: Windows support
        }
    }

    /// Block until no one else is modifying entry `id`.
    ///
    /// Hold it only around short load/update/save sequences.
    pub fn lock_entry(&self, id: &str) -> Result<EntryLock> {
        let dir = self.base_dir.join("locks");
        fs::create_dir_all(&dir)?;
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(dir.join(format!("{id}.entry.lock")))
            .context("Failed to open state entry lock")?;
        file.lock_exclusive()
            .context("Failed to lock state entry")?;
        Ok(EntryLock { file })
    }

    /// Flag entry `id` as being stopped and return its current content.
    ///
    /// Callers must signal the PID of the returned entry, not of an earlier
    /// snapshot: pm-daemon may have restarted the app in the meantime.
    pub fn mark_stopping(&self, id: &str) -> Result<Option<ManagedProcess>> {
        let _guard = self.lock_entry(id)?;
        let Some(mut process) = self.load(id)? else {
            return Ok(None);
        };
        process
            .metadata
            .insert(STOPPING_KEY.to_string(), "true".to_string());
        self.save(&process)?;
        Ok(Some(process))
    }

    /// Take the monitor singleton lock, or `None` if a monitor already holds it.
    ///
    /// Exactly one monitor loop may run per state directory; two would both
    /// restart a crashed app.
    pub fn try_lock_daemon(&self) -> Result<Option<DaemonLock>> {
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(self.base_dir.join(".daemon.lock"))
            .context("Failed to open daemon lock file")?;
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Some(DaemonLock { _file: file })),
            Err(_) => Ok(None),
        }
    }

    /// Whether a pm-daemon currently holds this store's singleton lock.
    pub fn daemon_running(&self) -> bool {
        let Ok(file) = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.base_dir.join(".daemon.lock"))
        else {
            return false;
        };
        match file.try_lock_exclusive() {
            Ok(()) => {
                let _ = FileExt::unlock(&file);
                false
            }
            Err(_) => true,
        }
    }

    /// Whether pm-daemon will pick this dead entry up and restart it.
    fn awaiting_restart(&self, process: &ManagedProcess) -> bool {
        !process.is_stopping()
            && process.should_restart(&process.task.restart_policy)
            && self.daemon_running()
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
            // Every CLI command runs this first. Deleting a crashed app that the
            // daemon is about to restart (it checks every few seconds, then waits
            // out the backoff) silently disabled auto-restart.
            if !self.is_running(&proc) && !self.awaiting_restart(&proc) {
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

        let mut cmd = std::process::Command::new(&daemon_bin);
        cmd.arg("--state-dir")
            .arg(&self.base_dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        // Detach into a session of its own. Otherwise the daemon stays in the
        // foreground process group of whatever terminal ran `devcli start`, so
        // Ctrl+C on a streaming `start` (or closing that terminal) killed it and
        // silently ended monitoring and auto-restart for every app.
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // SAFETY: setsid(2) is async-signal-safe and touches no Rust state.
            unsafe {
                cmd.pre_exec(|| {
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        cmd.spawn()
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
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let candidate = dir.join("pm-daemon");
            if candidate.exists() {
                return Some(candidate);
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

/// Slack between the OS start of a process and the `start_time` we record just
/// after spawning it (`ps` reports whole seconds).
const PID_REUSE_TOLERANCE: chrono::Duration = chrono::Duration::seconds(5);

impl ManagedProcess {
    /// Whether the live process holding `self.pid` started after we recorded
    /// ours, i.e. ours exited and the OS handed the PID to something else.
    ///
    /// Without this, `stop` (or `stop --all` on stale state) would signal an
    /// unrelated process of the user's that happened to receive the PID. When
    /// the age cannot be read the PID is assumed to be ours.
    /// Whether `stop` is taking this process down.
    pub fn is_stopping(&self) -> bool {
        self.metadata.contains_key(STOPPING_KEY)
    }

    pub fn pid_was_reused(&self) -> bool {
        let Some(age) = crate::engine::process_age(self.pid) else {
            return false;
        };
        let Ok(age) = chrono::Duration::from_std(age) else {
            return false;
        };
        let os_start = Utc::now() - age;
        os_start > self.start_time + PID_REUSE_TOLERANCE
    }

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
