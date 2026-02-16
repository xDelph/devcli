use crate::model::{RestartPolicy, Task};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedProcess {
    pub id: String,
    pub pid: u32,
    /// Process Group ID - used to track child processes if the parent exits
    pub pgid: Option<i32>,
    pub task: Task,
    pub start_time: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
    #[serde(default)]
    pub runtime: ProcessRuntime,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProcessRuntime {
    pub restart_count: u32,
    pub history: Vec<RestartEvent>,
    pub last_exit_code: Option<i32>,
    pub last_exit_time: Option<DateTime<Utc>>,
    pub health_failures: u32,
    pub last_health_check: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartEvent {
    pub timestamp: DateTime<Utc>,
    pub reason: String,
}

pub struct StateStore {
    base_dir: PathBuf,
}

impl StateStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    pub fn save(&self, process: &ManagedProcess) -> Result<()> {
        let path = self.base_dir.join(format!("{}.json", process.id));
        let json = serde_json::to_string_pretty(process)?;
        fs::write(path, json)?;
        self.notify_change()?;
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<Option<ManagedProcess>> {
        let path = self.base_dir.join(format!("{}.json", id));
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(path)?;
        let process = serde_json::from_str(&content)?;
        Ok(Some(process))
    }

    pub fn list(&self) -> Result<Vec<ManagedProcess>> {
        let mut processes = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(p) = serde_json::from_str(&content) {
                        processes.push(p);
                    }
                }
            }
        }
        Ok(processes)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.base_dir.join(format!("{}.json", id));
        if path.exists() {
            fs::remove_file(path)?;
            self.notify_change()?;
        }
        Ok(())
    }

    /// Check if a process or its group is still running
    pub fn is_running(&self, process: &ManagedProcess) -> bool {
        #[cfg(unix)]
        {
            use std::process::Command;

            // 1. Check the specific PID first
            let pid_running = Command::new("kill")
                .arg("-0")
                .arg(process.pid.to_string())
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if pid_running {
                return true;
            }

            // 2. If PID is dead, but we have a PGID, check the whole group
            // This handles cases where a wrapper script exits but children stay alive
            if let Some(pgid) = process.pgid {
                return Command::new("kill")
                    .arg("-0")
                    .arg(format!("-{}", pgid)) // Negative PID means PGID in kill command
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
            }

            false
        }
        #[cfg(not(unix))]
        {
            false // TODO: Windows support
        }
    }

    fn notify_change(&self) -> Result<()> {
        let path = self.base_dir.join(".status_changed");
        if path.exists() {
            let now = std::time::SystemTime::now();
            filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(now))?;
        } else {
            fs::write(&path, "")?;
        }
        Ok(())
    }

    pub fn cleanup_dead(&self) -> Result<Vec<String>> {
        let mut cleaned = Vec::new();
        for proc in self.list()? {
            if !self.is_running(&proc) {
                self.delete(&proc.id)?;
                cleaned.push(proc.id);
            }
        }
        Ok(cleaned)
    }

    /// Find processes by metadata key-value pair
    /// Returns all processes where metadata[key] == value
    pub fn find_by_metadata(&self, key: &str, value: &str) -> Result<Vec<ManagedProcess>> {
        let all_processes = self.list()?;
        Ok(all_processes
            .into_iter()
            .filter(|proc| proc.metadata.get(key).map(|v| v.as_str()) == Some(value))
            .collect())
    }

    /// Find a single process by metadata key-value pair
    /// Returns the first matching process, or None if no match found
    pub fn find_one_by_metadata(&self, key: &str, value: &str) -> Result<Option<ManagedProcess>> {
        let all_processes = self.list()?;
        Ok(all_processes
            .into_iter()
            .find(|proc| proc.metadata.get(key).map(|v| v.as_str()) == Some(value)))
    }
}

impl ManagedProcess {
    pub fn should_restart(&self, policy: &RestartPolicy) -> bool {
        if !policy.enabled {
            return false;
        }

        if let Some(code) = self.runtime.last_exit_code {
            if let Some(codes) = &policy.restart_on_exit_codes {
                if !codes.contains(&code) {
                    return false;
                }
            } else if code == 0 {
                return false;
            }
        }

        if policy.max_restarts > 0 {
            let window_start =
                Utc::now() - chrono::Duration::seconds(policy.restart_window_secs as i64);
            let recent = self
                .runtime
                .history
                .iter()
                .filter(|e| e.timestamp > window_start)
                .count() as u32;
            if recent >= policy.max_restarts {
                return false;
            }
        }
        true
    }

    pub fn calculate_backoff(&self, policy: &RestartPolicy) -> std::time::Duration {
        let attempts = self.runtime.restart_count.max(1);
        let backoff = policy.initial_backoff_secs as f64
            * policy.backoff_multiplier.powi((attempts - 1) as i32);
        let capped = backoff.min(policy.max_backoff_secs as f64);
        std::time::Duration::from_secs(capped as u64)
    }
}
