// Background monitor process management
// Spawns and manages a background daemon that monitors process health
// The monitor automatically exits when no processes are being tracked

use crate::process::ProcessTracker;
use crate::Result;
use std::process::Command;
use std::path::PathBuf;

// Check if the monitor process is currently running
// Returns true if a monitor process is active, false otherwise
pub fn is_monitor_running() -> bool {
    let tracker = match ProcessTracker::new() {
        Ok(t) => t,
        Err(_) => return false,
    };
    
    // Try to get the monitor's PID file
    // Monitor uses a special name: ".monitor"
    if let Ok(Some(process)) = tracker.get_process(".monitor") {
        // Check if the PID is actually running
        tracker.is_running(process.pid)
    } else {
        false
    }
}

// Spawn the background monitor process if not already running
// The monitor runs as a detached daemon using the devcli binary itself
// It calls "devcli monitor --daemon" which enters the monitoring loop
// Args:
//   - devcli_binary_path: Path to the devcli executable
// Returns: Result indicating success or failure
pub fn spawn_monitor_if_needed(devcli_binary_path: &PathBuf) -> Result<()> {
    // Check if monitor is already running
    if is_monitor_running() {
        // Monitor is already active, nothing to do
        return Ok(());
    }
    
    // Spawn the monitor process
    // We use the current devcli binary to run "monitor --daemon"
    let mut cmd = Command::new(devcli_binary_path);
    cmd.arg("monitor");
    cmd.arg("--daemon");
    
    // Configure the monitor to run completely detached
    // - stdin: null (no input)
    // - stdout: null (no output)
    // - stderr: null (no error output)
    // This ensures it runs silently in the background
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    
    // Platform-specific: Make it a session leader (like spawn_process does)
    // This detaches it from the current terminal
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                // Create a new session (detach from terminal)
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    
    // Spawn the process and don't wait for it
    let _child = cmd.spawn()?;
    
    // Note: We don't wait for the child or store its handle
    // It's fully detached and will manage itself
    
    Ok(())
}

// Get the path to the current devcli binary
// This is needed to spawn the monitor using the same binary
// Returns: Path to the devcli executable
pub fn get_devcli_binary_path() -> Result<PathBuf> {
    // std::env::current_exe() returns the path to the current executable
    // This works whether devcli is installed globally or run locally
    let exe_path = std::env::current_exe()?;
    Ok(exe_path)
}

