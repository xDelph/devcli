#[cfg(test)]
mod tests {
    use crate::model::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn test_health_check_default() {
        let default_check = HealthCheck::Process {};
        assert!(matches!(default_check, HealthCheck::Process {}));
    }

    #[test]
    fn test_health_check_http_serialization() {
        let check = HealthCheck::Http {
            url: "http://localhost:3000".to_string(),
            timeout_secs: 5,
            expected_status: 200,
        };

        let json = serde_json::to_string(&check).unwrap();
        let deserialized: HealthCheck = serde_json::from_str(&json).unwrap();

        assert_eq!(check, deserialized);
    }

    #[test]
    fn test_health_check_tcp_serialization() {
        let check = HealthCheck::Tcp {
            host: "127.0.0.1".to_string(),
            port: 5432,
            timeout_secs: 10,
        };

        let json = serde_json::to_string(&check).unwrap();
        let deserialized: HealthCheck = serde_json::from_str(&json).unwrap();

        assert_eq!(check, deserialized);
    }

    #[test]
    fn test_health_check_command_serialization() {
        let check = HealthCheck::Command {
            command: "echo test".to_string(),
            timeout_secs: 3,
            expected_exit_code: 0,
        };

        let json = serde_json::to_string(&check).unwrap();
        let deserialized: HealthCheck = serde_json::from_str(&json).unwrap();

        assert_eq!(check, deserialized);
    }

    #[test]
    fn test_restart_policy_default() {
        let policy = RestartPolicy::default();
        assert_eq!(policy.max_restarts, 3);
        assert_eq!(policy.restart_window_secs, 300);
        assert_eq!(policy.initial_backoff_secs, 1);
        assert_eq!(policy.max_backoff_secs, 60);
        assert_eq!(policy.backoff_multiplier, 2.0);
        assert!(policy.enabled);
        assert!(policy.restart_on_exit_codes.is_none());
    }

    #[test]
    fn test_restart_policy_serialization() {
        let policy = RestartPolicy {
            max_restarts: 5,
            restart_window_secs: 600,
            initial_backoff_secs: 2,
            max_backoff_secs: 120,
            backoff_multiplier: 3.0,
            restart_on_exit_codes: Some(vec![1, 2, 3]),
            enabled: false,
        };

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: RestartPolicy = serde_json::from_str(&json).unwrap();

        assert_eq!(policy, deserialized);
    }

    #[test]
    fn test_task_serialization() {
        let mut env = HashMap::new();
        env.insert("FOO".to_string(), "bar".to_string());

        let task = Task {
            id: "test-task".to_string(),
            command: "npm run dev".to_string(),
            args: vec!["--port".to_string(), "3000".to_string()],
            working_dir: PathBuf::from("/tmp"),
            env,
            is_detached: true,
            log_file: Some(PathBuf::from("/tmp/test.log")),
            health_check: HealthCheck::Http {
                url: "http://localhost:3000".to_string(),
                timeout_secs: 5,
                expected_status: 200,
            },
            restart_policy: RestartPolicy::default(),
        };

        let json = serde_json::to_string(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(task, deserialized);
    }

    #[test]
    fn test_output_source_serialization() {
        let stdout = OutputSource::Stdout;
        let stderr = OutputSource::Stderr;

        let stdout_json = serde_json::to_string(&stdout).unwrap();
        let stderr_json = serde_json::to_string(&stderr).unwrap();

        let stdout_de: OutputSource = serde_json::from_str(&stdout_json).unwrap();
        let stderr_de: OutputSource = serde_json::from_str(&stderr_json).unwrap();

        assert_eq!(stdout, stdout_de);
        assert_eq!(stderr, stderr_de);
    }
}
