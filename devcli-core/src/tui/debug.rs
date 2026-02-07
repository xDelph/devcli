// Debug logging utility for TUI
// Writes debug messages to tui-debug.log to help troubleshoot issues

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

static DEBUG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Initialize debug logging
pub fn init_debug_log() {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("tui-debug.log")
        .ok();

    if let Ok(mut debug_file) = DEBUG_FILE.lock() {
        *debug_file = file;
    }

    debug_log("=== TUI Debug Log Started ===");
}

/// Write a debug message to the log file
pub fn debug_log(msg: &str) {
    if let Ok(mut guard) = DEBUG_FILE.lock() {
        if let Some(file) = guard.as_mut() {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] {}", timestamp, msg);
            let _ = file.flush();
        }
    }
}

/// Macro for easier debug logging
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::tui::debug::debug_log(&format!($($arg)*))
    };
}
