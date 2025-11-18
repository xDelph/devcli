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
/// In non-detached mode: keeps process alive to show logs until Ctrl+C
pub async fn setup_log_monitoring(_started_apps: &[String], silent: bool) -> Result<()> {
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
        
        // Wait for Ctrl+C signal
        wait_for_interrupt().await?;
        
        if !silent {
            println!("\n\nStopped viewing logs. Processes are still running in the background.");
            println!("Use 'rustycli status' to check process status.");
        }
    }
    
    Ok(())
}

/// Wait for Ctrl+C signal using tokio's async signal handling
async fn wait_for_interrupt() -> Result<()> {
    let ctrl_c = tokio::signal::ctrl_c();
    
    match ctrl_c.await {
        Ok(()) => {
            // User pressed Ctrl+C - this is expected
            Ok(())
        }
        Err(err) => {
            // Something went wrong with signal handling
            println!("Unable to listen for shutdown signal: {}", err);
            Err(err.into())
        }
    }
}