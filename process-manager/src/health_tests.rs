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

    #[tokio::test]
    async fn test_health_check_http_success_exact_status() {
        let engine = HealthCheckEngine::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    use tokio::io::AsyncWriteExt;
                    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });

        let check = HealthCheck::Http {
            url: format!("http://{}/", addr),
            timeout_secs: 2,
            expected_status: 200,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_health_check_http_success_2xx_when_expecting_200() {
        let engine = HealthCheckEngine::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    use tokio::io::AsyncWriteExt;
                    let response = "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });

        let check = HealthCheck::Http {
            url: format!("http://{}/", addr),
            timeout_secs: 2,
            expected_status: 200,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_health_check_http_wrong_status() {
        let engine = HealthCheckEngine::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    use tokio::io::AsyncWriteExt;
                    let response = "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n";
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });

        let check = HealthCheck::Http {
            url: format!("http://{}/", addr),
            timeout_secs: 2,
            expected_status: 200,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_health_check_tcp_success() {
        let engine = HealthCheckEngine::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            while listener.accept().await.is_ok() {}
        });

        let check = HealthCheck::Tcp {
            host: "127.0.0.1".to_string(),
            port,
            timeout_secs: 2,
        };

        let result = engine.check(&check).await.unwrap();
        assert!(result);
    }

    #[test]
    fn test_health_check_engine_default() {
        let engine1 = HealthCheckEngine::new();
        let engine2 = HealthCheckEngine;

        // Both should work (no state to compare)
        assert!(std::mem::size_of_val(&engine1) == std::mem::size_of_val(&engine2));
    }
}
