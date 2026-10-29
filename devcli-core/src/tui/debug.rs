// Debug logging utility for TUI
//
// Forwards to `tracing` at debug level, i.e. to devcli's regular log file and
// filtered by LOG_LEVEL. It used to create `tui-debug.log` in whatever
// directory the TUI was started from and write (and flush) to it on every
// frame, growing without bound inside the user's repositories.

/// Kept for call sites; logging is configured globally by `init_tracing`.
pub fn init_debug_log() {
    tracing::debug!(target: "devcli::tui", "TUI started");
}

/// Macro for easier debug logging (formatting only happens when enabled)
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        ::tracing::debug!(target: "devcli::tui", $($arg)*)
    };
}
