#[cfg(test)]
mod tests {
    use crate::model::*;
    use crate::state::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            command: "echo test".to_string(),
            args: vec![],
            working_dir: PathBuf::from("/tmp"),
            env: HashMap::new(),
            is_detached: false,
            log_file: None,
            health_check: HealthCheck::Process {},
            restart_policy: RestartPolicy::default(),
        }
    }

    fn create_test_process(id: &str, pid: u32) -> ManagedProcess {
        let mut metadata = HashMap::new();
        metadata.insert("project".to_string(), "test-project".to_string());
        metadata.insert("app".to_string(), id.to_string());

        ManagedProcess {
            id: id.to_string(),
            pid,
            pgid: None,
            task: create_test_task(id),
            start_time: Utc::now(),
            metadata,
            runtime: ProcessRuntime::default(),
        }
    }

    #[test]
    fn test_state_store_new() {
        let temp_dir = TempDir::new().unwrap();
        let _store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();
        assert!(temp_dir.path().exists());
    }

    #[test]
    fn test_save_and_load_process() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        let process = create_test_process("test-app", 12345);

        // Save process
        store.save(&process).unwrap();

        // Load it back
        let loaded = store.load("test-app").unwrap();
        assert!(loaded.is_some());

        let loaded_process = loaded.unwrap();
        assert_eq!(loaded_process.id, "test-app");
        assert_eq!(loaded_process.pid, 12345);
    }

    #[test]
    fn test_load_nonexistent_process() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        let loaded = store.load("nonexistent").unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_list_processes() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        // Save multiple processes
        let proc1 = create_test_process("app1", 111);
        let proc2 = create_test_process("app2", 222);
        let proc3 = create_test_process("app3", 333);

        store.save(&proc1).unwrap();
        store.save(&proc2).unwrap();
        store.save(&proc3).unwrap();

        // List all
        let processes = store.list().unwrap();
        assert_eq!(processes.len(), 3);

        let ids: Vec<String> = processes.iter().map(|p| p.id.clone()).collect();
        assert!(ids.contains(&"app1".to_string()));
        assert!(ids.contains(&"app2".to_string()));
        assert!(ids.contains(&"app3".to_string()));
    }

    #[test]
    fn test_delete_process() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        let process = create_test_process("to-delete", 99999);
        store.save(&process).unwrap();

        // Verify it exists
        assert!(store.load("to-delete").unwrap().is_some());

        // Delete it
        store.delete("to-delete").unwrap();

        // Verify it's gone
        assert!(store.load("to-delete").unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_process() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        // Should not error
        store.delete("nonexistent").unwrap();
    }

    #[test]
    fn test_find_by_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        // Create processes with different metadata
        let mut proc1 = create_test_process("app1", 111);
        proc1
            .metadata
            .insert("environment".to_string(), "dev".to_string());
        proc1
            .metadata
            .insert("project".to_string(), "myproject".to_string());

        let mut proc2 = create_test_process("app2", 222);
        proc2
            .metadata
            .insert("environment".to_string(), "prod".to_string());
        proc2
            .metadata
            .insert("project".to_string(), "myproject".to_string());

        let mut proc3 = create_test_process("app3", 333);
        proc3
            .metadata
            .insert("environment".to_string(), "dev".to_string());
        proc3
            .metadata
            .insert("project".to_string(), "otherproject".to_string());

        store.save(&proc1).unwrap();
        store.save(&proc2).unwrap();
        store.save(&proc3).unwrap();

        // Find by environment=dev
        let dev_processes = store.find_by_metadata("environment", "dev").unwrap();
        assert_eq!(dev_processes.len(), 2);
        assert!(dev_processes.iter().any(|p| p.id == "app1"));
        assert!(dev_processes.iter().any(|p| p.id == "app3"));

        // Find by project=myproject
        let myproject_processes = store.find_by_metadata("project", "myproject").unwrap();
        assert_eq!(myproject_processes.len(), 2);
        assert!(myproject_processes.iter().any(|p| p.id == "app1"));
        assert!(myproject_processes.iter().any(|p| p.id == "app2"));

        // Find by nonexistent key
        let none_processes = store.find_by_metadata("nonexistent", "value").unwrap();
        assert_eq!(none_processes.len(), 0);
    }

    #[test]
    fn test_find_one_by_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        let mut proc = create_test_process("unique-app", 12345);
        proc.metadata
            .insert("unique_key".to_string(), "unique_value".to_string());
        store.save(&proc).unwrap();

        // Find the unique process
        let found = store
            .find_one_by_metadata("unique_key", "unique_value")
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "unique-app");

        // Find nonexistent
        let not_found = store
            .find_one_by_metadata("unique_key", "wrong_value")
            .unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_should_restart_enabled() {
        let mut proc = create_test_process("test", 123);
        proc.runtime.last_exit_code = Some(1);

        let policy = RestartPolicy {
            enabled: true,
            max_restarts: 3,
            restart_window_secs: 300,
            initial_backoff_secs: 1,
            max_backoff_secs: 60,
            backoff_multiplier: 2.0,
            restart_on_exit_codes: None,
        };

        assert!(proc.should_restart(&policy));
    }

    #[test]
    fn test_should_restart_disabled() {
        let mut proc = create_test_process("test", 123);
        proc.runtime.last_exit_code = Some(1);

        let policy = RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        };

        assert!(!proc.should_restart(&policy));
    }

    #[test]
    fn test_should_restart_exit_code_zero() {
        let mut proc = create_test_process("test", 123);
        proc.runtime.last_exit_code = Some(0);

        let policy = RestartPolicy::default();

        // Should not restart on exit code 0 by default
        assert!(!proc.should_restart(&policy));
    }

    #[test]
    fn test_should_restart_specific_exit_codes() {
        let mut proc = create_test_process("test", 123);
        proc.runtime.last_exit_code = Some(1);

        let policy = RestartPolicy {
            enabled: true,
            restart_on_exit_codes: Some(vec![2, 3]), // Only restart on 2 or 3
            ..Default::default()
        };

        // Should not restart because exit code 1 is not in the list
        assert!(!proc.should_restart(&policy));

        // Change exit code to 2
        proc.runtime.last_exit_code = Some(2);
        assert!(proc.should_restart(&policy));
    }

    #[test]
    fn test_should_restart_max_restarts_exceeded() {
        let mut proc = create_test_process("test", 123);
        proc.runtime.last_exit_code = Some(1);

        // Add restart events within the window
        for _ in 0..3 {
            proc.runtime.history.push(RestartEvent {
                timestamp: Utc::now(),
                reason: "crash".to_string(),
            });
        }

        let policy = RestartPolicy {
            enabled: true,
            max_restarts: 3,
            restart_window_secs: 300,
            ..Default::default()
        };

        // Should not restart - already hit max
        assert!(!proc.should_restart(&policy));
    }

    #[test]
    fn test_calculate_backoff() {
        let mut proc = create_test_process("test", 123);

        let policy = RestartPolicy {
            initial_backoff_secs: 2,
            max_backoff_secs: 60,
            backoff_multiplier: 2.0,
            ..Default::default()
        };

        // First restart (restart_count = 0, treated as 1)
        proc.runtime.restart_count = 0;
        let backoff = proc.calculate_backoff(&policy);
        assert_eq!(backoff.as_secs(), 2); // 2 * 2^0 = 2

        // Second restart
        proc.runtime.restart_count = 1;
        let backoff = proc.calculate_backoff(&policy);
        assert_eq!(backoff.as_secs(), 2); // 2 * 2^0 = 2

        // Third restart
        proc.runtime.restart_count = 2;
        let backoff = proc.calculate_backoff(&policy);
        assert_eq!(backoff.as_secs(), 4); // 2 * 2^1 = 4

        // Fourth restart
        proc.runtime.restart_count = 3;
        let backoff = proc.calculate_backoff(&policy);
        assert_eq!(backoff.as_secs(), 8); // 2 * 2^2 = 8

        // Many restarts - should cap at max_backoff_secs
        proc.runtime.restart_count = 100;
        let backoff = proc.calculate_backoff(&policy);
        assert_eq!(backoff.as_secs(), 60); // Capped at max
    }

    #[test]
    fn test_process_runtime_default() {
        let runtime = ProcessRuntime::default();
        assert_eq!(runtime.restart_count, 0);
        assert_eq!(runtime.history.len(), 0);
        assert_eq!(runtime.health_failures, 0);
        assert!(runtime.last_exit_code.is_none());
        assert!(runtime.last_exit_time.is_none());
        assert!(runtime.last_health_check.is_none());
    }

    #[test]
    fn test_status_changed_updated_on_save_and_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();
        let status_path = store.status_changed_path();

        let process = create_test_process("status-app", 12345);
        store.save(&process).unwrap();
        assert!(status_path.exists());

        let mtime_after_save = std::fs::metadata(&status_path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));

        store.delete("status-app").unwrap();
        let mtime_after_delete = std::fs::metadata(&status_path).unwrap().modified().unwrap();
        assert!(mtime_after_delete >= mtime_after_save);
    }

    /// Readers must never observe a half-written entry while it is rewritten.
    #[test]
    fn test_concurrent_save_never_hides_the_entry() {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().to_path_buf();
        let store = StateStore::new(dir.clone()).unwrap();
        let mut process = create_test_process("busy-app", 4242);
        // A large entry makes a torn write far more likely to be observed.
        for i in 0..4000 {
            process.metadata.insert(format!("k{i}"), "v".repeat(64));
        }
        store.save(&process).unwrap();

        let writer = std::thread::spawn(move || {
            let store = StateStore::new(dir).unwrap();
            for _ in 0..300 {
                store.save(&process).unwrap();
            }
        });

        while !writer.is_finished() {
            let listed = store.list().unwrap();
            assert_eq!(listed.len(), 1, "entry vanished during a concurrent save");
        }
        writer.join().unwrap();

        let leftovers: Vec<_> = std::fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files left behind");
    }

    /// Deleting an entry someone else already removed is not an error.
    #[test]
    fn test_delete_twice_is_ok() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();
        store.save(&create_test_process("gone", 1)).unwrap();
        store.delete("gone").unwrap();
        store.delete("gone").unwrap();
        assert!(store.load("gone").unwrap().is_none());
    }

    /// State entries and their directory are private to the user.
    #[test]
    #[cfg(unix)]
    fn test_state_files_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("processes");
        let store = StateStore::new(dir.clone()).unwrap();
        store.save(&create_test_process("secret-app", 1)).unwrap();

        let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        let file_mode = std::fs::metadata(dir.join("secret-app.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700);
        assert_eq!(file_mode, 0o600);
    }

    /// `mark_stopping` flags the entry and hands back its current content.
    #[test]
    fn test_mark_stopping_returns_current_entry() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();
        assert!(store.mark_stopping("missing").unwrap().is_none());

        store.save(&create_test_process("app", 111)).unwrap();
        // Someone (pm-daemon) restarted it meanwhile.
        store.save(&create_test_process("app", 222)).unwrap();

        let current = store.mark_stopping("app").unwrap().unwrap();
        assert_eq!(current.pid, 222);
        assert!(current.is_stopping());
        assert!(store.load("app").unwrap().unwrap().is_stopping());
    }

    /// While a daemon runs, a crashed app it will restart is left to it.
    #[test]
    fn test_cleanup_dead_leaves_restartable_entries_to_the_daemon() {
        use fs2::FileExt;

        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();
        let mut dead = create_test_process("crashed", 2_000_000_000);
        dead.task.restart_policy.enabled = true;
        store.save(&dead).unwrap();

        // Pretend to be the daemon.
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(temp_dir.path().join(".daemon.lock"))
            .unwrap();
        lock.lock_exclusive().unwrap();
        assert!(store.cleanup_dead().unwrap().is_empty());
        assert!(store.load("crashed").unwrap().is_some());

        // No daemon: nothing will restart it, so it is cleaned up.
        FileExt::unlock(&lock).unwrap();
        assert_eq!(store.cleanup_dead().unwrap(), vec!["crashed".to_string()]);
    }

    /// Only one monitor lock can be held per state directory.
    #[test]
    fn test_daemon_lock_is_exclusive() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();

        let first = store.try_lock_daemon().unwrap();
        assert!(first.is_some());
        assert!(store.daemon_running());
        assert!(store.try_lock_daemon().unwrap().is_none());

        drop(first);
        // Other tests in this binary fork concurrently; a child holds a copy of
        // the lock descriptor until it execs, so allow the release a moment.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while store.daemon_running() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!store.daemon_running());
        assert!(store.try_lock_daemon().unwrap().is_some());
    }

    /// PID of a process that has already exited (and been reaped).
    fn dead_pid() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    /// An entry is orphaned once the attached `devcli start` owning it is gone,
    /// including when the owner's PID now belongs to a younger process.
    #[test]
    fn test_owner_gone() {
        let mut process = create_test_process("owned", std::process::id());
        assert!(!process.owner_gone(), "no owner: never orphaned");

        process
            .metadata
            .insert(OWNER_PID_KEY.to_string(), std::process::id().to_string());
        assert_eq!(process.owner_pid(), Some(std::process::id()));
        assert!(!process.owner_gone(), "the owner (this test) is alive");

        process
            .metadata
            .insert(OWNER_PID_KEY.to_string(), dead_pid().to_string());
        assert!(process.owner_gone(), "the owner exited");

        // A live PID that started after the entry is not the owner.
        let mut younger = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        process
            .metadata
            .insert(OWNER_PID_KEY.to_string(), younger.id().to_string());
        process.start_time = Utc::now() - chrono::Duration::minutes(5);
        let reused = process.owner_gone();
        let _ = younger.kill();
        let _ = younger.wait();
        assert!(reused, "the owner's PID was reused");
    }
}
