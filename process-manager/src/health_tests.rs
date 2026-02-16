#[cfg(test)]
mod tests {
    use crate::health::*;
    use crate::model::*;

    #[tokio::test]
    async fn test_health_check_process_always_passes() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Process {};

        let result = engine.check(&check).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_health_check_command_success() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Command {
            command: "true".to_string(), // 'true' command always exits with 0
            timeout_secs: 5,
            expected_exit_code: 0,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_health_check_command_failure() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Command {
            command: "false".to_string(), // 'false' command always exits with 1
            timeout_secs: 5,
            expected_exit_code: 0,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_health_check_command_custom_exit_code() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Command {
            command: "exit 42".to_string(),
            timeout_secs: 5,
            expected_exit_code: 42,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_health_check_command_timeout() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Command {
            command: "sleep 10".to_string(),
            timeout_secs: 1,
            expected_exit_code: 0,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(!result); // Should fail due to timeout
    }

    #[tokio::test]
    async fn test_health_check_tcp_nonexistent_port() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Tcp {
            host: "127.0.0.1".to_string(),
            port: 19999, // Unlikely to be in use
            timeout_secs: 1,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_health_check_http_invalid_url() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Http {
            url: "http://localhost:19999/nonexistent".to_string(),
            timeout_secs: 1,
            expected_status: 200,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_health_check_http_timeout() {
        let engine = HealthCheckEngine::new();
        // Use a non-routable IP to cause timeout
        let check = HealthCheck::Http {
            url: "http://192.0.2.1:80/".to_string(), // TEST-NET-1 (RFC 5737)
            timeout_secs: 1,
            expected_status: 200,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(!result);
    }

    #[test]
    fn test_health_check_engine_default() {
        let engine1 = HealthCheckEngine::new();
        let engine2 = HealthCheckEngine::default();

        // Both should work (no state to compare)
        assert!(std::mem::size_of_val(&engine1) == std::mem::size_of_val(&engine2));
    }
}
