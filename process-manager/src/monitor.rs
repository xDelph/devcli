use crate::engine;
use crate::health::HealthCheckEngine;
use crate::restart::RestartCoordinator;
use crate::state::StateStore;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

/// How long after a (re)start health-check failures are not counted.
///
/// A dev server routinely needs longer than three 3-second ticks to start
/// listening; without a grace period an app with an HTTP check was restarted
/// over and over, then SIGKILLed for good once `max_restarts` was reached.
pub const DEFAULT_HEALTH_GRACE: Duration = Duration::from_secs(30);

pub struct Monitor {
    state: Arc<StateStore>,
    health_engine: Arc<HealthCheckEngine>,
    restart_coordinator: Arc<RestartCoordinator>,
    health_grace: Duration,
}

impl Monitor {
    pub fn new(
        state: Arc<StateStore>,
        health_engine: Arc<HealthCheckEngine>,
        restart_coordinator: Arc<RestartCoordinator>,
    ) -> Self {
        Self {
            state,
            health_engine,
            restart_coordinator,
            health_grace: DEFAULT_HEALTH_GRACE,
        }
    }

    /// Override the start-up grace period for health checks.
    pub fn with_health_grace(mut self, grace: Duration) -> Self {
        self.health_grace = grace;
        self
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Starting process monitor loop");

        loop {
            match self.tick().await {
                Ok(true) => {}
                Ok(false) => break,
                // A transient failure (a file mid-replace, a full disk, a lock
                // error) used to end the daemon and with it all monitoring. Only
                // give up when the state directory itself is gone.
                Err(e) if self.state.base_dir().exists() => {
                    tracing::warn!(error = %e, "Monitor tick failed, retrying");
                }
                Err(e) => return Err(e),
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }

        Ok(())
    }

    /// Run one monitor cycle over all managed processes.
    ///
    /// Returns `false` when the state store is empty and the monitor loop should stop.
    pub async fn tick(&self) -> Result<bool> {
        let processes = self.state.list()?;

        if processes.is_empty() {
            tracing::info!("No processes remaining, stopping monitor");
            return Ok(false);
        }

        for mut proc in processes {
            if proc.is_stopping() {
                // `stop` owns this entry until it deletes it, unless whoever
                // was stopping it died midway: finish the job, or the app
                // would run untracked forever.
                if proc.stopper_gone()
                    && let Err(e) = self.stop_orphan(&proc).await
                {
                    tracing::warn!(id = %proc.id, error = %e, "Failed to finish an abandoned stop");
                }
                continue;
            }

            if proc.owner_gone() && proc.is_shared() && !proc.worktree_gone() {
                // Its owner is gone, but another app depends on it.
                tracing::info!(id = %proc.id, "Owner exited; releasing a shared process");
                if let Err(e) = self.state.release(&proc.id) {
                    tracing::warn!(id = %proc.id, error = %e, "Failed to release process");
                }
                continue;
            }

            if proc.owner_gone() || proc.worktree_gone() {
                // The attached `devcli start` that owned it exited without
                // stopping it (killed, terminal closed), or the git worktree
                // it runs from was deleted.
                if let Err(e) = self.stop_orphan(&proc).await {
                    tracing::warn!(id = %proc.id, error = %e, "Failed to stop orphaned process");
                }
                continue;
            }

            // One misbehaving entry must not stop the others from being checked.
            if let Err(e) = self.check_process(&mut proc).await {
                tracing::warn!(id = %proc.id, error = %e, "Failed to check process");
            }
        }

        Ok(true)
    }

    /// Stop a process whose owner or worktree is gone: SIGTERM, then SIGKILL
    /// after 5s.
    async fn stop_orphan(&self, proc: &crate::state::ManagedProcess) -> Result<()> {
        tracing::info!(
            id = %proc.id,
            owner = ?proc.owner_pid(),
            worktree_gone = proc.worktree_gone(),
            "Orphaned process, stopping it"
        );
        let Some(current) = self.state.mark_stopping(&proc.id)? else {
            return Ok(());
        };

        engine::terminate(current.pid, current.pgid, false).await?;
        for _ in 0..50 {
            if !self.state.is_running(&current) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if self.state.is_running(&current) {
            engine::terminate(current.pid, current.pgid, true).await?;
        }

        self.state.delete(&current.id)
    }

    async fn check_process(&self, proc: &mut crate::state::ManagedProcess) -> Result<()> {
        if let Some(log) = &proc.task.log_file
            && let Err(e) = engine::rotate_log_if_large(log, engine::MAX_LOG_BYTES)
        {
            tracing::warn!(id = %proc.id, error = %e, "Failed to rotate log file");
        }

        if !self.state.is_running(proc) {
            tracing::warn!(id = %proc.id, "Process detected as dead");
            return self.handle_failure(proc, "crash".to_string()).await;
        }

        let age = (Utc::now() - proc.start_time).to_std().unwrap_or_default();
        if age < self.health_grace {
            return Ok(());
        }

        let health_passed = self.health_engine.check(&proc.task.health_check).await?;

        if !health_passed {
            proc.runtime.health_failures += 1;
            proc.runtime.last_health_check = Some(Utc::now());
            self.state.save(proc)?;

            if proc.runtime.health_failures >= 3 {
                tracing::warn!(id = %proc.id, "Process unhealthy (3 consecutive failures), triggering restart");
                self.handle_failure(proc, "health_check".to_string())
                    .await?;
            }
        } else if proc.runtime.health_failures > 0 {
            proc.runtime.health_failures = 0;
            proc.runtime.last_health_check = Some(Utc::now());
            self.state.save(proc)?;
        }

        Ok(())
    }

    async fn handle_failure(
        &self,
        proc: &mut crate::state::ManagedProcess,
        reason: String,
    ) -> Result<()> {
        let policy = proc.task.restart_policy.clone();

        if proc.should_restart(&policy) {
            let backoff = proc.calculate_backoff(&policy);
            let id = proc.id.clone();
            let task = proc.task.clone();
            let old_pid = proc.pid;
            let old_pgid = proc.pgid;

            let coordinator = self.restart_coordinator.clone();
            let state = self.state.clone();

            tokio::spawn(async move {
                let _ = coordinator.execute_if_allowed(&id, backoff, || async {
                    // Re-check under the entry lock: the backoff can last a minute,
                    // and `stop` takes the same lock before it signals anything.
                    let _guard = state.lock_entry(&id)?;
                    let Some(current) = state.load(&id)? else {
                        tracing::info!(id = %id, "Entry removed during backoff, not restarting");
                        return Ok(());
                    };
                    if current.is_stopping() || current.pid != old_pid {
                        tracing::info!(id = %id, "Entry stopped or already restarted, not restarting");
                        return Ok(());
                    }

                    tracing::info!(id = %id, reason = %reason, "Executing automatic restart...");

                    // Only an unhealthy-but-alive process needs killing, and only if
                    // its PID still belongs to it: after a crash plus a long backoff
                    // the OS may have handed `old_pid` to an unrelated process.
                    if state.is_running(&current) {
                        let _ = engine::terminate(old_pid, old_pgid, true).await;
                    }

                    let mut result = engine::spawn_supervised(&task).await?;
                    // Output goes straight to the log file; nobody reads the channel.
                    drop(result.output_rx);
                    let child = result.child.take();

                    let mut updated_proc = current;
                    updated_proc.pid = result.pid;
                    updated_proc.pgid = result.pgid;
                    // A new process: uptime and PID-reuse detection are measured
                    // from here.
                    updated_proc.start_time = Utc::now();
                    updated_proc.runtime.restart_count += 1;
                    updated_proc.runtime.health_failures = 0;
                    updated_proc.runtime.last_exit_code = None;
                    updated_proc.runtime.last_exit_time = None;
                    updated_proc.runtime.history.push(crate::state::RestartEvent {
                        timestamp: Utc::now(),
                        reason: format!("Auto-restart due to {}", reason),
                    });
                    state.save(&updated_proc)?;

                    // We are this process's parent: wait for it so its exit code
                    // reaches the restart policy (`restart_on_exit_codes`, "a clean
                    // exit is not a crash"). The first instance, started by the
                    // CLI, has no surviving parent, so its exit code stays unknown.
                    if let Some(child) = child {
                        watch_exit(Arc::clone(&state), id.clone(), result.pid, child);
                    }
                    Ok(())
                })
                .await;
            });
        } else {
            tracing::error!(id = %proc.id, "Max restarts reached or policy prevents restart. Deleting state.");
            if self.state.is_running(proc) {
                let _ = engine::terminate(proc.pid, proc.pgid, true).await;
            }
            self.state.delete(&proc.id)?;
        }

        Ok(())
    }
}

/// Record the exit status of a child we spawned in its state entry.
fn watch_exit(state: Arc<StateStore>, id: String, pid: u32, mut child: tokio::process::Child) {
    tokio::spawn(async move {
        let Ok(status) = child.wait().await else {
            return;
        };
        let code = exit_code(&status);
        let Ok(_guard) = state.lock_entry(&id) else {
            return;
        };
        if let Ok(Some(mut proc)) = state.load(&id) {
            // Only if the entry still describes this process.
            if proc.pid == pid {
                proc.runtime.last_exit_code = code;
                proc.runtime.last_exit_time = Some(Utc::now());
                let _ = state.save(&proc);
            }
        }
    });
}

/// Exit code, or 128 + signal number for a signal death (shell convention).
fn exit_code(status: &std::process::ExitStatus) -> Option<i32> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status
            .code()
            .or_else(|| status.signal().map(|sig| 128 + sig))
    }
    #[cfg(not(unix))]
    {
        status.code()
    }
}
