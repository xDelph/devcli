// Monitor command - Background daemon for monitoring process health
// Runs continuously in the background, checking process status
// Automatically exits when no processes are being tracked

use crate::process::{ProcessInfo, ProcessTracker};
use crate::Result;
use chrono::Utc;
use std::collections::HashMap;

// Main entry point for the monitor daemon
// This runs in a loop, checking process health every few seconds
// Args:
//   - daemon: If true, runs as a background daemon. If false, runs once and exits.
pub async fn monitor_command(daemon: bool) -> Result<()> {
    if daemon {
        // Run as daemon - continuous monitoring loop
        run_daemon_loop().await
    } else {
        // Run once - just do a single cleanup
        // This is useful for manual cleanup without starting the full daemon
        let tracker = ProcessTracker::new()?;
        let cleaned = tracker.cleanup_dead()?;
        
        if cleaned.is_empty() {
            println!("No dead processes found");
        } else {
            println!("Cleaned up {} dead process(es):", cleaned.len());
            for app_name in cleaned {
                println!("  - {}", app_name);
            }
        }
        
        Ok(())
    }
}

// Background daemon loop
// Continuously monitors process health and cleans up dead processes
// Exits automatically when no processes remain
async fn run_daemon_loop() -> Result<()> {
    let tracker = ProcessTracker::new()?;
    
    // Register the monitor itself as a tracked process
    // This allows other parts of the system to check if the monitor is running
    // We use a special name: ".monitor"
    let monitor_info = ProcessInfo {
        app_name: ".monitor".to_string(),
        pid: std::process::id(),
        command: "rustycli monitor --daemon".to_string(),
        working_dir: std::env::current_dir()?
            .to_string_lossy()
            .to_string(),
        start_time: Utc::now(),
        env_vars: HashMap::new(),
        project: None,
        app_config_name: Some(".monitor".to_string()),
        environment: None,
        command_variant: None,
    };
    
    tracker.register_process(monitor_info)?;
    
    // Main monitoring loop
    loop {
        // Clean up dead processes
        // This removes PID files for processes that are no longer running
        let _cleaned = tracker.cleanup_dead()?;
        
        // Check if there are any processes left (excluding the monitor itself)
        let processes = tracker.list_processes()?;
        let non_monitor_processes: Vec<_> = processes
            .iter()
            .filter(|p| p.app_name != ".monitor")
            .collect();
        
        if non_monitor_processes.is_empty() {
            // No processes left to monitor - exit gracefully
            // Remove our own PID file before exiting
            tracker.remove_process(".monitor")?;
            break;
        }
        
        // Sleep for 3 seconds before next check
        // This interval balances responsiveness with resource usage
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
    }
    
    Ok(())
}

