//! Start command module - Launches apps using configuration
//!
//! This module is organized by responsibility:
//! - `resolver`: App resolution and validation logic
//! - `dependencies`: Dependency handling and checking  
//! - `executor`: Process spawning and management
//! - `logging`: Log aggregation and display
//!
//! The main command can start multiple apps at once and handles dependencies
//! for all apps collectively. In non-detached mode, it keeps the main process
//! alive to show logs from all apps, and stops them when it ends.

mod dependencies;
mod executor;
mod logging;
mod resolver;

// Test modules (in separate files as per task requirements)
#[cfg(test)]
mod dependencies_test;
#[cfg(test)]
mod executor_test;
#[cfg(test)]
mod logging_test;
#[cfg(test)]
mod resolver_test;

// Re-export public API to maintain compatibility
pub use dependencies::handle_dependencies;
pub use executor::{start_apps_in_parallel, start_single_app_internal};
pub use logging::run_attached;
pub use resolver::{resolve_apps_to_start, validate_and_get_command, AppToStart, StartCommandArgs};

use crate::config::load_preferences;
use crate::process_manager_support::{find_process, state_store};
use crate::Result;

/// Main implementation of the start command
///
/// This is async because we do I/O operations (files, processes).
/// Handles multiple apps and keeps process alive for log viewing in non-detached mode.
///
/// Flow: Load config → Resolve all apps → Check deps → Start apps in parallel → Show logs (if not detached)
#[tracing::instrument(skip(args), fields(app_names = ?args.app_names, env = ?args.env, skip_deps = args.skip_deps, stage = ?args.stage))]
pub async fn start_command(args: StartCommandArgs) -> Result<()> {
    // Validate input: we need at least one app name
    if args.app_names.is_empty() {
        anyhow::bail!("At least one app name must be provided");
    }

    tracing::info!(
        app_count = args.app_names.len(),
        "Starting command execution"
    );

    let silent = args.silent;

    // `APP@*` starts the app in every git worktree, including the ones the
    // user and other agents work in: not something an agent may do.
    let every_worktree = args.app_names.iter().any(|a| a.ends_with("@*"))
        || args.project.as_deref().is_some_and(|p| p.ends_with("@*"));
    if every_worktree
        && !silent
        && let Some(agent) = crate::utils::agent::detect()
    {
        anyhow::bail!(
            "Starting an app in every git worktree (`@*`) is not allowed when devcli runs \
             under an AI agent ({agent}). Start it in your worktree: run `devcli --json start \
             APP` from inside it, or name the worktree (`APP@slug`)."
        );
    }

    // Attached (the default): the apps live as long as this command, like
    // `docker compose up`. Detached: they keep running after it returns.
    // `--json` implies detached mode: an agent wants a result, not a live log
    // stream it cannot consume. The TUI (silent) manages apps itself.
    let detached = args.detached || crate::output::json_enabled();
    let attached = !detached && !silent && !load_preferences()?.detached_mode;
    // Listen for Ctrl+C before anything starts, so an interrupt during start-up
    // stops what was already launched instead of leaving it behind.
    let mut signals = attached.then(logging::Signals::new).transpose()?;
    let owner = if attached {
        Some(std::process::id())
    } else {
        args.owner_override
    };

    let started = start_apps(&args, owner).await;
    let (started_apps, environment) = match (started, signals.as_mut()) {
        (Ok(started), _) => started,
        (Err(e), Some(signals)) => {
            // Do not leave behind the dependencies started before the failure.
            logging::stop_after_failed_start(signals).await;
            return Err(e);
        }
        (Err(e), None) => return Err(e),
    };

    // Step 4: Stream logs and stop the apps on exit, or return right away
    if signals.is_some() && started_apps.is_empty() {
        // The apps asked for were already running: only their missing
        // dependencies were started, and those serve apps this command does
        // not own. Attaching would end at once and stop them.
        logging::release_owned();
        println!("\nNothing new to attach to; the dependencies started keep running.");
    } else if let Some(signals) = signals.as_mut() {
        run_attached(&started_apps, signals).await?;
    } else if !silent {
        if crate::output::json_enabled() {
            crate::output::print_json(&serde_json::json!({
                "started": started_apps,
                "environment": environment,
                "detached": true,
            }))?;
        } else {
            println!("\nRunning in background (detached mode, no terminal output)");
        }
    }

    Ok(())
}

/// Steps 1-3 of `start_command`: resolve the apps, start their missing
/// dependencies, then the apps themselves. Returns the names of the apps
/// started and the environment used.
async fn start_apps(args: &StartCommandArgs, owner: Option<u32>) -> Result<(Vec<String>, String)> {
    let silent = args.silent;

    // Step 1: Resolve all apps to start and validate them
    let (apps_to_start, environment) = resolve_apps_to_start(args.clone()).await?;

    // Step 2: Handle dependencies for all apps collectively
    if !args.skip_deps {
        let apps_refs: Vec<&crate::config::resolver::ResolvedApp> =
            apps_to_start.iter().map(|a| &a.resolved_app).collect();
        handle_dependencies(&apps_refs, &environment, silent, owner).await?;
    }

    // Step 2.5: Filter out apps that are now running (started as dependencies)
    let store = state_store()?;
    let mut apps_to_start_filtered = Vec::new();
    for app in apps_to_start {
        let app_name = &app.resolved_app.app_name;
        if let Some(existing) = find_process(
            &store,
            &app.resolved_app.project,
            app_name,
            Some(&environment),
        )? && store.is_running(&existing)
        {
            tracing::info!(
                app = %app_name,
                pid = existing.pid,
                "App already running (started as dependency), skipping"
            );
            if crate::output::human(silent) {
                println!(
                    "⚠ Process '{}' is already running with PID {}, skipping",
                    app_name, existing.pid
                );
            }
            continue;
        }
        apps_to_start_filtered.push(app);
    }

    // Step 3: Start all remaining apps in parallel
    let started_apps = start_apps_in_parallel(
        apps_to_start_filtered,
        &environment,
        silent,
        args.stage.clone(),
        args.output_tx.clone(),
        owner,
    )
    .await?;

    Ok((started_apps, environment))
}
