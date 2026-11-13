#[cfg(test)]
mod tests {
    use crate::health::*;
    use crate::model::*;

    /// Consume the request head before answering. Replying and closing a socket
    /// that still has unread request bytes makes the kernel send a RST, which
    /// hyper 1 surfaces as a request error: the 2xx test failed ~1 run in 3.
    async fn read_request(stream: &mut tokio::net::TcpStream) {
        use tokio::io::AsyncReadExt;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
            match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
    }

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

    /// A timed-out check kills what the shell started, not only the shell.
    #[tokio::test]
    async fn test_health_check_command_timeout_kills_its_children() {
        let dir = tempfile::TempDir::new().unwrap();
        let pid_file = dir.path().join("child.pid");
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Command {
            command: format!("sleep 30 & echo $! > {}; wait", pid_file.display()),
            timeout_secs: 1,
            expected_exit_code: 0,
        };

        assert!(!engine.check(&check).await.unwrap());
        let child: u32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while crate::engine::pid_alive(child) && std::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert!(
            !crate::engine::pid_alive(child),
            "the check's child outlived its timeout"
        );
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
                    read_request(&mut stream).await;
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
                    read_request(&mut stream).await;
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
                    read_request(&mut stream).await;
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

        tokio::spawn(async move { while listener.accept().await.is_ok() {} });

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
