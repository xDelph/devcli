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

async fn wait_until_not_running(store: &StateStore, process: &ManagedProcess, timeout: Duration) {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if !store.is_running(process) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "process {} (pid {}) still reported as running after {:?}",
        process.id, process.pid, timeout
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

    for _ in 0..80 {
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

    let running = engine::spawn(&task).await.expect("spawn attached sleep");
    assert!(running.is_alive());
    assert!(running.pgid.is_none(), "attached spawn must not set PGID");
    assert!(
        running.child.is_some(),
        "attached spawn must retain Child handle"
    );

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

    // Attached spawn: no PGID, and we can reap the child so cleanup_dead does not
    // see a zombie or a recycled PGID from another detached test process.
    let mut task = sleep_task("tracked-app", 120, None);
    task.is_detached = false;
    let mut running = engine::spawn(&task).await.expect("spawn");
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
    if let Some(mut child) = running.child.take() {
        let _ = child.wait().await;
    }
    wait_until_not_running(&store, &loaded, Duration::from_secs(2)).await;

    let cleaned = store.cleanup_dead().unwrap();
    assert_eq!(cleaned, vec!["tracked-app".to_string()]);
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
    std::env::set_var(
        "PATH",
        format!("{}:{}", daemon_dir.display(), original_path),
    );

    let task = sleep_task("daemon-app", 120, None);
    let running = engine::spawn(&task).await.expect("spawn");
    let pid = running.pid;
    let pgid = running.pgid;
    drop(running.output_rx);

    store
        .save(&managed_from_running(
            task.clone(),
            pid,
            pgid,
            HashMap::new(),
        ))
        .unwrap();

    store.ensure_daemon_running().expect("first daemon spawn");
    store.ensure_daemon_running().expect("second call is no-op");

    let lock_path = temp_dir.path().join(".daemon.lock");
    assert!(lock_path.exists());

    // The daemon must lead its own process group (setsid), so a Ctrl+C sent to
    // the terminal that launched devcli cannot reach it.
    let daemon_pid = find_daemon_pid(temp_dir.path())
        .await
        .expect("pm-daemon running");
    let daemon_pgid = std::process::Command::new("ps")
        .args(["-o", "pgid=", "-p", &daemon_pid.to_string()])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    assert_eq!(
        daemon_pgid,
        daemon_pid.to_string(),
        "pm-daemon shares our process group"
    );

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

async fn find_daemon_pid(state_dir: &Path) -> Option<u32> {
    let needle = format!("--state-dir {}", state_dir.display());
    for _ in 0..30 {
        let output = std::process::Command::new("ps")
            .args(["-eo", "pid=,command="])
            .output()
            .ok()?;
        let found = String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|l| l.contains("pm-daemon") && l.contains(&needle))
            .and_then(|l| l.split_whitespace().next()?.parse().ok());
        if found.is_some() {
            return found;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

fn locate_pm_daemon_dir() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest.parent()?;
    for profile in ["debug", "release"] {
        let candidate = workspace_root
            .join("target")
            .join(profile)
            .join("pm-daemon");
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

/// A detached child must keep running and logging after the process that
/// spawned it goes away. Dropping the spawning runtime closes everything it
/// owned, exactly like the CLI exiting after `devcli start`: with piped output
/// the child died of SIGPIPE on its next write.
#[test]
#[cfg(unix)]
fn test_detached_output_survives_the_spawner() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("ticker.log");

    let task = Task {
        id: "ticker".to_string(),
        command: "sh -c 'i=0; while [ $i -lt 20 ]; do echo tick $i; i=$((i+1)); sleep 0.05; done'"
            .to_string(),
        args: vec![],
        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
        env: HashMap::new(),
        is_detached: true,
        log_file: Some(log_path.clone()),
        health_check: HealthCheck::Process {},
        restart_policy: RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    };

    let pid = {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let running = rt.block_on(engine::spawn(&task)).expect("spawn ticker");
        running.pid
    };

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let content = std::fs::read_to_string(&log_path).unwrap_or_default();
        if content.contains("tick 19") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "child stopped logging once the spawner was gone; log was:\n{content}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .status();
}

/// While the caller keeps the receiver, output of a detached child is still
/// delivered through `output_rx`, and the channel closes once the child exits.
#[tokio::test]
#[cfg(unix)]
async fn test_detached_output_is_streamed_from_the_log_file() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("stream.log");

    let task = Task {
        id: "stream".to_string(),
        command: "sh -c 'echo first; echo second >&2'".to_string(),
        args: vec![],
        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
        env: HashMap::new(),
        is_detached: true,
        log_file: Some(log_path.clone()),
        health_check: HealthCheck::Process {},
        restart_policy: RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    };

    let mut running = engine::spawn(&task).await.expect("spawn sh");
    let mut lines = Vec::new();
    let collected = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(msg) = running.output_rx.recv().await {
            lines.push(msg.content);
        }
    })
    .await;

    assert!(
        collected.is_ok(),
        "channel never closed after the child exited"
    );
    assert!(lines.contains(&"first".to_string()), "got {lines:?}");
    assert!(lines.contains(&"second".to_string()), "got {lines:?}");
}

/// Invalid UTF-8 on an attached child's output must not stop log capture.
#[tokio::test]
#[cfg(unix)]
async fn test_attached_output_survives_invalid_utf8() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("binary.log");

    let task = Task {
        id: "binary".to_string(),
        command: "sh -c \"printf 'bad \\377 byte\\n'; echo AFTER\"".to_string(),
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
    let mut lines = Vec::new();
    while let Some(msg) = running.output_rx.recv().await {
        lines.push(msg.content);
    }
    if let Some(child) = running.child.as_mut() {
        let _ = child.wait().await;
    }

    assert!(lines.iter().any(|l| l.starts_with("bad ")), "got {lines:?}");
    assert!(lines.contains(&"AFTER".to_string()), "got {lines:?}");
    wait_for_log_content(&log_path, "AFTER").await;
}

/// A live PID whose process started long after we recorded ours belongs to
/// someone else: the store must not report it as our running app (which would
/// make `stop` signal an unrelated process).
#[tokio::test]
#[cfg(unix)]
async fn test_reused_pid_is_not_reported_as_running() {
    let temp_dir = TempDir::new().unwrap();
    let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

    let task = sleep_task("reused", 30, None);
    let running = engine::spawn(&task).await.expect("spawn sleep");
    let (pid, pgid) = (running.pid, running.pgid);
    drop(running.output_rx);

    let genuine = managed_from_running(task.clone(), pid, None, HashMap::new());
    assert!(
        store.is_running(&genuine),
        "freshly recorded process is ours"
    );

    let mut stale = genuine.clone();
    stale.start_time = Utc::now() - chrono::Duration::hours(1);
    assert!(
        !store.is_running(&stale),
        "a process started an hour after our record is not ours"
    );

    let _ = engine::terminate(pid, pgid, true).await;
}

/// Rotating the log of a running detached app keeps it logging, from the start
/// of the truncated file (the child writes in append mode, so no sparse hole).
#[tokio::test]
#[cfg(unix)]
async fn test_log_rotation_while_app_runs() {
    let temp_dir = TempDir::new().unwrap();
    let log_path = temp_dir.path().join("rotate.log");

    let task = Task {
        id: "rotate".to_string(),
        command: "sh -c 'i=0; while [ $i -lt 100 ]; do echo line $i; i=$((i+1)); sleep 0.02; done'"
            .to_string(),
        args: vec![],
        working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp")),
        env: HashMap::new(),
        is_detached: true,
        log_file: Some(log_path.clone()),
        health_check: HealthCheck::Process {},
        restart_policy: RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    };

    let running = engine::spawn(&task).await.expect("spawn");
    drop(running.output_rx);
    wait_for_log_content(&log_path, "line 10").await;

    assert!(engine::rotate_log_if_large(&log_path, 1).unwrap());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !std::fs::read_to_string(&log_path)
        .unwrap_or_default()
        .contains("line 99")
    {
        assert!(std::time::Instant::now() < deadline, "app stopped logging");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let content = std::fs::read(&log_path).unwrap();
    assert_ne!(content.first(), Some(&0u8), "sparse hole after truncation");
    assert!(!String::from_utf8_lossy(&content).contains("line 0\n"));
    let rotated = std::fs::read_to_string(temp_dir.path().join("rotate.log.1")).unwrap();
    assert!(rotated.contains("line 0\n"));
}
