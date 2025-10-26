// Import functionality from our crate and standard library
use crate::logging::FileLogger; // For creating log files
use crate::process::{spawn_process, ProcessInfo, ProcessOptions, ProcessTracker};
use crate::Result; // Our custom Result type (Result<T, anyhow::Error>)
use chrono::Utc; // For timestamps
use std::collections::HashMap; // For environment variables
use std::path::PathBuf; // For file paths
use std::sync::Arc; // Arc = Atomic Reference Counted (smart pointer for sharing)
use tokio::sync::Mutex; // Async-aware mutex for thread-safe sharing

// Arguments for the start command
// This struct bundles all the parameters together
pub struct StartCommandArgs {
    pub app_name: String,                  // Name to identify the process
    pub working_dir: PathBuf,              // Directory where process runs
    pub command: String,                   // Command to execute
    pub env_vars: HashMap<String, String>, // Environment variables
    pub detached: bool,                    // Run in background?
}

// Main implementation of the start command
// This is async because we do I/O operations (files, processes)
pub async fn start_command(args: StartCommandArgs) -> Result<()> {
    // Create a process tracker to check for existing processes
    let tracker = ProcessTracker::new()?;

    // Check if a process with this name already exists
    // 'if let Some(existing)' only runs if get_process returns Some (not None)
    if let Some(existing) = tracker.get_process(&args.app_name)? {
        // Check if the existing process is still running
        if tracker.is_running(existing.pid) {
            // If running, return an error - can't start duplicate
            // bail! macro returns an error immediately
            anyhow::bail!(
                "Process '{}' is already running with PID {}",
                args.app_name,
                existing.pid
            );
        } else {
            // If not running, clean up the stale PID file
            println!(
                "Cleaning up stale process '{}' (PID {} is no longer running)",
                args.app_name, existing.pid
            );
            tracker.remove_process(&args.app_name)?;
        }
    }

    // Create a log file for this process
    // Arc<Mutex<FileLogger>> allows sharing the logger across async tasks:
    //   Arc: Multiple tasks can have a reference to it
    //   Mutex: Only one task can write to it at a time
    let log_writer = Arc::new(Mutex::new(FileLogger::new(&args.app_name).await?));

    // Get the log file path for display
    // Scope with {} ensures we release the lock quickly
    let log_path = {
        // Lock the mutex to access the logger
        let writer = log_writer.lock().await;
        // Get and clone the path (clone creates a copy)
        writer.log_path().clone()
    }; // Lock is automatically released here

    // Build the options struct for spawning the process
    // We need .clone() because Rust's ownership system doesn't allow
    // moving the same value multiple times
    let options = ProcessOptions {
        app_name: args.app_name.clone(),
        working_dir: args.working_dir.clone(),
        command: args.command.clone(),
        env_vars: args.env_vars.clone(),
        detached: args.detached,
    };

    // Display info to the user about what we're doing
    println!(
        "Starting process '{}' in {} (detached: {})",
        args.app_name,
        args.working_dir.display(), // .display() formats PathBuf nicely
        args.detached
    );
    println!("Command: {}", args.command);
    println!("Log file: {}", log_path.display());

    // Actually spawn the process!
    // This returns info about the spawned process
    let spawned = spawn_process(options, log_writer.clone()).await?;

    // Create metadata about this process for tracking
    let process_info = ProcessInfo {
        app_name: args.app_name.clone(),
        pid: spawned.pid, // Process ID from the OS
        command: args.command,
        // .to_string_lossy() converts PathBuf to String
        // "lossy" means invalid UTF-8 characters become �
        working_dir: args.working_dir.to_string_lossy().to_string(),
        start_time: Utc::now(), // Current time in UTC
        env_vars: args.env_vars,
    };

    // Save the process info to a PID file
    tracker.register_process(process_info)?;

    // Handle the two different modes differently
    if args.detached {
        // DETACHED MODE: Process runs independently
        println!(
            "✓ Process '{}' started successfully with PID {}",
            args.app_name, spawned.pid
        );
        println!("Process is running in detached mode");
        // We're done - the process continues in the background
    } else {
        // ATTACHED MODE: We monitor the process
        println!(
            "✓ Process '{}' started with PID {} (attached mode)",
            args.app_name, spawned.pid
        );
        println!("Streaming logs... (Ctrl+C to stop)\n");

        // If we have a child handle (we should in attached mode)
        // 'if let Some(mut child)' extracts the Child from Option<Child>
        // 'mut' because .wait() needs to modify the child
        if let Some(mut child) = spawned.child {
            // Wait for the child process to finish
            // This blocks until the process exits
            let exit_status = child.wait().await?;

            // Print the exit status (e.g., "exit status: 0" for success)
            println!(
                "\nProcess '{}' exited with status: {}",
                args.app_name, exit_status
            );

            // Remove the PID file since the process is done
            tracker.remove_process(&args.app_name)?;
        }
    }

    // Return Ok(()) to indicate success
    Ok(())
}
