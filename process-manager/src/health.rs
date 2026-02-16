use crate::model::HealthCheck;
use anyhow::{Context, Result};
use std::time::Duration;

/// Health check execution engine.
pub struct HealthCheckEngine;

impl HealthCheckEngine {
    pub fn new() -> Self {
        Self
    }

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
            HealthCheck::Process {} => Ok(true),
        }
    }

    #[tracing::instrument(skip(self), fields(url = %url))]
    async fn check_http(&self, url: &str, timeout_secs: u64, expected_status: u16) -> Result<bool> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .context("Failed to create HTTP client")?;

        match client.get(url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                if status == expected_status {
                    tracing::debug!("HTTP check passed: {}", status);
                    Ok(true)
                } else if expected_status == 200 && (200..300).contains(&status) {
                    tracing::debug!("HTTP check passed (2xx): {}", status);
                    Ok(true)
                } else {
                    tracing::warn!(
                        "HTTP check failed: {} (expected {})",
                        status,
                        expected_status
                    );
                    Ok(false)
                }
            }
            Err(e) => {
                tracing::warn!("HTTP check error: {}", e);
                Ok(false)
            }
        }
    }

    #[tracing::instrument(skip(self), fields(host = %host, port = %port))]
    async fn check_tcp(&self, host: &str, port: u16, timeout_secs: u64) -> Result<bool> {
        let addr = format!("{}:{}", host, port);
        let timeout = Duration::from_secs(timeout_secs);

        match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(&addr)).await {
            Ok(Ok(_)) => {
                tracing::debug!("TCP check passed");
                Ok(true)
            }
            Ok(Err(e)) => {
                tracing::warn!("TCP check failed: {}", e);
                Ok(false)
            }
            Err(_) => {
                tracing::warn!("TCP check timed out");
                Ok(false)
            }
        }
    }

    #[tracing::instrument(skip(self), fields(command = %command))]
    async fn check_command(
        &self,
        command: &str,
        timeout_secs: u64,
        expected_exit_code: i32,
    ) -> Result<bool> {
        let timeout = Duration::from_secs(timeout_secs);
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to spawn check command")?;

        match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => {
                let code = status.code().unwrap_or(-1);
                if code == expected_exit_code {
                    tracing::debug!("Command check passed");
                    Ok(true)
                } else {
                    tracing::warn!(
                        "Command check failed: code {} (expected {})",
                        code,
                        expected_exit_code
                    );
                    Ok(false)
                }
            }
            Ok(Err(e)) => {
                tracing::error!("Command execution failed: {}", e);
                Ok(false)
            }
            Err(_) => {
                let _ = child.kill().await;
                tracing::warn!("Command check timed out");
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
