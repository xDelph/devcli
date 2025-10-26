// Import statements bring external functionality into this file
use anyhow::{Context, Result}; // Error handling utilities
use std::collections::HashMap; // Hash map for key-value pairs (env vars)
use std::path::PathBuf; // Cross-platform file path handling
use std::process::{Command, Stdio}; // For spawning processes
use tokio::io::{AsyncBufReadExt, BufReader}; // Async I/O for reading process output
use tokio::process::Child; // Represents a running child process

// Configuration options for spawning a process
// #[derive(...)] automatically implements common traits:
// - Debug: allows printing with {:?}
// - Clone: allows making copies of this struct
#[derive(Debug, Clone)]
pub struct ProcessOptions {
    pub app_name: String,                  // Name to identify this process
    pub working_dir: PathBuf,              // Directory where process should run
    pub command: String,                   // Command to execute (e.g., "node server.js")
    pub env_vars: HashMap<String, String>, // Environment variables (KEY=VALUE pairs)
    pub detached: bool,                    // Whether process should run in background
}

// Information about a spawned process
// Option<Child> means "might have a Child or might be None"
// We use None for detached processes since we don't track them after spawning
#[derive(Debug)]
pub struct SpawnedProcess {
    pub pid: u32,             // Process ID assigned by the operating system
    pub app_name: String,     // Name of the application
    pub child: Option<Child>, // Optional handle to the child process (for attached mode)
}

// Main function to spawn a new process
// 'pub async fn' means this is a public asynchronous function
// async functions can use 'await' to wait for operations without blocking
// Arc<Mutex<T>> is Rust's way of sharing data safely across async tasks:
//   - Arc: allows multiple owners (reference counting)
//   - Mutex: ensures only one task can access data at a time
pub async fn spawn_process(
    options: ProcessOptions,
    log_writer: std::sync::Arc<tokio::sync::Mutex<crate::logging::FileLogger>>,
) -> Result<SpawnedProcess> {
    // Split the command string into parts (e.g., "npm start" -> ["npm", "start"])
    // split_whitespace() splits on spaces and tabs
    // collect() gathers the parts into a Vec (vector/array)
    let command_parts: Vec<&str> = options.command.split_whitespace().collect();

    // Validate that we have at least one command
    // bail! is a macro that returns an error immediately
    if command_parts.is_empty() {
        anyhow::bail!("Command cannot be empty");
    }

    // The first part is the program to run (e.g., "node")
    let program = command_parts[0];
    // The rest are arguments (e.g., ["server.js"])
    // &[1..] creates a slice (reference to a portion of the array)
    let args = &command_parts[1..];

    // Handle DETACHED mode: process runs in background independently
    if options.detached {
        // Create a new Command builder for the process
        // 'let mut' means this variable can be modified
        let mut cmd = Command::new(program);
        cmd.args(args); // Add command arguments

        // Configure the process:
        cmd.current_dir(&options.working_dir) // Set working directory
            .envs(&options.env_vars) // Add environment variables
            .stdin(Stdio::null()) // Don't accept input
            .stdout(Stdio::null()) // Ignore standard output
            .stderr(Stdio::null()); // Ignore standard error

        // Platform-specific code for Unix systems (macOS, Linux)
        // #[cfg(unix)] means "only compile this on Unix"
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // 'unsafe' is required for calling C functions (like setsid)
            // Rust can't guarantee safety of C code, so we must mark it
            unsafe {
                // pre_exec runs code in the child process before exec
                // setsid() creates a new session, detaching from terminal
                cmd.pre_exec(|| {
                    // If setsid() returns -1, it failed
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        // Actually spawn the process
        // The ? operator means "if this fails, return the error immediately"
        // .context() adds a helpful message to the error
        let child = cmd.spawn().context("Failed to spawn detached process")?;

        // Get the process ID (PID)
        let pid = child.id();

        // Return the spawned process info
        // We use None for child because we're not tracking detached processes
        Ok(SpawnedProcess {
            pid,
            app_name: options.app_name,
            child: None,
        })
    // Handle ATTACHED mode: we monitor the process and stream its output
    } else {
        // Use tokio's async Command for non-blocking I/O
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args);

        // Configure the process:
        cmd.current_dir(&options.working_dir)
            .envs(&options.env_vars)
            .stdout(Stdio::piped()) // Capture stdout so we can read it
            .stderr(Stdio::piped()); // Capture stderr so we can read it

        // Spawn the async process
        let mut child = cmd.spawn().context("Failed to spawn process")?;

        // Get the process ID
        // ok_or_else converts Option to Result:
        //   - Some(pid) becomes Ok(pid)
        //   - None becomes Err(...)
        let pid = child
            .id()
            .ok_or_else(|| anyhow::anyhow!("Failed to get process ID"))?;

        // Take ownership of stdout and stderr from the child process
        // .take() returns Option and leaves None in its place
        // This transfers ownership to us so we can read from these streams
        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        // Clone variables we need to move into the async task
        // Rust's ownership rules require this - we're moving data into a new task
        let app_name = options.app_name.clone();
        let log_writer_clone = log_writer.clone();

        // Spawn a background task to read and display stdout
        // tokio::spawn creates a new concurrent task
        // 'async move' captures variables and runs asynchronously
        tokio::spawn(async move {
            // BufReader buffers input for efficient line-by-line reading
            // .lines() returns an async iterator over lines
            let mut stdout_reader = BufReader::new(stdout).lines();

            // Loop while there are lines to read
            // 'while let' continues while the pattern matches
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                // Format the line with a label so user knows which app it's from
                let labeled_line = format!("[{}][stdout] {}", app_name, line);
                println!("{}", labeled_line);

                // Write to log file
                // .lock().await gets exclusive access to the log writer
                let mut writer = log_writer_clone.lock().await;
                // _ = ignores the result (we don't care if logging fails)
                let _ = writer.write_log(&line).await;
            }
        });

        // Clone again for the stderr task
        let app_name_clone = options.app_name.clone();
        let log_writer_clone = log_writer.clone();

        // Spawn another background task to read and display stderr
        // Same pattern as stdout above, but for error output
        tokio::spawn(async move {
            let mut stderr_reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                let labeled_line = format!("[{}][stderr] {}", app_name_clone, line);
                // eprintln! prints to stderr instead of stdout
                eprintln!("{}", labeled_line);

                let mut writer = log_writer_clone.lock().await;
                let _ = writer.write_log(&line).await;
            }
        });

        // Return the spawned process with the child handle
        // Some(child) means we keep a handle to monitor the process
        Ok(SpawnedProcess {
            pid,
            app_name: options.app_name,
            child: Some(child),
        })
    }
}
