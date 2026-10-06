//! Process-wide output configuration for CLI commands.
//!
//! Agents consume `--json` output; humans get colored text. Commands query
//! this module instead of threading flags through every function signature,
//! which keeps the change non-invasive for existing command code.
//!
//! The CLI entry point calls [`init`] once, then commands use
//! [`json_enabled`] / [`color_enabled`] and [`set_exit_code`].

use anyhow::Result;
use serde::Serialize;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

static JSON: AtomicBool = AtomicBool::new(false);
static COLOR: AtomicBool = AtomicBool::new(true);
static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

/// Initialise output mode for the current process.
///
/// `color` should already account for `--no-color`, `NO_COLOR`, and whether
/// stdout is a TTY.
pub fn init(json: bool, color: bool) {
    JSON.store(json, Ordering::Relaxed);
    COLOR.store(color, Ordering::Relaxed);
    // `colored` reads this override for every subsequent `.color()` call.
    colored::control::set_override(color);
}

/// True when machine-readable JSON output was requested.
pub fn json_enabled() -> bool {
    JSON.load(Ordering::Relaxed)
}

/// True when colored output is allowed.
pub fn color_enabled() -> bool {
    COLOR.load(Ordering::Relaxed)
}

/// Set the process exit code used after a successful command returns.
///
/// Commands that model a "result" (status, health-check) call this instead of
/// returning an error, so agents get a clean exit code without a scary message.
pub fn set_exit_code(code: i32) {
    EXIT_CODE.store(code, Ordering::Relaxed);
}

/// Current process exit code (0 unless a command set it).
pub fn exit_code() -> i32 {
    EXIT_CODE.load(Ordering::Relaxed)
}

/// Print a serializable value as pretty JSON followed by a newline.
pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer_pretty(&mut lock, value)?;
    writeln!(lock)?;
    Ok(())
}

/// Print one JSON object on a single line (newline-delimited JSON, for streams).
pub fn print_json_line<T: Serialize>(value: &T) -> Result<()> {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, value)?;
    writeln!(lock)?;
    Ok(())
}

/// Print a JSON error object to stderr (used when `--json` is active).
pub fn print_json_error(message: &str) {
    let value = serde_json::json!({ "error": message });
    eprintln!("{value}");
}
