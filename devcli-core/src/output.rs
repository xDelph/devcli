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
use std::sync::{Mutex, OnceLock};

static JSON: AtomicBool = AtomicBool::new(false);
static COLOR: AtomicBool = AtomicBool::new(true);
static EXIT_CODE: AtomicI32 = AtomicI32::new(0);

/// The real stdout, kept aside in JSON mode (see [`reserve_stdout_for_json`]).
static JSON_OUT: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

/// Initialise output mode for the current process.
///
/// `color` should already account for `--no-color`, `NO_COLOR`, and whether
/// stdout is a TTY.
pub fn init(json: bool, color: bool) {
    JSON.store(json, Ordering::Relaxed);
    COLOR.store(color, Ordering::Relaxed);
    // `colored` reads this override for every subsequent `.color()` call.
    colored::control::set_override(color);
    if json {
        reserve_stdout_for_json();
    }
}

/// Guarantee that stdout carries nothing but the JSON document.
///
/// Human-oriented progress lines are printed from many places (and from
/// libraries); one missed `println!` before the JSON broke every `jq` consumer.
/// In JSON mode the real stdout is duplicated into a private descriptor used
/// only by [`print_json`] / [`print_json_line`], and fd 1 is pointed at stderr,
/// so any stray text lands on stderr instead.
#[cfg(unix)]
fn reserve_stdout_for_json() {
    use std::os::fd::FromRawFd;

    if JSON_OUT.get().is_some() {
        return;
    }
    let _ = std::io::stdout().flush();
    // SAFETY: plain descriptor duplication; `saved` is owned by the File below.
    unsafe {
        let saved = libc::dup(libc::STDOUT_FILENO);
        if saved < 0 {
            return;
        }
        // Keep the private copy out of child processes.
        libc::fcntl(saved, libc::F_SETFD, libc::FD_CLOEXEC);
        if libc::dup2(libc::STDERR_FILENO, libc::STDOUT_FILENO) < 0 {
            libc::close(saved);
            return;
        }
        let _ = JSON_OUT.set(Mutex::new(std::fs::File::from_raw_fd(saved)));
    }
}

#[cfg(not(unix))]
fn reserve_stdout_for_json() {}

/// Run `write` against the JSON output stream.
fn with_json_out(write: impl FnOnce(&mut dyn Write) -> Result<()>) -> Result<()> {
    if let Some(out) = JSON_OUT.get() {
        let mut guard = out
            .lock()
            .map_err(|_| anyhow::anyhow!("JSON output lock poisoned"))?;
        write(&mut *guard)?;
        guard.flush()?;
        Ok(())
    } else {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        write(&mut lock)
    }
}

/// True when machine-readable JSON output was requested.
pub fn json_enabled() -> bool {
    JSON.load(Ordering::Relaxed)
}

/// True when human-oriented progress text should be printed: the caller is not
/// silent (TUI) and the user did not ask for `--json`.
pub fn human(silent: bool) -> bool {
    !silent && !json_enabled()
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
    with_json_out(|out| {
        serde_json::to_writer_pretty(&mut *out, value)?;
        writeln!(out)?;
        Ok(())
    })
}

/// Print one JSON object on a single line (newline-delimited JSON, for streams).
pub fn print_json_line<T: Serialize>(value: &T) -> Result<()> {
    with_json_out(|out| {
        serde_json::to_writer(&mut *out, value)?;
        writeln!(out)?;
        Ok(())
    })
}

/// Print a JSON error object to stderr (used when `--json` is active).
pub fn print_json_error(message: &str) {
    let value = serde_json::json!({ "error": message });
    eprintln!("{value}");
}
