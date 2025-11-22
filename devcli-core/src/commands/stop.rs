// Stop command - Stop running applications
// Allows stopping one or more running applications
//
// Example: devcli stop api
// Example: devcli stop api --project qm
// Example: devcli stop --all
// Flow: Find running process → Send termination signals → Clean up PID files

use crate::process::ProcessTracker;
use crate::Result;
use std::process::Command;

// Arguments for the stop command
pub struct StopCommandArgs {
    pub app_name: Option<String>,    // Optional: specific app to stop
    pub project: Option<String>,     // Optional: stop all apps in a project
    pub all: bool,                   // If true, stop all running processes
    pub force: bool,                 // If true, use SIGKILL instead of SIGTERM
    pub silent: bool,                // If true, don't print to terminal (for TUI mode)
}

// Main implementation of the stop command
pub async fn stop_command(args: StopCommandArgs) -> Result<()> {
    let tracker = ProcessTracker::new()?;
    let silent = args.silent;
    
    // Clean up dead processes to ensure accurate status
    tracker.cleanup_dead()?;
    
    // Get all running processes
    let mut processes = tracker.list_processes()?;
    
    // Filter processes based on arguments
    let processes_to_stop = filter_processes(&args, &mut processes, &tracker)?;
    
    if processes_to_stop.is_empty() {
        if !silent {
            if args.all {
                println!("No processes are currently running");
            } else if let Some(app_name) = &args.app_name {
                println!("Process '{}' is not currently running", app_name);
            } else if let Some(project) = &args.project {
                println!("No processes are currently running for project '{}'", project);
            } else {
                println!("No processes specified to stop");
            }
        }
        return Ok(());
    }
    
    // Display what we're about to stop
    if !silent {
        display_stop_summary(&processes_to_stop)?;
    }
    
    // Stop each process
    let mut stopped_count = 0;
    let mut errors = Vec::new();
    
    for process in &processes_to_stop {
        match stop_single_process(process, args.force, &tracker, silent).await {
            Ok(_) => {
                if !silent {
                    println!("✓ Stopped: {} (PID: {})", process.app_name, process.pid);
                }
                stopped_count += 1;
            }
            Err(e) => {
                let error_msg = format!("Failed to stop {}: {}", process.app_name, e);
                errors.push(error_msg.clone());
                if !silent {
                    println!("✗ {}", error_msg);
                }
            }
        }
    }
    
    // Summary
    if !silent {
        println!("\nStop Summary:");
        println!("  ✓ Successfully stopped: {}", stopped_count);
        
        if !errors.is_empty() {
            println!("  ✗ Failed to stop: {}", errors.len());
        }
        
        if stopped_count > 0 {
            println!("✓ All specified processes have been stopped");
        }
    }
    
    if !errors.is_empty() {
        anyhow::bail!(
            "Some processes failed to stop:\n  {}",
            errors.join("\n  ")
        );
    }
    
    Ok(())
}

// Filter processes based on command arguments
fn filter_processes(args: &StopCommandArgs, processes: &mut Vec<crate::process::ProcessInfo>, tracker: &ProcessTracker) -> Result<Vec<crate::process::ProcessInfo>> {
    let mut filtered = Vec::new();
    
    // Clean up dead processes first
    processes.retain(|p| tracker.is_running(p.pid));
    
    if args.all {
        // Stop all running processes
        filtered.extend(processes.clone());
    } else if let Some(ref app_name) = args.app_name {
        // Stop specific app
        let mut found = false;
        for process in processes.iter() {
            if &process.app_name == app_name || process.app_config_name.as_ref() == Some(app_name) {
                if let Some(ref project_filter) = args.project {
                    if process.project.as_ref() == Some(project_filter) {
                        filtered.push(process.clone());
                        found = true;
                        break;
                    }
                } else {
                    filtered.push(process.clone());
                    found = true;
                    break;
                }
            }
        }
        if !found {
            // No process found with that name
        }
    } else if let Some(ref project) = args.project {
        // Stop all apps in a project
        for process in processes.iter() {
            if process.project.as_ref() == Some(project) {
                filtered.push(process.clone());
            }
        }
    }
    
    Ok(filtered)
}

// Display summary of processes that will be stopped
fn display_stop_summary(processes: &[crate::process::ProcessInfo]) -> Result<()> {
    if processes.is_empty() {
        return Ok(());
    }
    
    println!("Processes to stop:");
    
    // Group by project for better display
    let mut by_project: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    
    for process in processes {
        let project = process.project.as_deref().unwrap_or("ungrouped");
        by_project
            .entry(project.to_string())
            .or_insert_with(Vec::new)
            .push(process);
    }
    
    for (project, procs) in by_project {
        println!("\n  Project: {}", project);
        for process in procs {
            println!("    [{}] PID: {}  {}", 
                process.app_config_name.as_ref().unwrap_or(&process.app_name),
                process.pid,
                process.command
            );
        }
    }
    
    println!();
    Ok(())
}

// Stop a single process with proper signal handling
async fn stop_single_process(process: &crate::process::ProcessInfo, force: bool, tracker: &ProcessTracker, silent: bool) -> Result<()> {
    #[cfg(unix)]
    {
        // First, try graceful shutdown with SIGTERM
        if !force {
            if !silent {
                println!("Sending SIGTERM to process '{}' (PID: {})...", process.app_name, process.pid);
            }
            
            if let Err(e) = Command::new("kill")
                .args(["-TERM", &process.pid.to_string()])
                .output()
            {
                return Err(anyhow::anyhow!("Failed to send SIGTERM: {}", e));
            }
            
            // Wait for graceful shutdown
            for _i in 0..10 {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                if !tracker.is_running(process.pid) {
                    if !silent {
                        println!("Process '{}' terminated gracefully", process.app_name);
                    }
                    
                    // Remove PID file
                    tracker.remove_process(&process.app_name)?;
                    return Ok(());
                }
            }
            
            // If we get here, the process didn't terminate gracefully
            if !force && !silent {
                println!("Process '{}' did not terminate gracefully, using SIGKILL...", process.app_name);
            }
        }
        
        // Force kill with SIGKILL
        if !silent {
            println!("Sending SIGKILL to process '{}' (PID: {})...", process.app_name, process.pid);
        }
        
        if let Err(e) = Command::new("kill")
            .args(["-KILL", &process.pid.to_string()])
            .output()
        {
            return Err(anyhow::anyhow!("Failed to send SIGKILL: {}", e));
        }
        
        // Wait a bit for the process to die
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Verify process is actually dead
        if tracker.is_running(process.pid) {
            return Err(anyhow::anyhow!("Process '{}' (PID: {}) is still running after SIGKILL", 
                process.app_name, process.pid));
        }
        
        if !silent {
            println!("Process '{}' (PID: {}) killed successfully", process.app_name, process.pid);
        }
        
        // Remove PID file
        tracker.remove_process(&process.app_name)?;
    }
    
    #[cfg(not(unix))]
    {
        // For non-Unix platforms, we don't have process management yet
        return Err(anyhow::anyhow!("Process termination not implemented for this platform"));
    }
    
    Ok(())
}