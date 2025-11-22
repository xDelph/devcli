// Status command - Display running processes grouped by project
// Shows process information with uptime, status, and optional dependency info
//
// Example: devcli status
// Example: devcli status api-private --deps

use crate::config::{dependencies::resolve_dependency_chain, load_config, resolve_app};
use crate::process::ProcessTracker;
use crate::Result;
use chrono::Utc;
use std::collections::HashMap;

// Arguments for the status command
pub struct StatusCommandArgs {
    pub app_name: Option<String>,    // Optional: filter by specific app
    pub project: Option<String>,     // Optional: filter by project
    pub show_deps: bool,              // If true, show dependency status
}

// Main implementation of the status command
// Displays processes grouped by project with nice formatting
pub async fn status_command(args: StatusCommandArgs) -> Result<()> {
    // Step 1: Get the process tracker to access running processes
    let tracker = ProcessTracker::new()?;
    
    // Step 2: Clean up dead processes
    // Removes PID files for processes that are no longer running
    // This keeps our status display accurate
    tracker.cleanup_dead()?;
    
    // Step 3: Get all tracked processes from PID files
    // Returns Vec<ProcessInfo> with all process metadata
    let mut processes = tracker.list_processes()?;
    
    // Step 4: Filter by project if requested
    // Example: devcli status --project qm
    if let Some(ref project_filter) = args.project {
        // .retain() keeps only items that match the condition
        // Removes all processes not in the specified project
        processes.retain(|p| {
            // Get the project field from ProcessInfo (it's Option<String>)
            // .as_ref() converts &Option<String> to Option<&String>
            // .map() transforms the inner value if Some
            // .unwrap_or(false) returns false if None
            p.project
                .as_ref()
                .map(|proj| proj == project_filter)
                .unwrap_or(false)
        });
    }
    
    // Step 5: Filter by app name if requested
    // Example: devcli status api-private
    if let Some(ref app_filter) = args.app_name {
        processes.retain(|p| {
            // Match either the process name OR the app config name
            // This handles both legacy processes and config-based ones
            &p.app_name == app_filter || p.app_config_name.as_ref() == Some(app_filter)
        });
    }
    
    // Step 6: Check if we have any processes to display
    if processes.is_empty() {
        println!("No processes are currently tracked");
        return Ok(());
    }
    
    // Step 7: Handle special case - show dependencies for a specific app
    // Example: devcli status api-private --deps
    if args.show_deps && args.app_name.is_some() {
        // This is a different display mode - show the app with its deps
        return show_with_dependencies(args.app_name.unwrap(), &tracker).await;
    }
    
    // Step 8: Group processes by project
    // We'll build two collections:
    // - grouped: HashMap of project_name -> Vec<ProcessInfo>
    // - ungrouped: Vec of processes without project metadata
    let mut grouped: HashMap<String, Vec<_>> = HashMap::new();
    let mut ungrouped = Vec::new();
    
    for process in processes {
        // Check if this process has project metadata
        if let Some(ref project) = process.project {
            // It has a project - add to the grouped HashMap
            // .entry() gets or creates the Vec for this project
            // .or_insert_with() creates Vec::new() if the key doesn't exist
            grouped
                .entry(project.clone())
                .or_insert_with(Vec::new)
                .push(process);
        } else {
            // No project metadata - add to ungrouped list
            // These are likely legacy processes started before config system
            ungrouped.push(process);
        }
    }
    
    // Step 9: Display grouped processes (by project)
    // First, sort the project names alphabetically for consistent display
    let mut project_names: Vec<_> = grouped.keys().cloned().collect();
    project_names.sort();
    
    // Display each project's processes
    for project_name in project_names {
        println!("\nPROJECT: {}", project_name);
        
        // Get the processes for this project
        // .get() returns Option<&Vec<ProcessInfo>>
        if let Some(processes) = grouped.get(&project_name) {
            // Display each process in this project
            for process in processes {
                // Check if process is still running
                let is_running = tracker.is_running(process.pid);
                let status = if is_running { "running" } else { "stopped" };
                
                // Calculate uptime (time since process started)
                let uptime = if is_running {
                    // Get the duration since process started
                    // .signed_duration_since() returns a Duration (time difference)
                    let duration = Utc::now().signed_duration_since(process.start_time);
                    
                    // Extract components: days, hours, minutes
                    let days = duration.num_days();
                    let hours = duration.num_hours() % 24; // % 24 gets remainder (0-23)
                    let minutes = duration.num_minutes() % 60; // % 60 gets remainder (0-59)
                    
                    // Format based on duration
                    // Show days if > 0, otherwise show hours/minutes
                    if days > 0 {
                        format!("{}d {}h {}m", days, hours, minutes)
                    } else if hours > 0 {
                        format!("{}h {}m", hours, minutes)
                    } else {
                        format!("{}m", minutes)
                    }
                } else {
                    // Process is stopped - show dash instead of uptime
                    "-".to_string()
                };
                
                // Build the environment display string
                // Example: " (local)" or " (docker)"
                // .as_ref() = convert &Option<String> to Option<&String>
                // .map() = transform the inner value if Some
                // .unwrap_or_default() = return "" if None
                let env_display = process
                    .environment
                    .as_ref()
                    .map(|e| format!(" ({})", e))
                    .unwrap_or_default();
                
                // Get the display name (prefer config name over process name)
                // For "api-private:build", we want to show just "api-private"
                let display_name = process
                    .app_config_name
                    .as_ref()
                    .unwrap_or(&process.app_name);
                
                // Print the process info line
                // Format: [app-name] (env)  PID: 12345  running  2h 15m  npm start
                println!(
                    "  [{}{}]  PID: {}  {}  {}  {}",
                    display_name, env_display, process.pid, status, uptime, process.command
                );
            }
        }
    }
    
    // Step 10: Display ungrouped processes
    // These are processes without project metadata (legacy or manual)
    if !ungrouped.is_empty() {
        println!("\nUNGROUPED:");
        
        for process in ungrouped {
            // Same logic as above for status and uptime
            let is_running = tracker.is_running(process.pid);
            let status = if is_running { "running" } else { "stopped" };
            
            let uptime = if is_running {
                let duration = Utc::now().signed_duration_since(process.start_time);
                let days = duration.num_days();
                let hours = duration.num_hours() % 24;
                let minutes = duration.num_minutes() % 60;
                
                if days > 0 {
                    format!("{}d {}h {}m", days, hours, minutes)
                } else if hours > 0 {
                    format!("{}h {}m", hours, minutes)
                } else {
                    format!("{}m", minutes)
                }
            } else {
                "-".to_string()
            };
            
            // For ungrouped, just show the process name as-is
            println!(
                "  [{}]  PID: {}  {}  {}  {}",
                process.app_name, process.pid, status, uptime, process.command
            );
        }
    }
    
    // Print blank line for spacing
    println!();
    
    Ok(())
}

// Special display mode: Show an app with its dependency status
// Example: devcli status api-private --deps
//
// This is called when both app_name and --deps are specified
// Shows the app's dependencies and whether each is running
async fn show_with_dependencies(app_name: String, tracker: &ProcessTracker) -> Result<()> {
    // Load the config to resolve dependencies
    let config = load_config()?;
    
    // Find the app in the config
    let resolved = resolve_app(&config, &app_name, None)?;
    
    // Get the full dependency chain
    // This returns all apps that this one depends on
    let dependencies = resolve_dependency_chain(&config, &resolved)?;
    
    // Display header with app and project info
    println!("\nApp: {} (Project: {})", resolved.app_name, resolved.project);
    
    // Display dependencies
    if dependencies.is_empty() {
        println!("No dependencies");
    } else {
        println!("\nDependencies:");
        
        // Check each dependency's status
        for dep in dependencies {
            // Build the display key: "project/app"
            let dep_key = format!("{}/{}", dep.project, dep.app_name);
            
            // Check if this dependency is running
            // Try to get its process info
            let is_running = if let Ok(Some(process)) = tracker.get_process(&dep.app_name) {
                // Process info exists - check if PID is still active
                if tracker.is_running(process.pid) {
                    // Running! Show checkmark and PID
                    format!("✓ running (PID: {})", process.pid)
                } else {
                    // Process died
                    "✗ stopped".to_string()
                }
            } else {
                // No process info found - never started
                "✗ not started".to_string()
            };
            
            // Display the dependency status
            // Format: - project/app ✓ running (PID: 12345)
            println!("  - {} {}", dep_key, is_running);
        }
    }
    
    // Display the main app's status
    if let Ok(Some(process)) = tracker.get_process(&app_name) {
        let is_running = tracker.is_running(process.pid);
        let status = if is_running { "running" } else { "stopped" };
        
        println!("\nStatus: {}", status);
        
        if is_running {
            // Show additional info if running
            println!("PID: {}", process.pid);
            
            // Calculate uptime
            let duration = Utc::now().signed_duration_since(process.start_time);
            let days = duration.num_days();
            let hours = duration.num_hours() % 24;
            let minutes = duration.num_minutes() % 60;
            
            let uptime = if days > 0 {
                format!("{}d {}h {}m", days, hours, minutes)
            } else if hours > 0 {
                format!("{}h {}m", hours, minutes)
            } else {
                format!("{}m", minutes)
            };
            
            println!("Uptime: {}", uptime);
        }
    } else {
        // App is not running
        println!("\nStatus: not started");
    }
    
    println!();
    
    Ok(())
}
