//! Attached mode: stream the output of the apps a `start` launched, and stop
//! them when the command ends.
//!
//! Without `--detached` (or the `detached_mode` preference), `devcli start`
//! behaves like `docker compose up`: the apps it starts, dependencies
//! included, live as long as the command. They are tagged with its PID
//! ([`OWNER_PID_KEY`]) so it can find them on the way out, and so pm-daemon
//! can stop them if the command dies without doing it (killed, crashed).

use crate::commands::stop::stop_single_process;
use crate::process_manager_support::state_store;
use crate::Result;
use process_manager::state::{ManagedProcess, OWNER_PID_KEY};
use process_manager::StateStore;

/// Why an attached session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ended {
    /// Ctrl+C or SIGTERM.
    Interrupted,
    /// The terminal went away: nothing can be printed any more.
    HangUp,
    /// Every app started by the command has exited on its own.
    AppsExited,
}

/// Stream output until Ctrl+C, SIGTERM, a closed terminal or the exit of all
/// `started_apps`, then stop every process this command started.
#[tracing::instrument(skip(started_apps, signals), fields(app_count = started_apps.len()))]
pub async fn run_attached(started_apps: &[String], signals: &mut Signals) -> Result<()> {
    let store = state_store()?;
    let owner = std::process::id().to_string();

    println!("\nStreaming output. Press Ctrl+C to stop the apps started by this command");
    println!("(use --detached to leave them running in the background).\n");

    let ended = wait_until_ended(&store, &owner, started_apps, signals).await;
    tracing::info!(?ended, "Attached session ended");

    let quiet = ended == Ended::HangUp;
    if !quiet {
        match ended {
            Ended::AppsExited => println!("\n\nAll apps have exited."),
            _ => {
                println!("\n\nStopping the apps started by this command (Ctrl+C again to force)...")
            }
        }
    }

    let stopped = stop_owned(&store, &owner, started_apps, quiet, signals).await?;
    if !quiet && stopped > 0 {
        println!("✓ Stopped {stopped} app(s)");
    }
    Ok(())
}

/// Stop whatever this command started before its start-up failed.
pub async fn stop_after_failed_start(signals: &mut Signals) {
    let Ok(store) = state_store() else {
        return;
    };
    let owner = std::process::id().to_string();
    if let Err(e) = stop_owned(&store, &owner, &[], false, signals).await {
        tracing::warn!(error = %e, "Failed to stop apps after a failed start");
    }
}

async fn wait_until_ended(
    store: &StateStore,
    owner: &str,
    started_apps: &[String],
    signals: &mut Signals,
) -> Ended {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    loop {
        tokio::select! {
            ended = signals.recv() => return ended,
            _ = interval.tick() => {
                if !any_started_app_alive(store, owner, started_apps) {
                    return Ended::AppsExited;
                }
            }
        }
    }
}

/// Whether one of the apps started by this command is running, or crashed and
/// is about to be restarted by pm-daemon.
fn any_started_app_alive(store: &StateStore, owner: &str, started_apps: &[String]) -> bool {
    let Ok(owned) = store.find_by_metadata(OWNER_PID_KEY, owner) else {
        // Unreadable state is no reason to tear the apps down.
        return true;
    };
    owned
        .iter()
        .filter(|p| is_started_app(p, started_apps))
        .any(|p| store.is_running(p) || store.awaiting_restart(p))
}

fn is_started_app(process: &ManagedProcess, started_apps: &[String]) -> bool {
    process.metadata.get("app_config_name").is_some_and(|name| {
        let worktree = process.metadata.get(crate::worktrees::meta::WORKTREE);
        let instance = match worktree {
            Some(slug) => format!("{name}@{slug}"),
            None => name.clone(),
        };
        started_apps.contains(&instance)
    })
}

/// Stop what this command owns: the requested apps first, then the
/// dependencies it started for them. A second interrupt switches to SIGKILL.
/// Returns how many processes there were to stop.
async fn stop_owned(
    store: &StateStore,
    owner: &str,
    started_apps: &[String],
    quiet: bool,
    signals: &mut Signals,
) -> Result<usize> {
    let owned = store.find_by_metadata(OWNER_PID_KEY, owner)?;
    let count = owned.len();
    let (apps, dependencies): (Vec<_>, Vec<_>) = owned
        .into_iter()
        .partition(|p| is_started_app(p, started_apps));

    let graceful = async {
        stop_wave(store, &apps, false, quiet).await;
        stop_wave(store, &dependencies, false, quiet).await;
    };

    tokio::select! {
        _ = graceful => {}
        _ = signals.recv() => {
            // Whatever is left: the graceful pass deletes what it stopped.
            let remaining = store.find_by_metadata(OWNER_PID_KEY, owner)?;
            stop_wave(store, &remaining, true, quiet).await;
        }
    }

    Ok(count)
}

async fn stop_wave(store: &StateStore, processes: &[ManagedProcess], force: bool, quiet: bool) {
    let results = futures::future::join_all(
        processes
            .iter()
            .map(|p| stop_single_process(p, force, store, quiet, None)),
    )
    .await;

    for (process, result) in processes.iter().zip(results) {
        if let Err(e) = result {
            tracing::warn!(id = %process.id, error = %e, "Failed to stop owned process");
            if !quiet {
                eprintln!("⚠ Failed to stop '{}': {e}", process.id);
            }
        }
    }
}

/// The signals that end an attached session.
///
/// Registered once, up front: a fresh `ctrl_c()` future per loop iteration
/// would miss a signal delivered between two iterations.
pub struct Signals {
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
    #[cfg(unix)]
    hangup: tokio::signal::unix::Signal,
}

impl Signals {
    pub fn new() -> Result<Self> {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            Ok(Self {
                interrupt: signal(SignalKind::interrupt())?,
                terminate: signal(SignalKind::terminate())?,
                hangup: signal(SignalKind::hangup())?,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    async fn recv(&mut self) -> Ended {
        #[cfg(unix)]
        {
            tokio::select! {
                _ = self.interrupt.recv() => Ended::Interrupted,
                _ = self.terminate.recv() => Ended::Interrupted,
                _ = self.hangup.recv() => Ended::HangUp,
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            Ended::Interrupted
        }
    }
}
