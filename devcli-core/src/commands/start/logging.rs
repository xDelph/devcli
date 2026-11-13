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
pub enum Ended {
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

/// Stop whatever was launched before an interrupt cut start-up short.
pub async fn stop_after_interrupted_start(signals: &mut Signals, ended: Ended) {
    let Ok(store) = state_store() else {
        return;
    };
    let quiet = ended == Ended::HangUp;
    if !quiet {
        println!(
            "\n\nInterrupted during start-up; stopping what was started (Ctrl+C again to force)..."
        );
    }
    let owner = std::process::id().to_string();
    match stop_owned(&store, &owner, &[], quiet, signals).await {
        Ok(stopped) if !quiet && stopped > 0 => println!("✓ Stopped {stopped} app(s)"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "Failed to stop apps after an interrupted start"),
    }
}

/// Hand back what this command owns without stopping it: when it started
/// only dependencies (the apps it was asked for were already running), those
/// serve apps it does not own.
pub fn release_owned() {
    let Ok(store) = state_store() else {
        return;
    };
    let owner = std::process::id().to_string();
    for process in store
        .find_by_metadata(OWNER_PID_KEY, &owner)
        .unwrap_or_default()
    {
        let _ = store.release(&process.id);
    }
}

/// What the running apps this command does not own depend on.
#[derive(Debug, Default)]
pub(super) struct Needed {
    /// `(project, app)` of their dependencies.
    pub(super) deps: std::collections::HashSet<(String, String)>,
    /// One of them depends on something that cannot be told (its app left
    /// the config): every dependency counts as needed.
    pub(super) unknown: bool,
}

impl Needed {
    /// Whether `process`, owned by this command, must keep running when the
    /// command ends.
    pub(super) fn keeps(&self, process: &ManagedProcess, started_apps: &[String]) -> bool {
        process.is_shared()
            || process
                .metadata
                .get("project")
                .zip(process.metadata.get("app_config_name"))
                .is_some_and(|(proj, app)| self.deps.contains(&(proj.clone(), app.clone())))
            || (self.unknown && !is_started_app(process, started_apps))
    }
}

/// What the running apps this command does not own depend on.
fn needed_by_others(store: &StateStore, owner: &str) -> Needed {
    let mut needed = Needed::default();
    let config = crate::config::load_config().ok();
    for process in store.list().unwrap_or_default() {
        let ours = process.metadata.get(OWNER_PID_KEY).map(String::as_str) == Some(owner);
        if ours || process.is_stopping() || !store.is_running(&process) {
            continue;
        }
        let (Some(project), Some(app)) = (
            process.metadata.get("project"),
            process.metadata.get("app_config_name"),
        ) else {
            continue;
        };
        let Some(config) = &config else {
            // Unreadable config: what it depends on cannot be told.
            needed.unknown = true;
            continue;
        };
        let Ok(resolved) = crate::worktrees::resolve(
            config,
            app,
            Some(&crate::worktrees::pinned(config, project)),
            crate::worktrees::Mode::Exact,
        ) else {
            // Its instance no longer resolves (worktree moved or deleted)
            // while it still runs: it still needs its dependencies.
            if !add_dependencies_of_configured_app(config, &process, app, &mut needed.deps) {
                needed.unknown = true;
            }
            continue;
        };
        for dep in crate::config::dependencies::resolve_dependency_chain(config, &resolved)
            .unwrap_or_default()
        {
            needed.deps.insert((dep.project, dep.app_name));
        }
    }
    needed
}

/// Add the dependencies of `app` as configured to `deps`, both as the
/// configured instances and as the copies in the worktree `process` runs
/// from. Returns `false` when the app is not in the config any more.
fn add_dependencies_of_configured_app(
    config: &crate::config::Config,
    process: &ManagedProcess,
    app: &str,
    deps: &mut std::collections::HashSet<(String, String)>,
) -> bool {
    let Some(base) = crate::worktrees::base_project(&process.metadata) else {
        return false;
    };
    let Ok(configured) = crate::worktrees::resolve(
        config,
        app,
        Some(&crate::worktrees::pinned(config, base)),
        crate::worktrees::Mode::Exact,
    ) else {
        return false;
    };
    let Ok(chain) = crate::config::dependencies::resolve_dependency_chain(config, &configured)
    else {
        return false;
    };
    let slug = process.metadata.get(crate::worktrees::meta::WORKTREE);
    for dep in chain {
        if let Some(slug) = slug {
            deps.insert((format!("{}@{slug}", dep.project), dep.app_name.clone()));
        }
        deps.insert((dep.project, dep.app_name));
    }
    true
}

async fn wait_until_ended(
    store: &StateStore,
    owner: &str,
    started_apps: &[String],
    signals: &mut Signals,
) -> Ended {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    // Gone on two polls in a row: `restart` stops an app and starts it
    // again, and the session must not end (and stop what the new instance
    // depends on) in between.
    let mut gone_once = false;
    loop {
        tokio::select! {
            ended = signals.recv() => return ended,
            _ = interval.tick() => {
                if any_started_app_alive(store, owner, started_apps) {
                    gone_once = false;
                } else if gone_once {
                    return Ended::AppsExited;
                } else {
                    gone_once = true;
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
        // `restart` is replacing one of its apps (stopped, not started yet).
        || store.restart_pending_for_owner(owner)
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

    // What other apps still need keeps running: stopping it would break
    // them (another session's app depending on it, or our app restarted by
    // another command, which this session no longer owns).
    let needed = needed_by_others(store, owner);
    let (kept, owned): (Vec<_>, Vec<_>) = owned
        .into_iter()
        .partition(|p| needed.keeps(p, started_apps));
    for process in &kept {
        let _ = store.release(&process.id);
        if !quiet {
            println!(
                "Left '{}' running: other running apps depend on it",
                process
                    .metadata
                    .get("app_config_name")
                    .map(String::as_str)
                    .unwrap_or(&process.id)
            );
        }
    }

    // Only what still runs counts as stopped; dead entries are just removed.
    let count = owned.iter().filter(|p| store.is_running(p)).count();
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

    pub async fn recv(&mut self) -> Ended {
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
