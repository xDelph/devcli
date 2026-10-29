// Log Manager for discovering and managing log files
// Handles log file discovery in ~/.devcli/logs/ directory
// Provides metadata about log files (name, size, modification date)

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use std::path::PathBuf;

/// Information about a log file
/// Contains metadata needed to display log files in the UI
#[derive(Debug, Clone)]
pub struct LogFileInfo {
    /// Full path to the log file
    pub path: PathBuf,
    /// Display name (filename without path)
    pub name: String,
    /// File size in bytes
    pub size: u64,
    /// Last modification time
    pub modified: DateTime<Utc>,
}

/// Log listings by `(project, app)`, with when they were taken.
type ListingCache =
    std::collections::HashMap<(String, String), (std::time::Instant, Vec<LogFileInfo>)>;

/// Manages log file discovery and access
/// Discovers log files in the ~/.devcli/logs/ directory
pub struct LogManager {
    /// Path to the log directory (~/.devcli/logs/)
    log_dir: PathBuf,
    /// Last known modification time of the logs directory
    /// Used to detect when new log files are created
    last_dir_modified: Option<DateTime<Utc>>,
    /// Recent listings per `(project, app)`: the status panel asks on every
    /// redraw, which used to cost a directory read plus a `stat` per file.
    listing_cache: std::cell::RefCell<ListingCache>,
}

impl LogManager {
    /// Creates a new LogManager
    /// Determines the log directory path but doesn't create it
    pub fn new() -> Result<Self> {
        // The directory the apps' logs are written to. It used to be hard-coded
        // to ~/.devcli/logs, so with DEVCLI_CONFIG_DIR (or a project-local
        // ./.devcli) the TUI showed "No logs available" for every app.
        // Only the path: nothing is created here (listing a directory that
        // does not exist yet simply finds no logs).
        let log_dir = crate::config::loader::config_home().join("logs");

        Ok(Self {
            log_dir,
            last_dir_modified: None,
            listing_cache: Default::default(),
        })
    }

    /// Lists all log files for a specific app
    /// Searches for files matching the pattern:
    /// - Format: {project_name}_{app_name}_{context}_{date}.log
    ///   Returns files sorted by modification date (newest first)
    pub fn list_logs_for_app(
        &self,
        project_name: &str,
        app_name: &str,
    ) -> Result<Vec<LogFileInfo>> {
        const FRESH_FOR: std::time::Duration = std::time::Duration::from_secs(1);
        let key = (project_name.to_string(), app_name.to_string());
        if let Some((at, logs)) = self.listing_cache.borrow().get(&key) {
            if at.elapsed() < FRESH_FOR {
                return Ok(logs.clone());
            }
        }
        let logs = self.scan_logs_for_app(project_name, app_name)?;
        self.listing_cache
            .borrow_mut()
            .insert(key, (std::time::Instant::now(), logs.clone()));
        Ok(logs)
    }

    fn scan_logs_for_app(&self, project_name: &str, app_name: &str) -> Result<Vec<LogFileInfo>> {
        // Check if log directory exists
        if !self.log_dir.exists() {
            // No logs directory means no logs yet
            return Ok(Vec::new());
        }

        let mut log_files = Vec::new();

        // Read all entries in the log directory
        // Use std::fs instead of tokio::fs since we're not in an async context
        let entries = std::fs::read_dir(&self.log_dir).context("Failed to read log directory")?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            // Skip if not a file
            if !path.is_file() {
                continue;
            }

            // Get filename as string
            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => continue, // Skip if filename is invalid UTF-8
            };

            // Exactly this app's logs (a prefix match also caught `api_worker`
            // under `api`).
            if crate::process_manager_support::LogFileName::parse(filename)
                .is_some_and(|name| name.belongs_to(project_name, app_name))
            {
                // Get file metadata for size and modification time
                let metadata = std::fs::metadata(&path).context("Failed to read file metadata")?;

                // Get modification time and convert to DateTime<Utc>
                let modified = metadata
                    .modified()
                    .context("Failed to get modification time")?;
                let modified: DateTime<Utc> = modified.into();

                log_files.push(LogFileInfo {
                    path: path.clone(),
                    name: filename.to_string(),
                    size: metadata.len(),
                    modified,
                });
            }
        }

        // Sort by modification date, newest first
        // This ensures the most recent logs appear at the top
        log_files.sort_by_key(|a| std::cmp::Reverse(a.modified));

        Ok(log_files)
    }

    /// Formats a file size in bytes to a human-readable string
    /// Examples: "1.2 KB", "3.5 MB", "1.1 GB"
    pub fn format_file_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Formats a date relative to now (ignoring time)
    pub fn format_relative_date(date: NaiveDate) -> String {
        // Today is local date for comparison
        let today = chrono::Local::now().naive_local().date();
        let diff = today.signed_duration_since(date).num_days();

        if diff == 0 {
            "Today".to_string()
        } else if diff == 1 {
            "Yesterday".to_string()
        } else if diff < 7 {
            format!("{} days ago", diff)
        } else {
            date.format("%Y-%m-%d").to_string()
        }
    }

    /// Formats a log filename into a display label
    /// Filename format: {project}_{app}_{context}_{date}.log
    /// Output: "{context} - {Today/Yesterday/X days ago}"
    pub fn format_log_label(filename: &str) -> String {
        if let Some(name) = crate::process_manager_support::LogFileName::parse(filename) {
            if let Ok(date) = NaiveDate::parse_from_str(&name.date, "%Y%m%d") {
                let smart_date = Self::format_relative_date(date);
                return format!("{} - {}", name.environment, smart_date);
            }
        }

        filename.to_string()
    }

    /// Checks if the logs directory has been modified since last check
    /// Returns true if the directory was modified or if this is the first check
    pub fn has_logs_dir_changed(&mut self) -> bool {
        // Get current modification time of logs directory
        let current_modified = if self.log_dir.exists() {
            std::fs::metadata(&self.log_dir)
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dt: DateTime<Utc> = t.into();
                    dt
                })
        } else {
            None
        };

        // Check if it changed
        let changed = current_modified != self.last_dir_modified;

        // Update our tracked time
        self.last_dir_modified = current_modified;

        changed
    }

    /// Returns the path to the logs directory
    pub fn log_dir_path(&self) -> &PathBuf {
        &self.log_dir
    }
}

// Note: Default trait implementation removed because LogManager::new() can fail
// Use LogManager::new()? instead of LogManager::default()

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_file_size() {
        assert_eq!(LogManager::format_file_size(500), "500 B");
        assert_eq!(LogManager::format_file_size(1024), "1.0 KB");
        assert_eq!(LogManager::format_file_size(1536), "1.5 KB");
        assert_eq!(LogManager::format_file_size(1024 * 1024), "1.0 MB");
        assert_eq!(LogManager::format_file_size(2_500_000), "2.4 MB");
        assert_eq!(LogManager::format_file_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_format_relative_date() {
        let now = chrono::Local::now().date_naive();

        // Today
        let today = now;
        let formatted = LogManager::format_relative_date(today);
        assert_eq!(formatted, "Today");

        // Yesterday
        let yesterday = now - chrono::Duration::days(1);
        assert_eq!(LogManager::format_relative_date(yesterday), "Yesterday");

        // 3 days ago
        let three_days = now - chrono::Duration::days(3);
        assert_eq!(LogManager::format_relative_date(three_days), "3 days ago");

        // 10 days ago (should show date)
        let ten_days = now - chrono::Duration::days(10);
        let formatted = LogManager::format_relative_date(ten_days);
        assert!(!formatted.contains("days ago"));
        assert!(!formatted.contains("Today"));
    }
}
