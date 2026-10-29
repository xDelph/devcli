use crate::Result;
use anyhow::Context;
use process_manager::engine::OutputReceiver;
use process_manager::state::ManagedProcess;
use process_manager::{HealthCheck, RestartPolicy, StateStore};
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;

pub type OutputChannel = UnboundedSender<String>;

pub fn state_dir() -> Result<PathBuf> {
    let dir = crate::config::loader::config_home().join("processes");
    std::fs::create_dir_all(&dir).context("Failed to create process state directory")?;
    Ok(dir)
}

pub fn state_store() -> Result<StateStore> {
    let base_dir = state_dir()?;
    StateStore::new(base_dir).context("Failed to initialize process manager state store")
}

pub fn logs_dir() -> Result<PathBuf> {
    let log_dir = crate::config::loader::config_home().join("logs");
    std::fs::create_dir_all(&log_dir).context("Failed to create log directory")?;
    Ok(log_dir)
}

/// The parts of a log file name, `{project}_{app}_{environment}_{YYYYMMDD}.log`.
///
/// Parsed from the right: environment names never contain `_` and the date is
/// eight digits, so those are unambiguous even when the project or app name
/// contains underscores (`proj_api_worker_local_20260101.log`). Splitting on
/// `_` from the left, as four call sites used to, read app `api_worker` as
/// `api`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogFileName {
    /// `{project}_{app}`
    pub stem: String,
    pub environment: String,
    /// `YYYYMMDD`
    pub date: String,
}

impl LogFileName {
    pub fn parse(file_name: &str) -> Option<Self> {
        let rest = file_name.strip_suffix(".log")?;
        let (rest, date) = rest.rsplit_once('_')?;
        let (stem, environment) = rest.rsplit_once('_')?;
        let date_ok = date.len() == 8 && date.chars().all(|c| c.is_ascii_digit());
        if !date_ok || environment.is_empty() || stem.is_empty() {
            return None;
        }
        Some(Self {
            stem: stem.to_string(),
            environment: environment.to_string(),
            date: date.to_string(),
        })
    }

    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        Self::parse(path.file_name()?.to_str()?)
    }

    /// Whether this is a log of `project/app` (exact, not a prefix match:
    /// `proj_api_` is also the start of app `api_worker`'s logs).
    pub fn belongs_to(&self, project: &str, app: &str) -> bool {
        self.stem.len() == project.len() + 1 + app.len()
            && self.stem.starts_with(project)
            && self.stem[project.len()..].starts_with('_')
            && self.stem.ends_with(app)
    }

    /// `(project, app)` when only the file name is known: split at the first
    /// `_`. Exact for project names without underscores.
    pub fn guess_project_app(&self) -> (String, String) {
        match self.stem.split_once('_') {
            Some((project, app)) => (project.to_string(), app.to_string()),
            None => (String::new(), self.stem.clone()),
        }
    }
}

pub fn log_file_path(project: &str, app_name: &str, environment: &str) -> Result<PathBuf> {
    let log_dir = logs_dir()?;
    // The user's local day: the TUI labels files "Today"/"Yesterday" against
    // the local date, so a UTC date put the wrong label on files written
    // around midnight.
    let date = chrono::Local::now().format("%Y%m%d");
    let filename = format!("{}_{}_{}_{}.log", project, app_name, environment, date);
    Ok(log_dir.join(filename))
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

/// Print a human progress line (suppressed with `--json`) and forward it to the
/// TUI channel when there is one.
pub fn emit_line(silent: bool, output_tx: Option<&OutputChannel>, line: impl Into<String>) {
    let line = line.into();
    if !silent && !crate::output::json_enabled() {
        println!("{}", line);
    }
    if let Some(tx) = output_tx {
        let _ = tx.send(line);
    }
}

pub fn emit_error(silent: bool, output_tx: Option<&OutputChannel>, line: impl Into<String>) {
    let line = line.into();
    if !silent && !crate::output::json_enabled() {
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

pub fn to_pm_restart_policy(
    policy: Option<&crate::config::models::RestartPolicy>,
) -> RestartPolicy {
    match policy {
        Some(policy) => RestartPolicy {
            enabled: policy.enabled,
            max_restarts: policy.max_restarts,
            restart_window_secs: policy.restart_window_secs,
            initial_backoff_secs: policy.initial_backoff_secs,
            max_backoff_secs: policy.max_backoff_secs,
            backoff_multiplier: policy.backoff_multiplier,
            restart_on_exit_codes: policy.restart_on_exit_codes.clone(),
        },
        None => RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_file_names_parse_from_the_right() {
        let name = LogFileName::parse("proj_api_worker_docker-compose_20260101.log").unwrap();
        assert_eq!(name.stem, "proj_api_worker");
        assert_eq!(name.environment, "docker-compose");
        assert_eq!(name.date, "20260101");
        assert!(name.belongs_to("proj", "api_worker"));
        assert!(!name.belongs_to("proj", "api"));
        assert_eq!(
            name.guess_project_app(),
            ("proj".to_string(), "api_worker".to_string())
        );

        assert!(LogFileName::parse("proj_api_local.log").is_none());
        assert!(LogFileName::parse("proj_api_local_20260101.log.1").is_none());
        assert!(LogFileName::parse("notes.txt").is_none());
    }
}
