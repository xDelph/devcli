// Unit tests for process tracker
// Tests PID file management, process status checking, and cleanup

#[cfg(test)]
mod tests {
    use crate::process::tracker::*;
    use chrono::Utc;
    use std::collections::HashMap;

    // Helper: Create a test process info
    fn create_test_process(app_name: &str, pid: u32) -> ProcessInfo {
        ProcessInfo {
            app_name: app_name.to_string(),
            pid,
            command: "test command".to_string(),
            working_dir: "/tmp/test".to_string(),
            start_time: Utc::now(),
            env_vars: HashMap::new(),
            project: Some("test-project".to_string()),
            app_config_name: Some(app_name.to_string()),
            environment: Some("local".to_string()),
            command_variant: Some("start".to_string()),
            stage: None,
        }
    }

    // Test: ProcessTracker creation
    #[test]
    fn test_tracker_creation() {
        let tracker = ProcessTracker::new();
        assert!(tracker.is_ok());
    }

    // Test: Register and retrieve process
    #[test]
    fn test_register_and_get_process() {
        let tracker = ProcessTracker::new().unwrap();
        let unique_name = format!("test-register-get-{}", std::process::id());
        let process = create_test_process(&unique_name, 12345);
        
        // Register
        tracker.register_process(process.clone()).unwrap();
        
        // Retrieve
        let retrieved = tracker.get_process(&unique_name).unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.app_name, unique_name);
        assert_eq!(retrieved.pid, 12345);
        assert_eq!(retrieved.command, "test command");
        
        // Cleanup
        tracker.remove_process(&unique_name).unwrap();
    }

    // Test: Get non-existent process returns None
    #[test]
    fn test_get_nonexistent_process() {
        let tracker = ProcessTracker::new().unwrap();
        let result = tracker.get_process("nonexistent").unwrap();
        assert!(result.is_none());
    }

    // Test: Register multiple processes
    #[test]
    fn test_register_multiple_processes() {
        let tracker = ProcessTracker::new().unwrap();
        
        // Register 3 processes
        for i in 1..=3 {
            let process = create_test_process(&format!("app-{}", i), 10000 + i);
            tracker.register_process(process).unwrap();
        }
        
        // List all processes
        let processes = tracker.list_processes().unwrap();
        assert!(processes.len() >= 3); // At least our 3 processes
        
        // Verify each exists
        for i in 1..=3 {
            let retrieved = tracker.get_process(&format!("app-{}", i)).unwrap();
            assert!(retrieved.is_some());
        }
        
        // Cleanup
        for i in 1..=3 {
            tracker.remove_process(&format!("app-{}", i)).unwrap();
        }
    }

    // Test: Overwrite existing process
    #[test]
    fn test_overwrite_process() {
        let tracker = ProcessTracker::new().unwrap();
        
        // Register initial process
        let process1 = create_test_process("test-app", 11111);
        tracker.register_process(process1).unwrap();
        
        // Register again with different PID
        let process2 = create_test_process("test-app", 22222);
        tracker.register_process(process2).unwrap();
        
        // Should have new PID
        let retrieved = tracker.get_process("test-app").unwrap().unwrap();
        assert_eq!(retrieved.pid, 22222);
        
        // Cleanup
        tracker.remove_process("test-app").unwrap();
    }

    // Test: Remove process
    #[test]
    fn test_remove_process() {
        let tracker = ProcessTracker::new().unwrap();
        
        // Register process
        let process = create_test_process("test-app", 12345);
        tracker.register_process(process).unwrap();
        
        // Verify it exists
        assert!(tracker.get_process("test-app").unwrap().is_some());
        
        // Remove it
        tracker.remove_process("test-app").unwrap();
        
        // Verify it's gone
        assert!(tracker.get_process("test-app").unwrap().is_none());
    }

    // Test: Remove non-existent process doesn't error
    #[test]
    fn test_remove_nonexistent_process() {
        let tracker = ProcessTracker::new().unwrap();
        
        // Should not error
        let result = tracker.remove_process("nonexistent");
        assert!(result.is_ok());
    }

    // Test: is_running with current process (should be true)
    #[test]
    fn test_is_running_current_process() {
        let tracker = ProcessTracker::new().unwrap();
        
        // Current process PID should be running
        let current_pid = std::process::id();
        assert!(tracker.is_running(current_pid));
    }

    // Test: is_running with fake PID (should be false)
    #[test]
    fn test_is_running_fake_pid() {
        let tracker = ProcessTracker::new().unwrap();
        
        // Very high PID unlikely to exist
        let fake_pid = 999999;
        assert!(!tracker.is_running(fake_pid));
    }

    // Test: ProcessInfo serialization
    #[test]
    fn test_process_info_serialization() {
        let process = create_test_process("test-app", 12345);
        
        // Serialize to JSON
        let json = serde_json::to_string(&process).unwrap();
        
        // Should contain key fields
        assert!(json.contains("test-app"));
        assert!(json.contains("12345"));
        assert!(json.contains("test command"));
        
        // Deserialize back
        let deserialized: ProcessInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.app_name, "test-app");
        assert_eq!(deserialized.pid, 12345);
    }

    // Test: ProcessInfo with all optional fields
    #[test]
    fn test_process_info_full() {
        let mut env_vars = HashMap::new();
        env_vars.insert("NODE_ENV".to_string(), "production".to_string());
        env_vars.insert("PORT".to_string(), "3000".to_string());
        
        let process = ProcessInfo {
            app_name: "full-app".to_string(),
            pid: 54321,
            command: "npm start".to_string(),
            working_dir: "/home/user/app".to_string(),
            start_time: Utc::now(),
            env_vars,
            project: Some("my-project".to_string()),
            app_config_name: Some("api-server".to_string()),
            environment: Some("docker".to_string()),
            command_variant: Some("run".to_string()),
            stage: None,
        };
        
        // Serialize and deserialize
        let json = serde_json::to_string(&process).unwrap();
        let deserialized: ProcessInfo = serde_json::from_str(&json).unwrap();
        
        // Verify all fields
        assert_eq!(deserialized.app_name, "full-app");
        assert_eq!(deserialized.project, Some("my-project".to_string()));
        assert_eq!(deserialized.app_config_name, Some("api-server".to_string()));
        assert_eq!(deserialized.environment, Some("docker".to_string()));
        assert_eq!(deserialized.command_variant, Some("run".to_string()));
        assert_eq!(deserialized.env_vars.len(), 2);
        assert_eq!(deserialized.env_vars.get("NODE_ENV"), Some(&"production".to_string()));
    }

    // Test: ProcessInfo with minimal fields (old format compatibility)
    #[test]
    fn test_process_info_minimal() {
        let json = r#"{
            "app_name": "minimal-app",
            "pid": 11111,
            "command": "redis-server",
            "working_dir": "/tmp",
            "start_time": "2025-01-01T00:00:00Z",
            "env_vars": {}
        }"#;
        
        // Should deserialize with defaults for optional fields
        let process: ProcessInfo = serde_json::from_str(json).unwrap();
        assert_eq!(process.app_name, "minimal-app");
        assert_eq!(process.pid, 11111);
        assert_eq!(process.project, None);
        assert_eq!(process.app_config_name, None);
        assert_eq!(process.environment, None);
        assert_eq!(process.command_variant, None);
    }

    // Test: List processes filters correctly
    #[test]
    fn test_list_processes() {
        let tracker = ProcessTracker::new().unwrap();
        
        // Register test processes
        for i in 1..=5 {
            let process = create_test_process(&format!("list-test-{}", i), 20000 + i);
            tracker.register_process(process).unwrap();
        }
        
        let processes = tracker.list_processes().unwrap();
        
        // Should contain our test processes
        let test_processes: Vec<_> = processes
            .iter()
            .filter(|p| p.app_name.starts_with("list-test-"))
            .collect();
        assert_eq!(test_processes.len(), 5);
        
        // Clean up
        for i in 1..=5 {
            tracker.remove_process(&format!("list-test-{}", i)).unwrap();
        }
    }

    // Test: ProcessInfo timestamp is recent
    #[test]
    fn test_process_info_timestamp() {
        let before = Utc::now();
        let process = create_test_process("timestamp-test", 99999);
        let after = Utc::now();
        
        // Timestamp should be between before and after
        assert!(process.start_time >= before);
        assert!(process.start_time <= after);
    }

    // Test: Special characters in app name
    #[test]
    fn test_special_characters_in_name() {
        let tracker = ProcessTracker::new().unwrap();
        
        // App names with special characters
        let special_names = vec![
            "app-with-dashes",
            "app_with_underscores",
            "app.with.dots",
            "app:with:colons",
        ];
        
        for name in special_names {
            let process = create_test_process(name, 30000);
            tracker.register_process(process).unwrap();
            
            let retrieved = tracker.get_process(name).unwrap();
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().app_name, name);
            
            tracker.remove_process(name).unwrap();
        }
    }

    // Test: Empty command is allowed
    #[test]
    fn test_empty_command() {
        let mut process = create_test_process("empty-cmd", 40000);
        process.command = String::new();
        
        // Should serialize/deserialize fine
        let json = serde_json::to_string(&process).unwrap();
        let deserialized: ProcessInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.command, "");
    }

    // Test: Very long working directory path
    #[test]
    fn test_long_working_directory() {
        let tracker = ProcessTracker::new().unwrap();
        
        let mut process = create_test_process("long-path", 50000);
        process.working_dir = "/very/long/path/that/goes/deep/into/the/filesystem/structure/for/testing/purposes".to_string();
        
        tracker.register_process(process).unwrap();
        
        let retrieved = tracker.get_process("long-path").unwrap().unwrap();
        assert!(retrieved.working_dir.len() > 50);
        
        tracker.remove_process("long-path").unwrap();
    }

    // Test: Process with many environment variables
    #[test]
    fn test_many_env_vars() {
        let tracker = ProcessTracker::new().unwrap();
        
        let mut env_vars = HashMap::new();
        for i in 0..100 {
            env_vars.insert(format!("VAR_{}", i), format!("value_{}", i));
        }
        
        let mut process = create_test_process("many-vars", 60000);
        process.env_vars = env_vars;
        
        tracker.register_process(process).unwrap();
        
        let retrieved = tracker.get_process("many-vars").unwrap().unwrap();
        assert_eq!(retrieved.env_vars.len(), 100);
        
        tracker.remove_process("many-vars").unwrap();
    }

    // Test: Default trait implementation
    #[test]
    fn test_default_trait() {
        let tracker = ProcessTracker::default();
        
        // Should work the same as new()
        let process = create_test_process("default-test", 70000);
        tracker.register_process(process).unwrap();
        
        let retrieved = tracker.get_process("default-test").unwrap();
        assert!(retrieved.is_some());
        
        tracker.remove_process("default-test").unwrap();
    }
}
