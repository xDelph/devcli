use crate::Result;
use anyhow::Context;
use chrono::Utc;
use process_manager::engine::OutputReceiver;
use process_manager::state::ManagedProcess;
use process_manager::{HealthCheck, StateStore};
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;

pub type OutputChannel = UnboundedSender<String>;

pub fn state_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".devcli").join("processes"))
}

pub fn state_store() -> Result<StateStore> {
    let base_dir = state_dir()?;
    StateStore::new(base_dir).context("Failed to initialize process manager state store")
}

pub fn logs_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let log_dir = home.join(".devcli").join("logs");
    std::fs::create_dir_all(&log_dir).context("Failed to create log directory")?;
    Ok(log_dir)
}

pub fn log_file_path(project: &str, app_name: &str, environment: &str) -> Result<PathBuf> {
    let log_dir = logs_dir()?;
    let date = Utc::now().format("%Y%m%d");
    let filename = format!("{}_{}_{}_{}.log", project, app_name, environment, date);
    Ok(log_dir.join(filename))
}

/// Path to the devcli binary for subprocess invocation.
///
/// Prefers `DEVCLI_BIN` when set, otherwise uses `current_exe()`.
/// Use this instead of hardcoding `"devcli"` so installs, wrappers, and aliases work.
pub fn devcli_binary_path() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("DEVCLI_BIN") {
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    std::env::current_exe().context("Could not determine devcli binary path")
}

pub fn process_id(project: &str, app_name: &str, environment: &str) -> String {
    format!("{}.{}.{}", project, app_name, environment)
}

pub fn find_process(
    store: &StateStore,
    project: &str,
    app_name: &str,
    environment: Option<&str>,
) -> Result<Option<ManagedProcess>> {
    let candidates = store.find_by_metadata("project", project)?;
    for proc in candidates {
        if proc.metadata.get("app_config_name").map(String::as_str) != Some(app_name) {
            continue;
        }

        if let Some(env) = environment {
            if proc.metadata.get("environment").map(String::as_str) != Some(env) {
                continue;
            }
        }

        return Ok(Some(proc));
    }

    Ok(None)
}

pub fn emit_line(silent: bool, output_tx: Option<&OutputChannel>, line: impl Into<String>) {
    let line = line.into();
    if !silent {
        println!("{}", line);
    }
    if let Some(tx) = output_tx {
        let _ = tx.send(line);
    }
}

pub fn emit_error(silent: bool, output_tx: Option<&OutputChannel>, line: impl Into<String>) {
    let line = line.into();
    if !silent {
        eprintln!("{}", line);
    }
    if let Some(tx) = output_tx {
        let _ = tx.send(format!("[stderr] {}", line));
    }
}

pub async fn stream_output(
    mut rx: OutputReceiver,
    show_output: bool,
    display_name: &str,
    output_tx: Option<&OutputChannel>,
) {
    while let Some(msg) = rx.recv().await {
        let prefix = match msg.source {
            process_manager::model::OutputSource::Stdout => "stdout",
            process_manager::model::OutputSource::Stderr => "stderr",
        };

        let formatted = format!("[{}][{}] {}", display_name, prefix, msg.content);

        if show_output {
            if prefix == "stderr" {
                eprintln!("{}", formatted);
            } else {
                println!("{}", formatted);
            }
        }

        if let Some(tx) = output_tx {
            let _ = tx.send(formatted);
        }
    }
}

pub fn to_pm_health_check(health: Option<&crate::config::models::HealthCheck>) -> HealthCheck {
    match health {
        Some(crate::config::models::HealthCheck::Http {
            url,
            timeout_secs,
            expected_status,
        }) => HealthCheck::Http {
            url: url.clone(),
            timeout_secs: *timeout_secs,
            expected_status: *expected_status,
        },
        Some(crate::config::models::HealthCheck::Tcp {
            host,
            port,
            timeout_secs,
        }) => HealthCheck::Tcp {
            host: host.clone(),
            port: *port,
            timeout_secs: *timeout_secs,
        },
        Some(crate::config::models::HealthCheck::Command {
            command,
            timeout_secs,
            expected_exit_code,
        }) => HealthCheck::Command {
            command: command.clone(),
            timeout_secs: *timeout_secs,
            expected_exit_code: *expected_exit_code,
        },
        Some(crate::config::models::HealthCheck::Process {}) | None => HealthCheck::Process {},
    }
}
