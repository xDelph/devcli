// Import required external functionality
use anyhow::{Context, Result}; // Error handling
use chrono::{DateTime, Utc}; // Date/time handling with timezone support
use serde::{Deserialize, Serialize}; // For converting structs to/from JSON
use std::collections::HashMap; // Key-value map for environment variables
use std::fs; // File system operations (read/write/delete files)
use std::path::PathBuf; // Cross-platform file path handling

// Information about a tracked process
// Serialize/Deserialize allow converting to/from JSON
// This data gets saved to ~/.rustycli/pids/<app-name>.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub app_name: String,
    pub pid: u32,
    pub command: String,
    pub working_dir: String,
    pub start_time: DateTime<Utc>,
    pub env_vars: HashMap<String, String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub app_config_name: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub command_variant: Option<String>,
}

// Manages tracking of spawned processes via PID files
// Stores files in ~/.rustycli/pids/
pub struct ProcessTracker {
    base_dir: PathBuf, // Path to the directory where PID files are stored
}

// Implementation block - contains all methods for ProcessTracker
impl ProcessTracker {
    // Constructor: creates a new ProcessTracker
    // Returns Result because getting home directory or creating directories might fail
    pub fn new() -> Result<Self> {
        // Get the user's home directory (e.g., /Users/thomas)
        // The ? operator returns the error if this fails
        let home = dirs::home_dir().context("Could not determine home directory")?;

        // Build the path: ~/.rustycli/pids/
        // .join() appends path segments in a platform-independent way
        let base_dir = home.join(".rustycli").join("pids");

        // Create the directory if it doesn't exist
        // create_dir_all is like 'mkdir -p' - creates parent directories too
        fs::create_dir_all(&base_dir).context("Failed to create PID tracking directory")?;

        // Return a new ProcessTracker with this base directory
        Ok(Self { base_dir })
    }

    // Private helper method to get the full path to a PID file
    // &self means this method needs a ProcessTracker instance
    // &str is a string slice (borrowed reference to a string)
    fn pid_file_path(&self, app_name: &str) -> PathBuf {
        // format! creates a string like "my-app.json"
        // .join() appends it to the base directory
        self.base_dir.join(format!("{}.json", app_name))
    }

    // Save a process to a PID file
    // Result<()> means returns Result<T> where T is the empty tuple ()
    // () in Rust is like 'void' in other languages - means no return value
    pub fn register_process(&self, info: ProcessInfo) -> Result<()> {
        // Get the full path where we'll save this process's info
        let path = self.pid_file_path(&info.app_name);

        // Convert the ProcessInfo struct to pretty-printed JSON string
        // serde_json does the heavy lifting of serialization
        let json =
            serde_json::to_string_pretty(&info).context("Failed to serialize process info")?;

        // Write the JSON string to the file
        // This will overwrite if the file already exists
        fs::write(&path, json).context("Failed to write PID file")?;

        // Notify watchers that a new process was registered
        self.notify_status_change()?;

        // Return Ok(()) to indicate success
        Ok(())
    }

    // Retrieve process info from a PID file
    // Returns Option because the process might not exist
    // Option<ProcessInfo> means either Some(ProcessInfo) or None
    pub fn get_process(&self, app_name: &str) -> Result<Option<ProcessInfo>> {
        let path = self.pid_file_path(app_name);

        // Check if the PID file exists
        if !path.exists() {
            // If not, return Ok(None) - this isn't an error, just no data
            return Ok(None);
        }

        // Read the file contents as a string
        let content = fs::read_to_string(&path).context("Failed to read PID file")?;

        // Parse the JSON string back into a ProcessInfo struct
        // The type annotation tells serde what structure to parse into
        let info: ProcessInfo =
            serde_json::from_str(&content).context("Failed to parse PID file")?;

        // Return the process info wrapped in Some
        Ok(Some(info))
    }

    // List all tracked processes by reading all PID files
    // Returns a Vec (vector/dynamic array) of ProcessInfo
    pub fn list_processes(&self) -> Result<Vec<ProcessInfo>> {
        // Create an empty vector to store processes
        // Vec::new() creates a new, empty vector
        // 'mut' allows us to modify it (add items)
        let mut processes = Vec::new();

        // Read all entries in the PID directory
        // Returns an iterator over directory entries
        let entries = fs::read_dir(&self.base_dir).context("Failed to read PID directory")?;

        // Iterate over each directory entry
        for entry in entries {
            // Get the actual entry (this can fail if permissions are wrong, etc.)
            let entry = entry.context("Failed to read directory entry")?;
            // Get the full path to this file
            let path = entry.path();

            // Check if this is a .json file
            // .extension() returns Option<&OsStr>, .and_then() chains operations
            // .to_str() converts to regular string, == Some("json") checks value
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                // Try to read and parse the file
                // 'if let Ok(...)' only runs if the operation succeeds
                // This gracefully skips corrupted files instead of failing
                if let Ok(content) = fs::read_to_string(&path) {
                    // Try to parse the JSON
                    // The ::<ProcessInfo> syntax specifies the type to parse into
                    if let Ok(info) = serde_json::from_str::<ProcessInfo>(&content) {
                        // Add this process to our list
                        processes.push(info);
                    }
                }
            }
        }

        // Return all the processes we found
        Ok(processes)
    }

    // Check if a process is still running
    // Returns bool: true if running, false if not
    pub fn is_running(&self, pid: u32) -> bool {
        // Platform-specific code for Unix (macOS, Linux)
        #[cfg(unix)]
        {
            use std::process::Command;
            // Use the 'kill' command with signal 0
            // Signal 0 doesn't actually kill, it just checks if process exists
            Command::new("kill")
                .arg("-0") // Signal 0 = check if process exists
                .arg(pid.to_string()) // Convert PID to string
                .output() // Execute and get output
                // .map transforms Result<Output, Error> to bool
                .map(|output| output.status.success()) // true if exit code was 0
                // .unwrap_or provides a default value if command failed
                .unwrap_or(false) // If we can't run kill, assume not running
        }

        // For non-Unix platforms (Windows)
        #[cfg(not(unix))]
        {
            // We don't support Windows yet, so always return false
            // TODO: Implement Windows process checking
            false
        }
    }

    // Remove a PID file (delete the tracking for a process)
    pub fn remove_process(&self, app_name: &str) -> Result<()> {
        let path = self.pid_file_path(app_name);

        // Only try to delete if the file exists
        // This prevents errors when trying to remove a non-existent file
        if path.exists() {
            fs::remove_file(&path).context("Failed to remove PID file")?;
            // Notify watchers that a process was removed
            self.notify_status_change()?;
        }

        Ok(())
    }

    // Remove PID files for processes that are no longer running
    // Returns a Vec of app names that were cleaned up
    pub fn cleanup_dead(&self) -> Result<Vec<String>> {
        // Get all tracked processes
        let processes = self.list_processes()?;
        // Create a list to store names of processes we clean up
        let mut cleaned = Vec::new();

        // Check each process
        for process in processes {
            // If the process is NOT running anymore...
            if !self.is_running(process.pid) {
                // Remove its PID file
                self.remove_process(&process.app_name)?;
                // Add its name to the cleaned list
                cleaned.push(process.app_name);
            }
        }

        // If any processes were cleaned up, notify watchers
        if !cleaned.is_empty() {
            self.notify_status_change()?;
        }

        // Return the list of cleaned process names
        Ok(cleaned)
    }

    // Get the path to the status notification file
    // This file is touched whenever process status changes
    fn status_notification_path(&self) -> PathBuf {
        self.base_dir.join(".status_changed")
    }

    // Notify watchers that process status has changed
    // Updates the modification time of a notification file
    pub fn notify_status_change(&self) -> Result<()> {
        let path = self.status_notification_path();
        
        // Touch the file to update its modification time
        // This is a lightweight way to signal changes across processes
        if path.exists() {
            // Update modification time
            let now = std::time::SystemTime::now();
            filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(now))
                .context("Failed to update status notification file")?;
        } else {
            // Create the file if it doesn't exist
            fs::write(&path, "").context("Failed to create status notification file")?;
        }
        
        Ok(())
    }

    // Get the last modification time of the status notification file
    // Returns None if the file doesn't exist
    pub fn get_last_status_change(&self) -> Result<Option<std::time::SystemTime>> {
        let path = self.status_notification_path();
        
        if !path.exists() {
            return Ok(None);
        }
        
        let metadata = fs::metadata(&path).context("Failed to read status notification file")?;
        let modified = metadata.modified().context("Failed to get modification time")?;
        
        Ok(Some(modified))
    }
}

// Implement the Default trait for ProcessTracker
// This allows creating a ProcessTracker with ProcessTracker::default()
// Default is a standard Rust trait for types that have a "default" value
impl Default for ProcessTracker {
    fn default() -> Self {
        // Just call new() and panic if it fails
        // expect() is like unwrap() but lets us provide a custom error message
        // We use expect here because if we can't create a tracker, the app can't function
        Self::new().expect("Failed to create ProcessTracker")
    }
}
