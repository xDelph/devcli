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

/// Manages log file discovery and access
/// Discovers log files in the ~/.devcli/logs/ directory
pub struct LogManager {
    /// Path to the log directory (~/.devcli/logs/)
    log_dir: PathBuf,
    /// Last known modification time of the logs directory
    /// Used to detect when new log files are created
    last_dir_modified: Option<DateTime<Utc>>,
}

impl LogManager {
    /// Creates a new LogManager
    /// Determines the log directory path but doesn't create it
    pub fn new() -> Result<Self> {
        // Get user's home directory
        let home = dirs::home_dir().context("Could not determine home directory")?;

        // Build the log directory path: ~/.devcli/logs/
        let log_dir = home.join(".devcli").join("logs");

        Ok(Self {
            log_dir,
            last_dir_modified: None,
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
        // Check if log directory exists
        if !self.log_dir.exists() {
            // No logs directory means no logs yet
            return Ok(Vec::new());
        }

        let mut log_files = Vec::new();

        // Read all entries in the log directory
        // Use std::fs instead of tokio::fs since we're not in an async context
        let entries = std::fs::read_dir(&self.log_dir).context("Failed to read log directory")?;

        // Pattern to match
        let prefix = format!("{}_{}_", project_name, app_name);

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

            // Check if filename matches our pattern
            if filename.starts_with(&prefix) && filename.ends_with(".log") {
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
        // Try to extract context from filename
        // Format: project_app_context_YYYYMMDD.log
        let without_ext = filename.trim_end_matches(".log");
        let parts: Vec<&str> = without_ext.split('_').collect();

        // We need at least 4 parts: project, app, context, date
        if parts.len() >= 4 {
            // Context is the third-to-last part (before the date)
            let context = parts[parts.len() - 2];
            let date_str = parts.last().unwrap_or(&"unknown");

            // Parse date string YYYYMMDD
            if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y%m%d") {
                let smart_date = Self::format_relative_date(date);
                return format!("{} - {}", context, smart_date);
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
