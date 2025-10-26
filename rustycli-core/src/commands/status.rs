// Import required functionality
use crate::process::ProcessTracker; // For accessing process tracking
use crate::Result; // Our custom Result type
use chrono::Utc; // For calculating uptime

// Implementation of the status command
// Shows information about all tracked processes
// async because we might do I/O operations
pub async fn status_command(app_name: Option<String>) -> Result<()> {
    // Create a tracker to access process information
    let tracker = ProcessTracker::new()?;

    // Clean up any dead processes first
    // This removes PID files for processes that are no longer running
    tracker.cleanup_dead()?;

    // Get all tracked processes
    let processes = tracker.list_processes()?;

    // Filter processes if a specific app name was provided
    // Vec<_> means "infer the vector type" (Rust figures it out)
    let filtered_processes: Vec<_> = if let Some(name) = app_name {
        // If app_name was provided, filter the list
        processes
            .into_iter() // Convert Vec to an iterator
            .filter(|p| p.app_name == name) // Keep only matching names
            .collect() // Convert iterator back to Vec
    } else {
        // If no app_name, use all processes
        processes
    };

    // If no processes match, print a message and exit early
    if filtered_processes.is_empty() {
        println!("No processes are currently tracked");
        return Ok(()); // Early return
    }

    // Print the table header
    // {:<20} means "left-aligned, 20 characters wide"
    println!(
        "\n{:<20} {:<10} {:<12} {:<15} COMMAND",
        "APP NAME", "PID", "STATUS", "UPTIME"
    );
    // Print a separator line (100 equal signs)
    let separator = "=".repeat(100);
    println!("{separator}");

    // Print info for each process
    for process in filtered_processes {
        // Check if the process is still running
        let is_running = tracker.is_running(process.pid);
        // Set status string based on whether it's running
        let status = if is_running { "running" } else { "stopped" };

        // Calculate uptime if the process is running
        let uptime = if is_running {
            // Get the duration since process started
            // signed_duration_since returns a Duration (time difference)
            let duration = Utc::now().signed_duration_since(process.start_time);

            // Extract days, hours, and minutes
            let days = duration.num_days();
            let hours = duration.num_hours() % 24; // % 24 gets remainder (0-23)
            let minutes = duration.num_minutes() % 60; // % 60 gets remainder (0-59)

            // Format nicely based on how long it's been running
            if days > 0 {
                format!("{}d {}h {}m", days, hours, minutes)
            } else if hours > 0 {
                format!("{}h {}m", hours, minutes)
            } else {
                format!("{}m", minutes)
            }
        } else {
            // If not running, show dash instead of uptime
            "-".to_string()
        };

        // Truncate command if it's too long (keep it readable)
        let command_display = if process.command.len() > 40 {
            // Take first 37 characters and add "..."
            // &process.command[..37] is a slice (substring)
            format!("{}...", &process.command[..37])
        } else {
            // If short enough, use the whole command
            process.command.clone()
        };

        // Print the row with all the information
        // Each field is formatted to match the header width
        println!(
            "{:<20} {:<10} {:<12} {:<15} {}",
            process.app_name, process.pid, status, uptime, command_display
        );
    }

    // Print empty line for spacing
    println!();

    Ok(())
}
