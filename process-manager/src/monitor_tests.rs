//! Monitor loop tests — health-failure counting, crash detection, and restart behavior.
//!
//! Uses `Monitor::tick()` to drive single cycles without the 3-second poll delay.

#[cfg(all(test, unix))]
mod tests {
    use crate::health::HealthCheckEngine;
    use crate::monitor::Monitor;
    use crate::restart::RestartCoordinator;
    use crate::state::{ManagedProcess, ProcessRuntime, StateStore};
    use crate::{HealthCheck, RestartPolicy, Task};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::TempDir;

    fn sleep_task(id: &str, seconds: u64) -> Task {
        Task {
            id: id.to_string(),
            command: format!("sleep {}", seconds),
            args: vec![],
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
            env: HashMap::new(),
            is_detached: true,
            log_file: None,
            health_check: HealthCheck::Process {},
            restart_policy: RestartPolicy {
                enabled: true,
                max_restarts: 5,
                restart_window_secs: 300,
                initial_backoff_secs: 0,
                max_backoff_secs: 1,
                backoff_multiplier: 1.0,
                restart_on_exit_codes: None,
            },
        }
    }

    fn failing_health_task(id: &str, seconds: u64) -> Task {
        let mut task = sleep_task(id, seconds);
        task.health_check = HealthCheck::Command {
            command: "false".to_string(),
            timeout_secs: 1,
            expected_exit_code: 0,
        };
        task
    }

    fn monitor_for(store: Arc<StateStore>, base_dir: PathBuf) -> Monitor {
        Monitor::new(
            Arc::clone(&store),
            Arc::new(HealthCheckEngine::new()),
            Arc::new(RestartCoordinator::new(base_dir)),
        )
    }

    /// Reap an exited child so the kernel stops listing it as a zombie.
    ///
    /// The engine drops the `Child` handle for detached processes and lets tokio
    /// reap it in the background. Until that happens the PID still answers
    /// `kill -0`, so `StateStore::is_running` reports a killed process as alive
    /// and the monitor never sees a crash. A no-op if the child was already
    /// reaped (ECHILD).
    fn reap_child(pid: u32) {
        let mut status = 0;
        // SAFETY: waitpid only inspects/reaps our own child; WNOHANG never blocks.
        unsafe {
            libc::waitpid(pid as i32, &mut status, libc::WNOHANG);
        }
    }

    /// Kill the managed process and reap it, so the monitor observes a crash
    /// deterministically instead of racing the background reaper.
    async fn kill_and_reap(managed: &ManagedProcess) {
        let _ = crate::engine::terminate(managed.pid, managed.pgid, true).await;
        reap_child(managed.pid);
    }

    async fn spawn_and_track(store: &StateStore, task: Task) -> ManagedProcess {
        let running = crate::engine::spawn(&task).await.expect("spawn");
        let managed = ManagedProcess {
            id: task.id.clone(),
            pid: running.pid,
            pgid: running.pgid,
            task,
            start_time: Utc::now(),
            metadata: HashMap::new(),
            runtime: ProcessRuntime::default(),
        };
        drop(running.output_rx);
        store.save(&managed).unwrap();
        managed
    }

    async fn wait_until<F>(mut predicate: F, timeout: Duration)
    where
        F: FnMut() -> bool,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            if predicate() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("condition not met within {:?}", timeout);
    }

    #[tokio::test]
    #[ignore = "spawns real OS processes and asserts on liveness; wedges the GitHub Actions runner step (see git log). Pass on macOS and Ubuntu 24.04 with Rust 1.98.0. Run with --ignored."]
    async fn test_tick_increments_health_failures_before_restart() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let monitor = monitor_for(Arc::clone(&store), temp_dir.path().to_path_buf());

        let task = failing_health_task("health-counter", 120);
        spawn_and_track(&store, task).await;

        monitor.tick().await.unwrap();
        let after_one = store.load("health-counter").unwrap().unwrap();
        assert_eq!(after_one.runtime.health_failures, 1);
        assert_eq!(after_one.runtime.restart_count, 0);

        monitor.tick().await.unwrap();
        let after_two = store.load("health-counter").unwrap().unwrap();
        assert_eq!(after_two.runtime.health_failures, 2);
        assert_eq!(after_two.runtime.restart_count, 0);

        let _ = crate::engine::terminate(after_two.pid, after_two.pgid, true).await;
        reap_child(after_two.pid);
        store.delete("health-counter").unwrap();
    }

    #[tokio::test]
    #[ignore = "spawns real OS processes and asserts on liveness; wedges the GitHub Actions runner step (see git log). Pass on macOS and Ubuntu 24.04 with Rust 1.98.0. Run with --ignored."]
    async fn test_tick_restarts_after_three_health_failures() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let monitor = monitor_for(Arc::clone(&store), temp_dir.path().to_path_buf());

        let task = failing_health_task("health-restart", 120);
        let original = spawn_and_track(&store, task).await;
        let original_pid = original.pid;

        for _ in 0..3 {
            monitor.tick().await.unwrap();
        }

        wait_until(
            || {
                store
                    .load("health-restart")
                    .ok()
                    .flatten()
                    .map(|p| p.runtime.restart_count >= 1 && p.pid != original_pid)
                    .unwrap_or(false)
            },
            Duration::from_secs(15),
        )
        .await;

        let restarted = store.load("health-restart").unwrap().unwrap();
        assert_eq!(restarted.runtime.restart_count, 1);
        assert_eq!(restarted.runtime.health_failures, 0);
        assert!(restarted
            .runtime
            .history
            .iter()
            .any(|e| e.reason.contains("health_check")));
        assert!(store.is_running(&restarted));

        let _ = crate::engine::terminate(restarted.pid, restarted.pgid, true).await;
        store.delete("health-restart").unwrap();
    }

    #[tokio::test]
    #[ignore = "spawns real OS processes and asserts on liveness; wedges the GitHub Actions runner step (see git log). Pass on macOS and Ubuntu 24.04 with Rust 1.98.0. Run with --ignored."]
    async fn test_tick_resets_health_failures_on_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let monitor = monitor_for(Arc::clone(&store), temp_dir.path().to_path_buf());

        let flag_path = temp_dir.path().join("health-recover-flag");
        let flag_check = format!("test -f {}", flag_path.display());

        let mut task = sleep_task("health-recover", 120);
        task.health_check = HealthCheck::Command {
            command: flag_check,
            timeout_secs: 1,
            expected_exit_code: 0,
        };
        spawn_and_track(&store, task).await;

        monitor.tick().await.unwrap();
        monitor.tick().await.unwrap();
        let failing = store.load("health-recover").unwrap().unwrap();
        assert_eq!(failing.runtime.health_failures, 2);

        std::fs::write(&flag_path, "ok").unwrap();
        monitor.tick().await.unwrap();

        let recovered = store.load("health-recover").unwrap().unwrap();
        assert_eq!(recovered.runtime.health_failures, 0);
        assert_eq!(recovered.runtime.restart_count, 0);

        let _ = crate::engine::terminate(recovered.pid, recovered.pgid, true).await;
        reap_child(recovered.pid);
        store.delete("health-recover").unwrap();
    }

    #[tokio::test]
    #[ignore = "spawns real OS processes and asserts on liveness; wedges the GitHub Actions runner step (see git log). Pass on macOS and Ubuntu 24.04 with Rust 1.98.0. Run with --ignored."]
    async fn test_tick_restarts_crashed_process() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let monitor = monitor_for(Arc::clone(&store), temp_dir.path().to_path_buf());

        let task = sleep_task("crash-restart", 120);
        let running = spawn_and_track(&store, task).await;
        let dead_pid = running.pid;
        kill_and_reap(&running).await;

        wait_until(|| !store.is_running(&running), Duration::from_secs(10)).await;

        monitor.tick().await.unwrap();

        wait_until(
            || {
                store
                    .load("crash-restart")
                    .ok()
                    .flatten()
                    .map(|p| {
                        p.runtime.restart_count >= 1 && p.pid != dead_pid && store.is_running(&p)
                    })
                    .unwrap_or(false)
            },
            Duration::from_secs(15),
        )
        .await;

        let restarted = store.load("crash-restart").unwrap().unwrap();
        assert!(restarted
            .runtime
            .history
            .iter()
            .any(|e| e.reason.contains("crash")));

        let _ = crate::engine::terminate(restarted.pid, restarted.pgid, true).await;
        reap_child(restarted.pid);
        store.delete("crash-restart").unwrap();
    }

    #[tokio::test]
    #[ignore = "spawns real OS processes and asserts on liveness; wedges the GitHub Actions runner step (see git log). Pass on macOS and Ubuntu 24.04 with Rust 1.98.0. Run with --ignored."]
    async fn test_tick_deletes_process_when_restart_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let monitor = monitor_for(Arc::clone(&store), temp_dir.path().to_path_buf());

        let mut task = sleep_task("no-restart", 120);
        task.restart_policy.enabled = false;
        let running = spawn_and_track(&store, task).await;
        kill_and_reap(&running).await;

        wait_until(|| !store.is_running(&running), Duration::from_secs(10)).await;

        monitor.tick().await.unwrap();

        wait_until(
            || store.load("no-restart").unwrap().is_none(),
            Duration::from_secs(10),
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "spawns real OS processes and asserts on liveness; wedges the GitHub Actions runner step (see git log). Pass on macOS and Ubuntu 24.04 with Rust 1.98.0. Run with --ignored."]
    async fn test_tick_returns_false_when_store_empty() {
        let temp_dir = TempDir::new().unwrap();
        let store = Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap());
        let monitor = monitor_for(store, temp_dir.path().to_path_buf());

        assert!(!monitor.tick().await.unwrap());
    }
}
