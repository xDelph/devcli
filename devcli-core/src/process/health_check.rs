// Health check engine for monitoring process health
// Supports HTTP, TCP, Command, and Process-alive checks

use crate::config::models::HealthCheck;
use anyhow::{Context, Result};
use std::time::Duration;

/// Health check execution engine
/// Performs various types of health checks on running processes
pub struct HealthCheckEngine;

impl HealthCheckEngine {
    /// Create a new health check engine
    pub fn new() -> Self {
        Self
    }

    /// Execute a health check based on its configuration
    /// Returns true if the check passed, false if it failed
    #[tracing::instrument(skip(self), fields(check_type = ?health_check))]
    pub async fn check(&self, health_check: &HealthCheck) -> Result<bool> {
        match health_check {
            HealthCheck::Http {
                url,
                timeout_secs,
                expected_status,
            } => self.check_http(url, *timeout_secs, *expected_status).await,
            HealthCheck::Tcp {
                host,
                port,
                timeout_secs,
            } => self.check_tcp(host, *port, *timeout_secs).await,
            HealthCheck::Command {
                command,
                timeout_secs,
                expected_exit_code,
            } => {
                self.check_command(command, *timeout_secs, *expected_exit_code)
                    .await
            }
            HealthCheck::Process {} => {
                // Process-alive check is handled separately by ProcessTracker.is_running()
                // This variant is just a marker indicating no additional health check is needed
                Ok(true)
            }
        }
    }

    /// Perform HTTP health check
    /// Makes a GET request to the URL and checks if the status code matches expected
    #[tracing::instrument(skip(self), fields(url = %url, timeout_secs = timeout_secs, expected_status = expected_status))]
    async fn check_http(&self, url: &str, timeout_secs: u64, expected_status: u16) -> Result<bool> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to create HTTP client")?;

        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                // Check if status matches expected
                if status == expected_status {
                    tracing::debug!(
                        url = %url,
                        status = status,
                        expected = expected_status,
                        "HTTP health check passed"
                    );
                    Ok(true)
                } else {
                    // Also accept any 2xx status if expected is 200
                    if expected_status == 200 && (200..300).contains(&status) {
                        tracing::debug!(
                            url = %url,
                            status = status,
                            expected = expected_status,
                            "HTTP health check passed (2xx status)"
                        );
                        Ok(true)
                    } else {
                        tracing::warn!(
                            url = %url,
                            status = status,
                            expected = expected_status,
                            "HTTP health check failed - status mismatch"
                        );
                        Ok(false)
                    }
                }
            }
            Err(e) => {
                // Connection errors, timeouts, etc. are health check failures
                // Don't propagate the error, just return false
                tracing::warn!(
                    url = %url,
                    error = %e,
                    "HTTP health check failed"
                );
                Ok(false)
            }
        }
    }

    /// Perform TCP health check
    /// Attempts to connect to host:port and returns true if successful
    #[tracing::instrument(skip(self), fields(host = %host, port = port, timeout_secs = timeout_secs))]
    async fn check_tcp(&self, host: &str, port: u16, timeout_secs: u64) -> Result<bool> {
        let addr = format!("{}:{}", host, port);
        let timeout = Duration::from_secs(timeout_secs);

        // Use tokio's timeout to wrap the connection attempt
        match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr)).await {
            Ok(Ok(_stream)) => {
                // Connection succeeded
                tracing::debug!(
                    addr = %addr,
                    "TCP health check passed"
                );
                Ok(true)
            }
            Ok(Err(e)) => {
                // Connection failed
                tracing::warn!(
                    addr = %addr,
                    error = %e,
                    "TCP health check failed"
                );
                Ok(false)
            }
            Err(_) => {
                // Timeout
                tracing::warn!(
                    addr = %addr,
                    timeout_secs = timeout_secs,
                    "TCP health check timed out"
                );
                Ok(false)
            }
        }
    }

    /// Perform command health check
    /// Executes a shell command and checks if the exit code matches expected
    #[tracing::instrument(skip(self), fields(command = %command, timeout_secs = timeout_secs, expected_exit_code = expected_exit_code))]
    async fn check_command(
        &self,
        command: &str,
        timeout_secs: u64,
        expected_exit_code: i32,
    ) -> Result<bool> {
        let timeout = Duration::from_secs(timeout_secs);

        // Spawn the command via shell
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to spawn health check command")?;

        // Wait for the command with timeout
        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => {
                let exit_code = status.code().unwrap_or(-1);
                if exit_code == expected_exit_code {
                    tracing::debug!(
                        command = %command,
                        exit_code = exit_code,
                        "Command health check passed"
                    );
                    Ok(true)
                } else {
                    tracing::warn!(
                        command = %command,
                        exit_code = exit_code,
                        expected = expected_exit_code,
                        "Command health check failed - exit code mismatch"
                    );
                    Ok(false)
                }
            }
            Ok(Err(e)) => {
                // Command execution failed
                tracing::error!(
                    command = %command,
                    error = %e,
                    "Command health check failed to execute"
                );
                Ok(false)
            }
            Err(_) => {
                // Timeout - kill the process
                let _ = child.kill().await;
                tracing::warn!(
                    command = %command,
                    timeout_secs = timeout_secs,
                    "Command health check timed out"
                );
                Ok(false)
            }
        }
    }
}

impl Default for HealthCheckEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_health_check_always_passes() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Process {};
        let result = engine.check(&check).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_command_health_check_success() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Command {
            command: "exit 0".to_string(),
            timeout_secs: 5,
            expected_exit_code: 0,
        };
        let result = engine.check(&check).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_command_health_check_failure() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Command {
            command: "exit 1".to_string(),
            timeout_secs: 5,
            expected_exit_code: 0,
        };
        let result = engine.check(&check).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_command_health_check_timeout() {
        let engine = HealthCheckEngine::new();
        let check = HealthCheck::Command {
            command: "sleep 10".to_string(),
            timeout_secs: 1,
            expected_exit_code: 0,
        };
        let result = engine.check(&check).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_tcp_health_check_failure() {
        let engine = HealthCheckEngine::new();
        // Connect to a port that's unlikely to be open
        let check = HealthCheck::Tcp {
            host: "127.0.0.1".to_string(),
            port: 54321,
            timeout_secs: 1,
        };
        let result = engine.check(&check).await.unwrap();
        assert!(!result);
    }

    // HTTP test would require a mock server, skipping for now
    // In a real implementation, you might use `wiremock` crate for HTTP tests
}
