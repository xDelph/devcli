use crate::model::{RestartPolicy, Task};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

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

/// What [`StateStore::spawn_unless_running`] did.
#[derive(Debug)]
pub enum SpawnOutcome {
    /// The process was spawned and recorded.
    Started {
        process: ManagedProcess,
        output_rx: crate::engine::OutputReceiver,
    },
    /// The entry already held this live process; nothing was spawned.
    AlreadyRunning(ManagedProcess),
}

/// Metadata key set by `stop` before it signals a process. pm-daemon leaves such
/// an entry alone: a process that dies because the user stopped it is not a
/// crash to recover from.
pub const STOPPING_KEY: &str = "stopping";

/// Metadata key set next to [`STOPPING_KEY`]: when the stop began (RFC 3339).
/// A live process holding the stopper's PID that started later is not the
/// stopper (its PID was reused), so the stop counts as abandoned.
pub const STOPPING_SINCE_KEY: &str = "stopping_since";

/// Metadata key holding the PID of the attached `devcli start` that owns the
/// process. The owner stops its processes when it exits; pm-daemon stops them
/// if the owner died without doing so (killed, crashed, terminal closed).
pub const OWNER_PID_KEY: &str = "owner_pid";

/// Metadata key set on an owned process that another app depends on: when
/// its owner ends (or dies), the process is released (it loses its owner and
/// keeps running) instead of being stopped under the app that needs it.
pub const SHARED_KEY: &str = "shared";

/// Metadata key holding the root of the git worktree the process runs from.
/// When that directory is deleted (a sub-agent removing its worktree), the
/// app has nothing left to serve and pm-daemon stops it.
pub const WORKTREE_ROOT_KEY: &str = "worktree_root";

/// How long [`StateStore::lock_entry`] waits for another holder of the lock.
///
/// Holders keep it for a few milliseconds (a load/update/save), or for the
/// kill and respawn of an automatic restart. Waiting forever turned any stuck
/// holder into a `devcli stop` that hung without a word.
pub const ENTRY_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

/// Interval between two attempts to take a busy entry lock.
const ENTRY_LOCK_POLL: Duration = Duration::from_millis(20);

/// Exclusive hold on one state entry; released on drop.
///
/// Serialises the read-modify-write sequences of `stop`, `start`, health
/// checks and pm-daemon's automatic restart, which otherwise interleave: the
/// daemon could respawn an app the user had just stopped and leave the new
/// process running untracked, or write back a copy of the entry that lacks
/// the `stopping` flag or the PID of a restart.
pub struct EntryLock {
    file: fs::File,
}

/// A restart in progress (see [`StateStore::begin_restart`]); removed on drop.
pub struct RestartMarker {
    path: PathBuf,
}

impl Drop for RestartMarker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
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
        // The tracked PID answers for itself - as long as it is still ours.
        if crate::engine::pid_alive(process.pid) {
            return !process.pid_was_reused();
        }

        // The leader is gone, but the app may still be alive as a child of its
        // group: `npm run dev` records npm's pid while node keeps serving.
        // Only trust the group if it still holds a live, non-zombie member.
        process.pgid.is_some_and(group_has_live_member)
    }

    /// Wait (at most [`ENTRY_LOCK_TIMEOUT`]) until no one else is modifying
    /// entry `id`, and hold it until the returned guard is dropped.
    ///
    /// Hold it only around short load/update/save sequences. This blocks the
    /// calling thread: from async code (pm-daemon), use
    /// [`Self::lock_entry_async`] instead.
    pub fn lock_entry(&self, id: &str) -> Result<EntryLock> {
        let deadline = std::time::Instant::now() + ENTRY_LOCK_TIMEOUT;
        loop {
            if let Some(lock) = self.try_lock_entry(id)? {
                return Ok(lock);
            }
            if std::time::Instant::now() >= deadline {
                return Err(lock_timeout(id));
            }
            std::thread::sleep(ENTRY_LOCK_POLL);
        }
    }

    /// [`Self::lock_entry`] for async code: waits without blocking the thread.
    ///
    /// `flock` blocks the thread that calls it. Inside pm-daemon's runtime a
    /// task waiting that way could occupy the very worker the holder needed
    /// to make progress, and the daemon (and every `devcli stop` waiting on
    /// the lock) hung for good.
    pub async fn lock_entry_async(&self, id: &str) -> Result<EntryLock> {
        let deadline = tokio::time::Instant::now() + ENTRY_LOCK_TIMEOUT;
        loop {
            if let Some(lock) = self.try_lock_entry(id)? {
                return Ok(lock);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(lock_timeout(id));
            }
            tokio::time::sleep(ENTRY_LOCK_POLL).await;
        }
    }

    /// Take the lock on entry `id` if nobody holds it, without waiting.
    pub fn try_lock_entry(&self, id: &str) -> Result<Option<EntryLock>> {
        let dir = self.base_dir.join("locks");
        fs::create_dir_all(&dir)?;
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(dir.join(format!("{id}.entry.lock")))
            .context("Failed to open state entry lock")?;
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Some(EntryLock { file })),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.raw_os_error() == fs2::lock_contended_error().raw_os_error() =>
            {
                Ok(None)
            }
            Err(e) => Err(anyhow::Error::new(e).context("Failed to lock state entry")),
        }
    }

    /// Flag entry `id` as being stopped and return its current content.
    ///
    /// Callers must signal the PID of the returned entry, not of an earlier
    /// snapshot: pm-daemon may have restarted the app in the meantime.
    pub fn mark_stopping(&self, id: &str) -> Result<Option<ManagedProcess>> {
        let guard = self.lock_entry(id)?;
        self.mark_stopping_locked(&guard, id)
    }

    /// [`Self::mark_stopping`] for async code (see [`Self::lock_entry_async`]).
    pub async fn mark_stopping_async(&self, id: &str) -> Result<Option<ManagedProcess>> {
        let guard = self.lock_entry_async(id).await?;
        self.mark_stopping_locked(&guard, id)
    }

    fn mark_stopping_locked(&self, _guard: &EntryLock, id: &str) -> Result<Option<ManagedProcess>> {
        let Some(mut process) = self.load(id)? else {
            return Ok(None);
        };
        // Who is stopping it, and since when: if that process dies midway,
        // pm-daemon finishes the job instead of skipping the entry forever.
        process
            .metadata
            .insert(STOPPING_KEY.to_string(), std::process::id().to_string());
        process
            .metadata
            .insert(STOPPING_SINCE_KEY.to_string(), Utc::now().to_rfc3339());
        self.save(&process)?;
        Ok(Some(process))
    }

    /// Delete entry `id` if it still describes the process `pid`. Returns
    /// whether it was deleted.
    ///
    /// For whoever stopped (or gave up on) that process: by the time it is
    /// done, a fresh `start` may have recorded a new process under the same
    /// id, and a plain `delete` would leave that one running untracked.
    pub fn forget(&self, id: &str, pid: u32) -> Result<bool> {
        let guard = self.lock_entry(id)?;
        self.forget_locked(&guard, id, pid)
    }

    /// [`Self::forget`] for async code (see [`Self::lock_entry_async`]).
    pub async fn forget_async(&self, id: &str, pid: u32) -> Result<bool> {
        let guard = self.lock_entry_async(id).await?;
        self.forget_locked(&guard, id, pid)
    }

    /// [`Self::forget`] for a caller that already holds the lock on `id`.
    pub fn forget_locked(&self, _guard: &EntryLock, id: &str, pid: u32) -> Result<bool> {
        match self.load(id)? {
            Some(current) if current.pid == pid => {
                self.delete(id)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Apply `update` to entry `id` and save it, if the entry still describes
    /// the process `pid` and nobody is stopping it. Returns the saved entry.
    ///
    /// For updates computed from an earlier read (a health check that took
    /// seconds): writing that earlier copy back would drop a `stopping` flag
    /// set meanwhile (and the stopped app came back) or the PID of a restart.
    pub fn update_if_current(
        &self,
        id: &str,
        pid: u32,
        update: impl FnOnce(&mut ManagedProcess),
    ) -> Result<Option<ManagedProcess>> {
        let guard = self.lock_entry(id)?;
        self.update_if_current_locked(&guard, id, pid, update)
    }

    /// [`Self::update_if_current`] for async code (see [`Self::lock_entry_async`]).
    pub async fn update_if_current_async(
        &self,
        id: &str,
        pid: u32,
        update: impl FnOnce(&mut ManagedProcess),
    ) -> Result<Option<ManagedProcess>> {
        let guard = self.lock_entry_async(id).await?;
        self.update_if_current_locked(&guard, id, pid, update)
    }

    fn update_if_current_locked(
        &self,
        _guard: &EntryLock,
        id: &str,
        pid: u32,
        update: impl FnOnce(&mut ManagedProcess),
    ) -> Result<Option<ManagedProcess>> {
        let Some(mut current) = self.load(id)? else {
            return Ok(None);
        };
        if current.pid != pid || current.is_stopping() {
            return Ok(None);
        }
        update(&mut current);
        self.save(&current)?;
        Ok(Some(current))
    }

    /// Spawn `task` and record it under `task.id`, unless that entry already
    /// holds a live process.
    ///
    /// The check, the spawn and the save happen under the entry lock: two
    /// commands starting the same app (`start api` and `start worker` both
    /// auto-starting a missing redis) used to both spawn it, the second save
    /// overwriting the first and leaving one copy running untracked.
    pub async fn spawn_unless_running(
        &self,
        task: Task,
        metadata: HashMap<String, String>,
    ) -> Result<SpawnOutcome> {
        let _guard = self.lock_entry_async(&task.id).await?;
        if let Some(existing) = self.load(&task.id)?
            && self.is_running(&existing)
        {
            return Ok(SpawnOutcome::AlreadyRunning(existing));
        }
        let running = crate::engine::spawn(&task).await?;
        let process = ManagedProcess {
            id: task.id.clone(),
            pid: running.pid,
            pgid: running.pgid,
            task,
            start_time: Utc::now(),
            metadata,
            runtime: ProcessRuntime::default(),
        };
        // Replaces a dead entry, if any: pm-daemon, about to restart it,
        // finds another PID and leaves it alone.
        self.save(&process)?;
        Ok(SpawnOutcome::Started {
            process,
            output_rx: running.output_rx,
        })
    }

    /// Record that `process` is being restarted, until the returned marker is
    /// dropped: between the stop and the start no entry exists, and the
    /// attached session owning the app must not take that gap for its end.
    pub fn begin_restart(&self, process: &ManagedProcess) -> Result<RestartMarker> {
        let path = self.base_dir.join(format!("{}.restarting", process.id));
        let owner = process
            .owner_pid()
            .map(|o| o.to_string())
            .unwrap_or_else(|| "-".to_string());
        // When the marker was written: a live PID that started later is not
        // the restarter (it died and the OS reused its PID).
        fs::write(
            &path,
            format!(
                "{} {} {}",
                std::process::id(),
                owner,
                Utc::now().timestamp()
            ),
        )?;
        Ok(RestartMarker { path })
    }

    /// Whether a restart still running (its process alive) is replacing an
    /// app owned by `owner`.
    pub fn restart_pending_for_owner(&self, owner: &str) -> bool {
        let Ok(entries) = fs::read_dir(&self.base_dir) else {
            return false;
        };
        entries.flatten().any(|entry| {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("restarting") {
                return false;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                return false;
            };
            let mut fields = content.split_whitespace();
            let restarter = fields.next().and_then(|p| p.parse::<u32>().ok());
            let for_owner = fields.next() == Some(owner);
            // Markers written by older versions carry no time.
            let written = fields
                .next()
                .and_then(|t| t.parse::<i64>().ok())
                .and_then(|t| DateTime::<Utc>::from_timestamp(t, 0));
            for_owner
                && restarter.is_some_and(|pid| {
                    crate::engine::pid_alive(pid)
                        && !written.is_some_and(|since| pid_started_after(pid, since))
                })
        })
    }

    /// Mark entry `id` as needed by another app (see [`SHARED_KEY`]).
    pub fn mark_shared(&self, id: &str) -> Result<()> {
        let _guard = self.lock_entry(id)?;
        if let Some(mut process) = self.load(id)?
            && !process.is_shared()
        {
            process
                .metadata
                .insert(SHARED_KEY.to_string(), "true".to_string());
            self.save(&process)?;
        }
        Ok(())
    }

    /// Let entry `id` go: it keeps running, owned by nobody, until stopped.
    pub fn release(&self, id: &str) -> Result<()> {
        let guard = self.lock_entry(id)?;
        self.release_locked(&guard, id)
    }

    /// [`Self::release`] for async code (see [`Self::lock_entry_async`]).
    pub async fn release_async(&self, id: &str) -> Result<()> {
        let guard = self.lock_entry_async(id).await?;
        self.release_locked(&guard, id)
    }

    fn release_locked(&self, _guard: &EntryLock, id: &str) -> Result<()> {
        if let Some(mut process) = self.load(id)? {
            process.metadata.remove(OWNER_PID_KEY);
            process.metadata.remove(SHARED_KEY);
            self.save(&process)?;
        }
        Ok(())
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
    pub fn awaiting_restart(&self, process: &ManagedProcess) -> bool {
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
            if self.is_running(&proc) || self.awaiting_restart(&proc) {
                continue;
            }
            // Someone is working on this entry (a stop, a start, a restart):
            // leave it to them rather than wait.
            let Some(guard) = self.try_lock_entry(&proc.id)? else {
                continue;
            };
            // Decide on the entry as it is now: a `start` may have recorded a
            // new process under this id since the listing.
            let Some(current) = self.load(&proc.id)? else {
                continue;
            };
            if current.pid == proc.pid
                && !self.is_running(&current)
                && !self.awaiting_restart(&current)
                && self.forget_locked(&guard, &proc.id, proc.pid)?
            {
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

/// When the process holding `pid` was started, according to the OS.
fn os_start_time(pid: u32) -> Option<DateTime<Utc>> {
    let age = chrono::Duration::from_std(crate::engine::process_age(pid)?).ok()?;
    Some(Utc::now() - age)
}

/// Whether the process now holding `pid` started after `since`: whatever
/// held the PID at `since` exited and the OS gave it to something else.
/// `false` when the age cannot be read.
fn pid_started_after(pid: u32, since: DateTime<Utc>) -> bool {
    os_start_time(pid).is_some_and(|os_start| os_start > since + PID_REUSE_TOLERANCE)
}

fn lock_timeout(id: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "Timed out after {}s waiting for the lock on process entry '{id}': another devcli \
         command or pm-daemon is still working on it. Try again in a moment.",
        ENTRY_LOCK_TIMEOUT.as_secs()
    )
}

impl ManagedProcess {
    /// Whether `stop` is taking this process down.
    pub fn is_stopping(&self) -> bool {
        self.metadata.contains_key(STOPPING_KEY)
    }

    /// Whether the live process holding `self.pid` started after we recorded
    /// ours, i.e. ours exited and the OS handed the PID to something else.
    ///
    /// Without this, `stop` (or `stop --all` on stale state) would signal an
    /// unrelated process of the user's that happened to receive the PID. When
    /// the age cannot be read the PID is assumed to be ours.
    pub fn pid_was_reused(&self) -> bool {
        pid_started_after(self.pid, self.start_time)
    }

    /// Whether the process that set the `stopping` flag is gone (killed while
    /// stopping the app), including when its PID now belongs to a process
    /// started after the stop began. Entries flagged by older versions
    /// (`true`) count as abandoned.
    pub fn stopper_gone(&self) -> bool {
        let Some(value) = self.metadata.get(STOPPING_KEY) else {
            return false;
        };
        let Ok(stopper) = value.parse::<u32>() else {
            return true;
        };
        if !crate::engine::pid_alive(stopper) {
            return true;
        }
        self.metadata
            .get(STOPPING_SINCE_KEY)
            .and_then(|since| DateTime::parse_from_rfc3339(since).ok())
            .is_some_and(|since| pid_started_after(stopper, since.with_timezone(&Utc)))
    }

    /// Whether another app depends on this owned process.
    pub fn is_shared(&self) -> bool {
        self.metadata.contains_key(SHARED_KEY)
    }

    /// Whether the process runs from a git worktree that has been deleted.
    ///
    /// Only a root reported as not found counts, not one that cannot be read
    /// for a moment. And a worktree moved with `mv` is not deleted: the
    /// process's working directory then still exists, at its new path.
    pub fn worktree_gone(&self) -> bool {
        let Some(root) = self.metadata.get(WORKTREE_ROOT_KEY) else {
            return false;
        };
        let missing = matches!(
            std::fs::symlink_metadata(root),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound
        );
        missing && !crate::engine::process_cwd(self.pid).is_some_and(|cwd| cwd.is_dir())
    }

    /// The attached `devcli start` this process lives and dies with, if any.
    pub fn owner_pid(&self) -> Option<u32> {
        self.metadata.get(OWNER_PID_KEY)?.parse().ok()
    }

    /// Whether this process has an owner that is no longer running.
    ///
    /// The owner started before the process did, so a live PID that started
    /// later belongs to something else: the owner exited and its PID was
    /// reused. When the age cannot be read the owner is assumed alive.
    pub fn owner_gone(&self) -> bool {
        let Some(owner) = self.owner_pid() else {
            return false;
        };
        if !crate::engine::pid_alive(owner) {
            return true;
        }
        pid_started_after(owner, self.start_time)
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
