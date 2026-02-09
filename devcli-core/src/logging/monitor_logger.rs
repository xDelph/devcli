// DEPRECATED: This custom monitor logger is being phased out in favor of the tracing infrastructure.
//
// New code should use the tracing crate for structured logging:
// - Use tracing::info!(), tracing::warn!(), tracing::error!() macros with structured fields
// - Logs are automatically written to JSON files at ~/.devcli/logs/devcli.YYYY-MM-DD.json
// - All monitor events now logged via tracing spans and events in src/commands/monitor.rs
//
// This module is kept temporarily for backwards compatibility but will be removed
// in a future release once all usages are migrated to tracing.

use crate::process::RestartReason;
use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;

/// Logger for monitor daemon events
/// Writes to a centralized monitor.log file
#[deprecated(
    since = "0.2.0",
    note = "Use tracing infrastructure instead. Monitor events are now logged via tracing spans in monitor.rs"
)]
pub struct MonitorLogger {
    file: File,
    log_path: PathBuf,
}

impl MonitorLogger {
    /// Create a new MonitorLogger
    /// Creates/opens ~/.devcli/logs/monitor.log
    pub async fn new() -> Result<Self> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        let log_dir = home.join(".devcli").join("logs");

        // Create log directory if it doesn't exist
        tokio::fs::create_dir_all(&log_dir)
            .await
            .context("Failed to create log directory")?;

        let log_path = log_dir.join("monitor.log");

        // Open in append mode - we want to keep historical monitor logs
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .await
            .context("Failed to open monitor log file")?;

        Ok(Self { file, log_path })
    }

    /// Write a timestamped log message
    async fn write_log(&mut self, message: &str) -> Result<()> {
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let log_line = format!("[{}] {}\n", timestamp, message);

        self.file
            .write_all(log_line.as_bytes())
            .await
            .context("Failed to write to monitor log")?;

        self.file
            .flush()
            .await
            .context("Failed to flush monitor log")?;

        Ok(())
    }

    /// Log a health check failure
    pub async fn log_health_check_failure(
        &mut self,
        project: &str,
        app: &str,
        check_type: &str,
        error: &str,
    ) -> Result<()> {
        let message = format!(
            "HEALTH_CHECK_FAILED: {}/{} - type={} error={}",
            project, app, check_type, error
        );
        self.write_log(&message).await
    }

    /// Log that a restart has been triggered
    pub async fn log_restart_triggered(
        &mut self,
        project: &str,
        app: &str,
        reason: &RestartReason,
        backoff_secs: u64,
        restart_count: u32,
    ) -> Result<()> {
        let reason_str = match reason {
            RestartReason::Crash { exit_code } => format!("crash (exit_code={})", exit_code),
            RestartReason::HealthCheckFailure { check_type } => {
                format!("health_check_failure (type={})", check_type)
            }
            RestartReason::Manual => "manual".to_string(),
        };

        let message = format!(
            "RESTART_TRIGGERED: {}/{} - reason={} backoff={}s attempt={}/total",
            project, app, reason_str, backoff_secs, restart_count
        );
        self.write_log(&message).await
    }

    /// Log a successful restart
    pub async fn log_restart_success(
        &mut self,
        project: &str,
        app: &str,
        new_pid: u32,
    ) -> Result<()> {
        let message = format!("RESTART_SUCCESS: {}/{} - new_pid={}", project, app, new_pid);
        self.write_log(&message).await
    }

    /// Log a failed restart attempt
    pub async fn log_restart_failure(
        &mut self,
        project: &str,
        app: &str,
        error: &str,
    ) -> Result<()> {
        let message = format!("RESTART_FAILED: {}/{} - error={}", project, app, error);
        self.write_log(&message).await
    }

    /// Log that max restarts limit has been reached
    pub async fn log_max_restarts_reached(
        &mut self,
        project: &str,
        app: &str,
        max_restarts: u32,
        window_secs: u64,
    ) -> Result<()> {
        let message = format!(
            "MAX_RESTARTS_REACHED: {}/{} - max={} window={}s - auto-restart disabled",
            project, app, max_restarts, window_secs
        );
        self.write_log(&message).await
    }

    /// Log health check success (recovery from failure state)
    pub async fn log_health_check_recovered(&mut self, project: &str, app: &str) -> Result<()> {
        let message = format!(
            "HEALTH_CHECK_RECOVERED: {}/{} - health checks now passing",
            project, app
        );
        self.write_log(&message).await
    }

    /// Log monitor daemon startup
    pub async fn log_monitor_started(&mut self) -> Result<()> {
        let message = "MONITOR_STARTED: Background monitor daemon initialized";
        self.write_log(message).await
    }

    /// Log monitor daemon shutdown
    pub async fn log_monitor_stopped(&mut self, reason: &str) -> Result<()> {
        let message = format!("MONITOR_STOPPED: {}", reason);
        self.write_log(&message).await
    }

    /// Get the path to the monitor log file
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_monitor_logger_creation() {
        let logger = MonitorLogger::new().await;
        assert!(logger.is_ok());
        let logger = logger.unwrap();
        assert!(logger.log_path().exists());
    }

    #[tokio::test]
    async fn test_log_health_check_failure() {
        let mut logger = MonitorLogger::new().await.unwrap();
        let result = logger
            .log_health_check_failure("test-project", "test-app", "http", "timeout")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_restart_triggered() {
        let mut logger = MonitorLogger::new().await.unwrap();
        let reason = RestartReason::Crash { exit_code: 1 };
        let result = logger
            .log_restart_triggered("test-project", "test-app", &reason, 5, 1)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_restart_success() {
        let mut logger = MonitorLogger::new().await.unwrap();
        let result = logger
            .log_restart_success("test-project", "test-app", 12345)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_monitor_lifecycle() {
        let mut logger = MonitorLogger::new().await.unwrap();

        let start = logger.log_monitor_started().await;
        assert!(start.is_ok());

        let stop = logger.log_monitor_stopped("no processes remaining").await;
        assert!(stop.is_ok());
    }
}
