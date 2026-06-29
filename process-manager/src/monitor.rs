use crate::engine;
use crate::health::HealthCheckEngine;
use crate::restart::RestartCoordinator;
use crate::state::StateStore;
use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

pub struct Monitor {
    state: Arc<StateStore>,
    health_engine: Arc<HealthCheckEngine>,
    restart_coordinator: Arc<RestartCoordinator>,
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
        }
    }

    pub async fn run(&self) -> Result<()> {
        tracing::info!("Starting process monitor loop");

        loop {
            if !self.tick().await? {
                break;
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
            let is_alive = self.state.is_running(&proc);

            if !is_alive {
                tracing::warn!(id = %proc.id, "Process detected as dead");
                self.handle_failure(&mut proc, "crash".to_string()).await?;
                continue;
            }

            let health_passed = self.health_engine.check(&proc.task.health_check).await?;

            if !health_passed {
                proc.runtime.health_failures += 1;
                proc.runtime.last_health_check = Some(Utc::now());
                self.state.save(&proc)?;

                if proc.runtime.health_failures >= 3 {
                    tracing::warn!(id = %proc.id, "Process unhealthy (3 consecutive failures), triggering restart");
                    self.handle_failure(&mut proc, "health_check".to_string())
                        .await?;
                }
            } else if proc.runtime.health_failures > 0 {
                proc.runtime.health_failures = 0;
                proc.runtime.last_health_check = Some(Utc::now());
                self.state.save(&proc)?;
            }
        }

        Ok(true)
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
                    tracing::info!(id = %id, reason = %reason, "Executing automatic restart...");

                    let result = engine::restart(
                        &task,
                        old_pid,
                        old_pgid,
                        Duration::from_secs(0),
                    )
                    .await?;

                    // For background restarts, we drop the output_rx as logs are already being written to file
                    drop(result.output_rx);

                    if let Some(mut updated_proc) = state.load(&id)? {
                        updated_proc.pid = result.pid;
                        updated_proc.pgid = result.pgid;
                        updated_proc.runtime.restart_count += 1;
                        updated_proc.runtime.health_failures = 0;
                        updated_proc.runtime.history.push(crate::state::RestartEvent {
                            timestamp: Utc::now(),
                            reason: format!("Auto-restart due to {}", reason),
                        });
                        state.save(&updated_proc)?;
                    }
                    Ok(())
                })
                .await;
            });
        } else {
            tracing::error!(id = %proc.id, "Max restarts reached or policy prevents restart. Deleting state.");
            let _ = engine::terminate(proc.pid, proc.pgid, true).await;
            self.state.delete(&proc.id)?;
        }

        Ok(())
    }
}
