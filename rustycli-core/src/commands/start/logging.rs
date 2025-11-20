//! Log aggregation and display
//! 
//! This module handles:
//! - Setting up log monitoring for started apps
//! - Managing detached vs non-detached modes
//! - Handling Ctrl+C signal for graceful exit

use crate::config::load_preferences;
use crate::Result;



/// Setup log monitoring based on detached mode preference
/// 
/// In detached mode: exits immediately after starting processes
/// In non-detached mode: keeps process alive to show logs until Ctrl+C or all processes exit
pub async fn setup_log_monitoring(started_apps: &[String], silent: bool) -> Result<()> {
    let preferences = load_preferences()?;
    
    if preferences.detached_mode {
        // In detached mode, we exit immediately after starting processes
        if !silent {
            println!("\nRunning in background (detached mode, no terminal output)");
        }
    } else {
        // In non-detached mode, keep the main process alive to show logs
        if !silent {
            println!("\nRunning in background with output streaming (use Ctrl+C to stop viewing)");
            
            // Explain to user what's happening and how to exit
            println!("Press Ctrl+C to stop viewing logs (processes will continue running)...\n");
        }
        
        // Wait for Ctrl+C signal or all processes to exit
        wait_for_interrupt_or_exit(started_apps).await?;
        
        if !silent {
            println!("\n\nStopped viewing logs. Processes may still be running in the background.");
            println!("Use 'rustycli status' to check process status.");
        }
    }
    
    Ok(())
}

/// Wait for Ctrl+C signal or all processes to exit
async fn wait_for_interrupt_or_exit(started_apps: &[String]) -> Result<()> {
    use crate::process::ProcessTracker;
    
    let tracker = ProcessTracker::new()?;
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    
    loop {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                match result {
                    Ok(()) => {
                        // User pressed Ctrl+C - this is expected
                        return Ok(());
                    }
                    Err(err) => {
                        // Something went wrong with signal handling
                        println!("Unable to listen for shutdown signal: {}", err);
                        return Err(err.into());
                    }
                }
            }
            _ = interval.tick() => {
                // Check if any of the started processes are still running
                let mut any_running = false;
                for app_name in started_apps {
                    if let Ok(Some(process)) = tracker.get_process(app_name) {
                        if tracker.is_running(process.pid) {
                            any_running = true;
                            break;
                        }
                    }
                }
                
                if !any_running {
                    println!("\n\nAll processes have exited.");
                    return Ok(());
                }
            }
        }
    }
}