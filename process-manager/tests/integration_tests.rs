//! End-to-end integration tests for process-manager.
//!
//! Uses isolated temp directories and real subprocesses. Unix-only.

use chrono::Utc;
use process_manager::engine;
use process_manager::state::{ManagedProcess, ProcessRuntime, StateStore};
use process_manager::{HealthCheck, RestartPolicy, Task};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

fn sleep_task(id: &str, seconds: u64, log_file: Option<PathBuf>) -> Task {
    Task {
        id: id.to_string(),
        command: format!("sleep {}", seconds),
        args: vec![],
        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
        env: HashMap::new(),
        is_detached: true,
        log_file,
        health_check: HealthCheck::Process {},
        restart_policy: RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    }
}

fn managed_from_running(
    task: Task,
    pid: u32,
    pgid: Option<i32>,
    metadata: HashMap<String, String>,
) -> ManagedProcess {
    ManagedProcess {
        id: task.id.clone(),
        pid,
        pgid,
        task,
        start_time: Utc::now(),
        metadata,
        runtime: ProcessRuntime::default(),
    }
}

async fn wait_for_log_content(path: &Path, needle: &str) {
    for _ in 0..20 {
        if let Ok(content) = tokio::fs::read_to_string(path).await {
            if content.contains(needle) {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "log file {} did not contain {:?} in time",
        path.display(),
        needle
    );
}

#[tokio::test]
#[cfg(unix)]
async fn test_spawn_and_terminate_detached_process() {
    let task = sleep_task("spawn-terminate", 120, None);
    let running = engine::spawn(&task).await.expect("spawn sleep");
    assert!(running.is_alive());
    assert!(running.pgid.is_some(), "detached spawn must set PGID");
    assert_eq!(running.pgid, Some(running.pid as i32));
    assert!(
        running.child.is_none(),
        "detached spawn must not retain Child handle"
    );

    let terminated = engine::terminate(running.pid, running.pgid, true)
        .await
        .expect("terminate");
    assert!(terminated);

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!running.is_alive());
}

#[tokio::test]
#[cfg(unix)]
async fn test_spawn_parses_quoted_shell_words() {
    let task = Task {
        id: "quoted-args".to_string(),
        command: r#"echo "hello world""#.to_string(),
        args: vec![],
        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
        env: HashMap::new(),
        is_detached: false,
        log_file: None,
        health_check: HealthCheck::Process {},
        restart_policy: RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    };

    let mut running = engine::spawn(&task).await.expect("spawn echo");
    assert!(running.is_alive());

    let mut output = String::new();
    if let Some(child) = running.child.as_mut() {
        let _ = child.wait().await;
    }

    for _ in 0..20 {
        while let Ok(msg) = running.output_rx.try_recv() {
            output.push_str(&msg.content);
        }
        if output.contains("hello world") {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    panic!("expected quoted argument in output, got: {output:?}");
}

#[tokio::test]
#[cfg(unix)]
async fn test_terminate_sigterm_before_force() {
    let task = sleep_task("sigterm-target", 120, None);
    let running = engine::spawn(&task).await.expect("spawn sleep");
    assert!(running.is_alive());

    let sent = engine::terminate(running.pid, running.pgid, false)
        .await
        .expect("SIGTERM");
    assert!(sent);

    for _ in 0..40 {
        if !running.is_alive() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    panic!("process should exit after SIGTERM");
}

#[tokio::test]
#[cfg(unix)]
async fn test_spawn_attached_process_keeps_child_handle() {
    let task = Task {
        id: "attached".to_string(),
        command: "sleep 120".to_string(),
        args: vec![],
        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
        env: HashMap::new(),
        is_detached: false,
        log_file: None,
        health_check: HealthCheck::Process {},
        restart_policy: RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    };

    let mut running = engine::spawn(&task).await.expect("spawn attached sleep");
    assert!(running.is_alive());
    assert!(running.pgid.is_none(), "attached spawn must not set PGID");
    assert!(running.child.is_some(), "attached spawn must retain Child handle");

    drop(running.output_rx);
    let _ = engine::terminate(running.pid, running.pgid, true).await;
}

#[tokio::test]
#[cfg(unix)]
async fn test_spawn_streams_to_log_file() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("output.log");

    let task = Task {
        id: "log-stream".to_string(),
        command: "sh -c \"echo OUTLINE; echo ERRLINE >&2\"".to_string(),
        args: vec![],
        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
        env: HashMap::new(),
        is_detached: false,
        log_file: Some(log_path.clone()),
        health_check: HealthCheck::Process {},
        restart_policy: RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    };

    let mut running = engine::spawn(&task).await.expect("spawn sh");
    assert!(running.is_alive());

    // Drain output channel so handlers complete
    while running.output_rx.try_recv().is_ok() {}

    if let Some(child) = running.child.as_mut() {
        let _ = child.wait().await;
    }

    wait_for_log_content(&log_path, "OUTLINE").await;
    wait_for_log_content(&log_path, "ERRLINE").await;

    let content = tokio::fs::read_to_string(&log_path).await.unwrap();
    assert!(content.contains("[STDOUT]"));
    assert!(content.contains("[STDERR]"));
}

#[tokio::test]
#[cfg(unix)]
async fn test_restart_replaces_process() {
    let task = sleep_task("restart-me", 120, None);
    let running = engine::spawn(&task).await.expect("spawn");
    let old_pid = running.pid;
    let old_pgid = running.pgid;
    drop(running.output_rx);

    let restarted = engine::restart(&task, old_pid, old_pgid, Duration::from_millis(50))
        .await
        .expect("restart");
    let new_pid = restarted.pid;
    let new_pgid = restarted.pgid;

    assert_ne!(new_pid, old_pid);
    assert!(restarted.is_alive());
    drop(restarted.output_rx);

    let _ = engine::terminate(new_pid, new_pgid, true).await;
}

#[tokio::test]
#[cfg(unix)]
async fn test_state_store_cleanup_dead() {
    let temp_dir = TempDir::new().unwrap();
    let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

    let dead = ManagedProcess {
        id: "dead-app".to_string(),
        pid: 2_000_000_000,
        pgid: None,
        task: sleep_task("dead-app", 1, None),
        start_time: Utc::now(),
        metadata: HashMap::new(),
        runtime: ProcessRuntime::default(),
    };
    store.save(&dead).unwrap();

    let cleaned = store.cleanup_dead().unwrap();
    assert_eq!(cleaned, vec!["dead-app".to_string()]);
    assert!(store.load("dead-app").unwrap().is_none());
}

#[tokio::test]
#[cfg(unix)]
async fn test_state_store_tracks_live_process() {
    let temp_dir = TempDir::new().unwrap();
    let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

    let task = sleep_task("tracked-app", 120, None);
    let running = engine::spawn(&task).await.expect("spawn");
    let pid = running.pid;
    let pgid = running.pgid;
    drop(running.output_rx);

    let mut metadata = HashMap::new();
    metadata.insert("project".to_string(), "test-project".to_string());
    metadata.insert("app_config_name".to_string(), "tracked-app".to_string());

    let managed = managed_from_running(task, pid, pgid, metadata);
    store.save(&managed).unwrap();

    let loaded = store.load("tracked-app").unwrap().expect("saved process");
    assert!(store.is_running(&loaded));

    let _ = engine::terminate(loaded.pid, loaded.pgid, true).await;
    store.cleanup_dead().unwrap();
    assert!(store.load("tracked-app").unwrap().is_none());
}

#[tokio::test]
#[cfg(unix)]
async fn test_ensure_daemon_running_singleton() {
    let temp_dir = TempDir::new().unwrap();
    let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

    let Some(daemon_dir) = locate_pm_daemon_dir() else {
        eprintln!("Skipping daemon test: pm-daemon binary not built yet");
        return;
    };

    let original_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", daemon_dir.display(), original_path));

    let task = sleep_task("daemon-app", 120, None);
    let running = engine::spawn(&task).await.expect("spawn");
    let pid = running.pid;
    let pgid = running.pgid;
    drop(running.output_rx);

    store
        .save(&managed_from_running(task.clone(), pid, pgid, HashMap::new()))
        .unwrap();

    store.ensure_daemon_running().expect("first daemon spawn");
    store.ensure_daemon_running().expect("second call is no-op");

    let lock_path = temp_dir.path().join(".daemon.lock");
    assert!(lock_path.exists());

    let _ = engine::terminate(pid, pgid, true).await;
    store.delete("daemon-app").unwrap();

    // Daemon should exit once the store is empty
    for _ in 0..30 {
        if !daemon_lock_held(&lock_path) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    assert!(
        !daemon_lock_held(&lock_path),
        "pm-daemon should release lock when state store is empty"
    );
}

fn locate_pm_daemon_dir() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent()?;
    for profile in ["debug", "release"] {
        let candidate = workspace_root.join("target").join(profile).join("pm-daemon");
        if candidate.exists() {
            return candidate.parent().map(Path::to_path_buf);
        }
    }
    None
}

fn daemon_lock_held(lock_path: &Path) -> bool {
    use fs2::FileExt;
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
    {
        Ok(f) => f,
        Err(_) => return false,
    };

    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = file.unlock();
            false
        }
        Err(_) => true,
    }
}
