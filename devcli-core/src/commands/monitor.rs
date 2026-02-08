// Monitor command - Background daemon for monitoring process health
// Runs continuously in the background, checking process status
// Automatically exits when no processes are being tracked

use crate::config::{load_config, Config};
use crate::logging::MonitorLogger;
use crate::metrics::{start_metrics_server, MetricsCollector};
use crate::process::{HealthCheckEngine, ProcessInfo, ProcessTracker, RestartCoordinator, RestartReason};
use crate::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;

// Main entry point for the monitor daemon
// This runs in a loop, checking process health every few seconds
// Args:
//   - daemon: If true, runs as a background daemon. If false, runs once and exits.
#[tracing::instrument]
pub async fn monitor_command(daemon: bool) -> Result<()> {
    if daemon {
        // Run as daemon - continuous monitoring loop
        run_daemon_loop().await
    } else {
        // Run once - just do a single cleanup
        // This is useful for manual cleanup without starting the full daemon
        let tracker = ProcessTracker::new()?;
        let cleaned = tracker.cleanup_dead()?;

        if cleaned.is_empty() {
            println!("No dead processes found");
        } else {
            println!("Cleaned up {} dead process(es):", cleaned.len());
            for app_name in cleaned {
                println!("  - {}", app_name);
            }
        }

        Ok(())
    }
}

/// Monitor state encapsulates all components needed for monitoring
struct MonitorState {
    tracker: ProcessTracker,
    config: Config,
    health_check_engine: HealthCheckEngine,
    restart_coordinator: RestartCoordinator,
    logger: MonitorLogger,
    metrics: Arc<MetricsCollector>,
}

impl MonitorState {
    /// Create new monitor state
    async fn new(metrics: Arc<MetricsCollector>) -> Result<Self> {
        let tracker = ProcessTracker::new()?;
        let config = load_config()?;
        let health_check_engine = HealthCheckEngine::new();
        let restart_coordinator = RestartCoordinator::new();
        let logger = MonitorLogger::new().await?;

        Ok(Self {
            tracker,
            config,
            health_check_engine,
            restart_coordinator,
            logger,
            metrics,
        })
    }

    /// Check if we should exit (no processes left)
    fn should_exit(&self) -> Result<bool> {
        let processes = self.tracker.list_processes()?;
        let non_monitor_processes: Vec<_> = processes
            .iter()
            .filter(|p| p.app_name != ".monitor")
            .collect();

        Ok(non_monitor_processes.is_empty())
    }

    /// Perform health checks on all running processes
    #[tracing::instrument(skip(self), name = "health_check_cycle")]
    async fn perform_health_checks(&mut self) -> Result<()> {
        let processes = self.tracker.list_processes()?;

        for process in processes {
            // Skip the monitor itself
            if process.app_name == ".monitor" {
                continue;
            }

            // Skip if process is not running
            if !self.tracker.is_running(process.pid) {
                continue;
            }

            // Get app configuration
            let project = process.project.as_deref().unwrap_or("unknown");
            let app_name = process.app_config_name.as_deref().unwrap_or(&process.app_name);

            let Some(project_config) = self.config.projects.get(project) else {
                continue;
            };

            let Some(app_config) = project_config.apps.get(app_name) else {
                continue;
            };

            // Skip if no health check configured
            let Some(ref health_check) = app_config.health_check else {
                continue;
            };

            // Execute health check
            match self.health_check_engine.check(health_check).await {
                Ok(true) => {
                    // Health check passed - reset failure count
                    if process.health_check_failures > 0 {
                        // Log recovery
                        let _ = self.logger.log_health_check_recovered(project, app_name).await;

                        let mut updated_process = process.clone();
                        updated_process.health_check_failures = 0;
                        updated_process.last_health_check = Some(Utc::now());
                        let _ = self.tracker.register_process(updated_process);
                    }
                }
                Ok(false) => {
                    // Health check failed - increment failure count
                    let mut updated_process = process.clone();
                    updated_process.health_check_failures += 1;
                    updated_process.last_health_check = Some(Utc::now());

                    // Log health check failure
                    let check_type = format!("{:?}", health_check);
                    let _ = self.logger.log_health_check_failure(
                        project,
                        app_name,
                        &check_type,
                        &format!("failure #{}", updated_process.health_check_failures),
                    ).await;

                    // If multiple failures, trigger restart
                    // TODO: Make threshold configurable (currently hardcoded to 3)
                    if updated_process.health_check_failures >= 3 {
                        tracing::warn!(
                            project = %project,
                            app = %app_name,
                            failures = updated_process.health_check_failures,
                            "Health check threshold reached, will attempt restart"
                        );

                        eprintln!(
                            "Health check failed {} times for {}/{}, will attempt restart",
                            updated_process.health_check_failures, project, app_name
                        );

                        // Record restart reason
                        updated_process.record_restart(
                            None,
                            RestartReason::HealthCheckFailure {
                                check_type: check_type.clone(),
                            },
                        );

                        // Reset failure count for next cycle
                        updated_process.health_check_failures = 0;
                    }

                    let _ = self.tracker.register_process(updated_process);
                }
                Err(e) => {
                    tracing::error!(
                        project = %project,
                        app = %app_name,
                        error = %e,
                        "Error performing health check"
                    );

                    let error_msg = format!("{}", e);
                    let check_type = format!("{:?}", health_check);
                    let _ = self.logger.log_health_check_failure(
                        project,
                        app_name,
                        &check_type,
                        &error_msg,
                    ).await;
                    eprintln!("Error performing health check for {}/{}: {}", project, app_name, e);
                }
            }
        }

        Ok(())
    }

    /// Handle crashed processes and trigger restarts if configured
    #[tracing::instrument(skip(self), name = "handle_crashed")]
    async fn handle_crashed_processes(&mut self) -> Result<()> {
        let processes = self.tracker.list_processes()?;

        for process in processes {
            // Skip the monitor itself
            if process.app_name == ".monitor" {
                continue;
            }

            // Check if process has recently exited
            let Some(exit_code) = process.last_exit_code else {
                continue;
            };

            // Check if process is still running (might have been restarted manually)
            if self.tracker.is_running(process.pid) {
                continue;
            }

            // Get app configuration
            let project = process.project.as_deref().unwrap_or("unknown");
            let app_name = process.app_config_name.as_deref().unwrap_or(&process.app_name);

            let Some(project_config) = self.config.projects.get(project) else {
                continue;
            };

            let Some(app_config) = project_config.apps.get(app_name) else {
                continue;
            };

            // Check if restart policy is configured
            let Some(ref restart_policy) = app_config.restart_policy else {
                continue;
            };

            // Check if we should restart
            if !process.should_restart(restart_policy) {
                // Max restarts reached or other policy violation
                if restart_policy.max_restarts > 0 && process.restart_count >= restart_policy.max_restarts {
                    let _ = self.logger.log_max_restarts_reached(
                        project,
                        app_name,
                        restart_policy.max_restarts,
                        restart_policy.restart_window_secs,
                    ).await;
                }
                continue;
            }

            // Calculate backoff
            let backoff = process.calculate_backoff(restart_policy);

            // Log restart trigger
            let reason = RestartReason::Crash { exit_code };
            let _ = self.logger.log_restart_triggered(
                project,
                app_name,
                &reason,
                backoff.as_secs(),
                process.restart_count + 1,
            ).await;

            // Trigger restart via coordinator
            let app_key = format!("{}:{}:{}", project, app_name, process.environment.as_deref().unwrap_or("local"));

            tracing::warn!(
                project = %project,
                app = %app_name,
                exit_code = exit_code,
                backoff_secs = backoff.as_secs(),
                restart_count = process.restart_count + 1,
                "Process crashed, triggering restart"
            );

            eprintln!(
                "Process {}/{} crashed with exit code {}. Restarting after {:?}...",
                project, app_name, exit_code, backoff
            );

            // Execute restart (this will be implemented in a future commit)
            // For now, just log that we would restart
            let _result = self.restart_coordinator.execute_restart_if_allowed(
                &app_key,
                backoff,
                async {
                    // TODO: Implement actual restart logic
                    // This should call the restart command with proper arguments
                    tracing::info!(
                        project = %project,
                        app = %app_name,
                        "Executing restart (TODO: implement)"
                    );
                    eprintln!("TODO: Execute restart for {}/{}", project, app_name);
                    Ok(())
                },
            )
            .await;
        }

        Ok(())
    }

    /// Cleanup and exit
    async fn cleanup_and_exit(&mut self) -> Result<()> {
        let _ = self.logger.log_monitor_stopped("no processes remaining").await;
        self.tracker.remove_process("unknown", ".monitor", None)?;
        Ok(())
    }
}

// Background daemon loop
// Continuously monitors process health and cleans up dead processes
// Exits automatically when no processes remain
#[tracing::instrument(name = "monitor_daemon")]
async fn run_daemon_loop() -> Result<()> {
    // Create metrics collector
    let metrics = Arc::new(MetricsCollector::new());

    // Start metrics HTTP server in background
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        if let Err(e) = start_metrics_server(metrics_clone).await {
            tracing::error!(error = %e, "Metrics server error");
            eprintln!("Metrics server error: {}", e);
        }
    });

    let mut state = MonitorState::new(metrics.clone()).await?;
    let tracker = &state.tracker;

    tracing::info!("Monitor daemon starting");

    // Log monitor startup
    let _ = state.logger.log_monitor_started().await;

    // Register the monitor itself as a tracked process
    // This allows other parts of the system to check if the monitor is running
    // We use a special name: ".monitor"
    let monitor_info = ProcessInfo {
        app_name: ".monitor".to_string(),
        pid: std::process::id(),
        command: "devcli monitor --daemon".to_string(),
        working_dir: std::env::current_dir()?.to_string_lossy().to_string(),
        start_time: Utc::now(),
        env_vars: HashMap::new(),
        project: None,
        app_config_name: Some(".monitor".to_string()),
        environment: None,
        command_variant: None,
        stage: None, // Monitor process doesn't have a stage
        restart_count: 0,
        restart_history: Vec::new(),
        last_exit_code: None,
        last_exit_time: None,
        health_check_failures: 0,
        last_health_check: None,
    };

    tracker.register_process(monitor_info)?;

    // Main monitoring loop
    loop {
        // Increment loop iteration counter for metrics
        state.metrics.increment_loop_iteration();

        // 1. Cleanup dead processes (existing behavior)
        let _cleaned = state.tracker.cleanup_dead()?;

        // 2. Exit if no processes left
        if state.should_exit()? {
            state.cleanup_and_exit().await?;
            break;
        }

        // 3. Perform health checks on running processes
        if let Err(e) = state.perform_health_checks().await {
            tracing::error!(error = %e, "Error performing health checks");
            eprintln!("Error performing health checks: {}", e);
        }

        // 4. Handle crashed processes and trigger restarts
        if let Err(e) = state.handle_crashed_processes().await {
            tracing::error!(error = %e, "Error handling crashed processes");
            eprintln!("Error handling crashed processes: {}", e);
        }

        // 5. Sleep for 3 seconds before next iteration
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }

    Ok(())
}
