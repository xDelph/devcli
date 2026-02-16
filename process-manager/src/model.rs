use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a unit of work to be executed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    /// Unique identifier for this task.
    pub id: String,
    /// The command to execute.
    pub command: String,
    /// Arguments for the command.
    pub args: Vec<String>,
    /// Working directory for the process.
    pub working_dir: PathBuf,
    /// Environment variables to set for the process.
    pub env: HashMap<String, String>,
    /// Whether the process should be detached from the parent session.
    pub is_detached: bool,
    /// Path to the log file for stdout/stderr redirection.
    pub log_file: Option<PathBuf>,
    /// Health check configuration.
    #[serde(default = "default_health_check")]
    pub health_check: HealthCheck,
    /// Restart behavior configuration.
    #[serde(default)]
    pub restart_policy: RestartPolicy,
}

/// Configuration for health checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HealthCheck {
    /// Check an HTTP endpoint.
    Http {
        url: String,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
        #[serde(default = "default_status")]
        expected_status: u16,
    },
    /// Check a TCP port.
    Tcp {
        host: String,
        port: u16,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
    },
    /// Run a custom command.
    Command {
        command: String,
        #[serde(default = "default_timeout")]
        timeout_secs: u64,
        #[serde(default = "default_exit_code")]
        expected_exit_code: i32,
    },
    /// Just check if the process is running (default).
    Process {},
}

fn default_health_check() -> HealthCheck {
    HealthCheck::Process {}
}

fn default_timeout() -> u64 {
    5
}
fn default_status() -> u16 {
    200
}
fn default_exit_code() -> i32 {
    0
}

/// Configuration for process restart behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestartPolicy {
    /// Maximum number of restarts allowed within the window.
    pub max_restarts: u32,
    /// The time window in seconds for the max_restarts count.
    pub restart_window_secs: u64,
    /// Initial backoff delay in seconds.
    pub initial_backoff_secs: u64,
    /// Maximum backoff delay in seconds.
    pub max_backoff_secs: u64,
    /// Multiplier for the backoff calculation (exponential).
    pub backoff_multiplier: f64,
    /// Exit codes that trigger a restart (None means all non-zero).
    pub restart_on_exit_codes: Option<Vec<i32>>,
    /// Whether auto-restart is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_restarts: 3,
            restart_window_secs: 300,
            initial_backoff_secs: 1,
            max_backoff_secs: 60,
            backoff_multiplier: 2.0,
            restart_on_exit_codes: None,
            enabled: true,
        }
    }
}

/// Metadata map for tagging processes (e.g., project="my-app", env="prod").
pub type ProcessMetadata = HashMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OutputSource {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputMessage {
    pub source: OutputSource,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
