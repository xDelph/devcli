// Log Manager for discovering and managing log files
// Handles log file discovery in ~/.devcli/logs/ directory
// Provides metadata about log files (name, size, modification date)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
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
}

impl LogManager {
    /// Creates a new LogManager
    /// Determines the log directory path but doesn't create it
    pub fn new() -> Result<Self> {
        // Get user's home directory
        let home = dirs::home_dir().context("Could not determine home directory")?;
        
        // Build the log directory path: ~/.devcli/logs/
        let log_dir = home.join(".devcli").join("logs");
        
        Ok(Self { log_dir })
    }

    /// Lists all log files for a specific app
    /// Searches for files matching the pattern: {app_name}_*.log
    /// Returns files sorted by modification date (newest first)
    pub fn list_logs_for_app(&self, app_name: &str) -> Result<Vec<LogFileInfo>> {
        // Check if log directory exists
        if !self.log_dir.exists() {
            // No logs directory means no logs yet
            return Ok(Vec::new());
        }

        let mut log_files = Vec::new();
        
        // Read all entries in the log directory
        // Use std::fs instead of tokio::fs since we're not in an async context
        let entries = std::fs::read_dir(&self.log_dir)
            .context("Failed to read log directory")?;

        // Pattern to match: {app_name}_*.log
        // For example, if app_name is "api-server", we match "api-server_20251114.log"
        let prefix = format!("{}_", app_name);
        
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
            // Must start with "{app_name}_" and end with ".log"
            if filename.starts_with(&prefix) && filename.ends_with(".log") {
                // Get file metadata for size and modification time
                let metadata = std::fs::metadata(&path)
                    .context("Failed to read file metadata")?;
                
                // Get modification time and convert to DateTime<Utc>
                let modified = metadata.modified()
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
        log_files.sort_by(|a, b| b.modified.cmp(&a.modified));
        
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

    /// Formats a date relative to now
    /// Examples: "Today 14:32", "Yesterday", "2 days ago", "Nov 12"
    pub fn format_relative_date(date: &DateTime<Utc>) -> String {
        let now = Utc::now();
        let duration = now.signed_duration_since(*date);
        
        // Calculate days difference
        let days = duration.num_days();
        
        if days == 0 {
            // Today - show time
            format!("Today {}", date.format("%H:%M"))
        } else if days == 1 {
            // Yesterday
            "Yesterday".to_string()
        } else if days < 7 {
            // Within a week - show "X days ago"
            format!("{} days ago", days)
        } else {
            // Older - show date
            date.format("%b %d").to_string()
        }
    }
}

impl Default for LogManager {
    fn default() -> Self {
        Self::new().expect("Failed to create LogManager")
    }
}

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
        let now = Utc::now();
        
        // Today
        let today = now;
        let formatted = LogManager::format_relative_date(&today);
        assert!(formatted.starts_with("Today"));
        
        // Yesterday
        let yesterday = now - chrono::Duration::days(1);
        assert_eq!(LogManager::format_relative_date(&yesterday), "Yesterday");
        
        // 3 days ago
        let three_days = now - chrono::Duration::days(3);
        assert_eq!(LogManager::format_relative_date(&three_days), "3 days ago");
        
        // 10 days ago (should show date)
        let ten_days = now - chrono::Duration::days(10);
        let formatted = LogManager::format_relative_date(&ten_days);
        assert!(!formatted.contains("days ago"));
        assert!(!formatted.contains("Today"));
    }
}
