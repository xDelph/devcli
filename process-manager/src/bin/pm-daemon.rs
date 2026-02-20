//! pm-daemon — background monitor daemon for process-manager
//!
//! Spawned automatically by `StateStore::ensure_daemon_running()` when a client
//! starts a managed process. Never launched manually by the user.
//!
//! Lifecycle:
//!   1. Acquires an exclusive file lock on `state_dir/.daemon.lock`.
//!      If the lock is already held, another daemon is running → exit silently.
//!   2. Runs the `Monitor` loop (health checks + auto-restarts) until the
//!      state store is empty.
//!   3. Releases the lock and exits. The next `start` call will spawn a new
//!      daemon if processes are added again.
//!
//! All monitoring state lives under `state_dir/`:
//!   - `*.json`         — managed process state (read by Monitor)
//!   - `locks/`         — per-process restart coordination locks
//!   - `.daemon.lock`   — daemon singleton lock (held for daemon lifetime)
//!   - `.status_changed`— mtime-based change notification for TUI polling

use fs2::FileExt;
use process_manager::{HealthCheckEngine, Monitor, RestartCoordinator, StateStore};
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let state_dir = parse_state_dir()?;

    // Acquire exclusive lock for daemon singleton guarantee.
    // The lock is held for the entire daemon lifetime and released on exit.
    let lock_path = state_dir.join(".daemon.lock");
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;

    if lock_file.try_lock_exclusive().is_err() {
        // Another daemon is already running for this state dir — exit silently.
        return Ok(());
    }

    // Everything the daemon needs lives under state_dir.
    let state = Arc::new(StateStore::new(state_dir.clone())?);
    let locks_dir = state_dir.join("locks");
    let health_engine = Arc::new(HealthCheckEngine::new());
    let restart_coordinator = Arc::new(RestartCoordinator::new(locks_dir));

    let monitor = Monitor::new(state, health_engine, restart_coordinator);

    // Blocks until the state store is empty (all processes stopped/removed).
    monitor.run().await?;

    // lock_file drops here → OS releases the exclusive lock automatically.
    Ok(())
}

fn parse_state_dir() -> anyhow::Result<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let pos = args
        .iter()
        .position(|a| a == "--state-dir")
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: --state-dir <path>"))?;
    args.get(pos + 1)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("--state-dir requires a path argument"))
}
