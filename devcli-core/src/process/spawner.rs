// Import statements bring external functionality into this file
use anyhow::{Context, Result}; // Error handling utilities
use colored::Colorize; // For colored terminal output
use std::collections::HashMap; // Hash map for key-value pairs (env vars)
use std::path::PathBuf; // Cross-platform file path handling
use std::process::Stdio; // For spawning processes
use tokio::io::{AsyncBufReadExt, BufReader}; // Async I/O for reading process output
use tokio::process::Child; // Represents a running child process

// #[derive(...)] automatically implements common traits:
// - Debug: allows printing with {:?}
// - Clone: allows making copies of this struct
// Configuration options for spawning a process
// All processes are spawned detached (with setsid) to survive parent process termination
#[derive(Debug, Clone)]
pub struct ProcessOptions {
    pub app_name: String,                  // Name to identify this process
    pub working_dir: PathBuf,              // Directory where process should run
    pub command: String,                   // Command to execute (e.g., "node server.js")
    pub env_vars: HashMap<String, String>, // Environment variables (KEY=VALUE pairs)
    pub detached: bool,                    // Always true - all processes use setsid()
    pub show_output: bool,                 // Whether to stream output to terminal (colored)
}

// Optional channel for streaming output to TUI
pub type OutputSender = tokio::sync::mpsc::UnboundedSender<String>;

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
    output_tx: Option<OutputSender>,
) -> Result<SpawnedProcess> {
    let command_parts: Vec<&str> = options.command.split_whitespace().collect();

    if command_parts.is_empty() {
        anyhow::bail!("Command cannot be empty");
    }

    let program = command_parts[0];
    let args = &command_parts[1..];

    if options.detached {
        // Create the command
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args);

        // Configure process:
        // - Set working directory
        // - Detach stdin (null)
        // - Pipe stdout/stderr so we can capture and log them
        cmd.current_dir(&options.working_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &options.env_vars {
            cmd.env(key, value);
        }

        // Unix-specific: Use setsid to detach from controlling terminal
        // This prevents the child process from being killed when the parent exits
        #[cfg(unix)]
        {
            #[allow(unused_imports)]
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    // setsid() creates a new session and sets the process group ID
                    if libc::setsid() == -1 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        // Spawn the process
        let mut child = cmd.spawn().context("Failed to spawn detached process")?;

        let pid = child
            .id()
            .ok_or_else(|| anyhow::anyhow!("Failed to get process ID"))?;

        // Capture stdout/stderr handles
        // We must take() them to transfer ownership to the background tasks
        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        let app_name = options.app_name.clone();
        let show_output = options.show_output;

        // Spawn background tasks to handle output streams
        // These tasks will run until the stream closes (process exits)
        let stdout_task = handle_output_stream(
            stdout,
            "stdout",
            app_name.clone(),
            show_output,
            log_writer.clone(),
            output_tx.clone(),
        );

        let stderr_task = handle_output_stream(
            stderr,
            "stderr",
            app_name.clone(),
            show_output,
            log_writer.clone(),
            output_tx.clone(),
        );

        let app_name_for_wait = options.app_name.clone();
        tokio::spawn(async move {
            match child.wait().await {
                Ok(status) => {
                    let _ = tokio::join!(stdout_task, stderr_task);

                    if status.success() {
                        if show_output {
                            println!(
                                "[{}] Process exited successfully",
                                app_name_for_wait.cyan().bold()
                            );
                        }
                    } else if show_output {
                        eprintln!(
                            "[{}] Process exited with status: {}",
                            app_name_for_wait.cyan().bold(),
                            status
                        );
                    }
                }
                Err(e) => {
                    if show_output {
                        eprintln!(
                            "[{}] Error waiting for process: {}",
                            app_name_for_wait.cyan().bold(),
                            e
                        );
                    }
                }
            }
        });

        Ok(SpawnedProcess {
            pid,
            app_name: options.app_name,
            child: None,
        })
    } else {
        // Create the command
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args);

        // Configure process:
        // - Set working directory
        // - Pipe stdout/stderr (even for attached, we want to capture/log them)
        // - Note: stdin is NOT piped to null, allowing interaction if needed (though we don't explicitly handle it here)
        cmd.current_dir(&options.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &options.env_vars {
            cmd.env(key, value);
        }

        // Spawn the process (attached to current session)
        let mut child = cmd.spawn().context("Failed to spawn process")?;

        let pid = child
            .id()
            .ok_or_else(|| anyhow::anyhow!("Failed to get process ID"))?;

        // Capture stdout/stderr handles
        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        let app_name = options.app_name.clone();

        // In attached mode, we always show output to the terminal
        // We reuse the same helper function to handle logging and TUI updates
        handle_output_stream(
            stdout,
            "stdout",
            app_name.clone(),
            true, // Always show output in attached mode
            log_writer.clone(),
            output_tx.clone(),
        );

        handle_output_stream(
            stderr,
            "stderr",
            app_name.clone(),
            true, // Always show output in attached mode
            log_writer.clone(),
            output_tx.clone(),
        );

        // Return the child handle so the caller can wait on it or kill it
        Ok(SpawnedProcess {
            pid,
            app_name: options.app_name,
            child: Some(child),
        })
    }
}

/// Handles reading from an output stream (stdout/stderr), logging, and displaying output
///
/// This helper function spawns a background task to:
/// 1. Read lines from the provided stream
/// 2. Print them to the terminal if `show_output` is true
/// 3. Send them to the TUI via `output_tx` if provided
/// 4. Write them to the log file via `log_writer`
///
/// # Arguments
/// * `stream` - The async stream to read from (stdout or stderr)
/// * `stream_type` - Label for the stream ("stdout" or "stderr")
/// * `app_name` - Name of the app for logging context
/// * `show_output` - Whether to print to terminal (stdout/stderr)
/// * `log_writer` - Shared logger instance
/// * `output_tx` - Optional channel to send output to TUI
fn handle_output_stream<R>(
    stream: R,
    stream_type: &'static str,
    app_name: String,
    show_output: bool,
    log_writer: std::sync::Arc<tokio::sync::Mutex<crate::logging::FileLogger>>,
    output_tx: Option<OutputSender>,
) -> tokio::task::JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        // Use a buffered reader for efficient line-by-line reading
        let mut reader = BufReader::new(stream).lines();
        let mut stream_alive = true;

        while let Ok(Some(line)) = reader.next_line().await {
            // 1. Show output in terminal if requested (and stream is still writable)
            if show_output && stream_alive {
                use std::io::Write;

                // Write to appropriate standard stream based on type
                let result = if stream_type == "stderr" {
                    writeln!(
                        std::io::stderr(),
                        "[{}][{}] {}",
                        app_name.cyan().bold(),
                        stream_type,
                        line
                    )
                } else {
                    writeln!(
                        std::io::stdout(),
                        "[{}][{}] {}",
                        app_name.cyan().bold(),
                        stream_type,
                        line
                    )
                };

                // If writing fails (e.g., pipe broken), stop trying to write to terminal
                // but continue processing logs and TUI updates
                if result.is_err() {
                    stream_alive = false;
                }
            }

            // 2. Send to TUI popup if channel is provided
            // This allows the TUI to show real-time logs in the "Executing" popup
            if let Some(ref tx) = output_tx {
                let msg = if stream_type == "stderr" {
                    format!("[stderr] {}", line)
                } else {
                    line.clone()
                };
                // Ignore send errors (receiver might have dropped if popup closed)
                let _ = tx.send(msg);
            }

            // 3. Always write to log file
            // This ensures we have a permanent record even if terminal/TUI are closed
            let mut writer = log_writer.lock().await;
            let _ = writer.write_log(&line).await;
        }
    })
}
