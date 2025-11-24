// Import required functionality
use anyhow::{Context, Result}; // Error handling
use chrono::Utc; // UTC timezone for timestamps
use std::path::PathBuf; // File path handling
use tokio::fs::{File, OpenOptions}; // Async file operations
use tokio::io::AsyncWriteExt; // Trait for async write operations (write_all, flush)

// Handles writing logs to a file with timestamps
// Uses async I/O to avoid blocking when writing
pub struct FileLogger {
    file: File,        // The open file handle
    log_path: PathBuf, // Path to the log file (for reference)
}

impl FileLogger {
    // Create a new FileLogger for an app
    // This is async because file operations are async
    // truncate: if true, clears the file; if false, appends to existing content
    pub async fn new(app_name: &str, truncate: bool) -> Result<Self> {
        // Get user's home directory
        let home = dirs::home_dir().context("Could not determine home directory")?;

        // Build the log directory path: ~/.devcli/logs/
        let log_dir = home.join(".devcli").join("logs");

        // Create the directory if it doesn't exist
        // .await is required for async operations - it waits for completion
        tokio::fs::create_dir_all(&log_dir)
            .await
            .context("Failed to create log directory")?;

        // Get current date (no time) to have one log file per day
        // Format: 20251024
        let date = Utc::now().format("%Y%m%d");
        // Create filename: my-app_20251024.log
        // Multiple starts of same app on same day will append to this file
        let filename = format!("{}_{}.log", app_name, date);
        // Full path to the log file
        let log_path = log_dir.join(filename);

        // Open (or create) the log file
        // OpenOptions is like configuration for how to open the file
        let file = if truncate {
            // Truncate mode: clear the file if it exists
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&log_path)
                .await
                .context("Failed to open log file")?
        } else {
            // Append mode: keep existing content
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .await
                .context("Failed to open log file")?
        };

        // Return the new FileLogger
        Ok(Self { file, log_path })
    }

    // Write a log message with timestamp
    // &mut self means this method needs exclusive access to modify the FileLogger
    pub async fn write_log(&mut self, message: &str) -> Result<()> {
        // Get current time formatted: 2025-10-24 10:30:45.123
        // %.3f adds milliseconds
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        // Format the log line: [2025-10-24 10:30:45.123] message content
        let log_line = format!("[{}] {}\n", timestamp, message);

        // Write the log line to the file
        // .as_bytes() converts string to bytes (files work with bytes, not strings)
        self.file
            .write_all(log_line.as_bytes())
            .await
            .context("Failed to write to log file")?;

        // Flush ensures the data is actually written to disk immediately
        // Without flush, data might stay in a buffer temporarily
        // This is important so we don't lose logs if the program crashes
        self.file
            .flush()
            .await
            .context("Failed to flush log file")?;

        Ok(())
    }

    // Get the path to the log file
    // Returns a reference (&) to avoid copying the PathBuf
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }
}
