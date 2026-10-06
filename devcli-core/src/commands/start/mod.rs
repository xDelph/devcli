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
//! alive to show logs from all apps.

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
pub use logging::setup_log_monitoring;
pub use resolver::{resolve_apps_to_start, validate_and_get_command, AppToStart, StartCommandArgs};

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

    // Step 1: Resolve all apps to start and validate them
    let (apps_to_start, environment) = resolve_apps_to_start(args.clone()).await?;

    // Step 2: Handle dependencies for all apps collectively
    if !args.skip_deps {
        let apps_refs: Vec<&crate::config::resolver::ResolvedApp> =
            apps_to_start.iter().map(|a| &a.resolved_app).collect();
        handle_dependencies(&apps_refs, &environment, silent).await?;
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
        )? {
            if store.is_running(&existing) {
                tracing::info!(
                    app = %app_name,
                    pid = existing.pid,
                    "App already running (started as dependency), skipping"
                );
                if !silent {
                    println!(
                        "⚠ Process '{}' is already running with PID {}, skipping",
                        app_name, existing.pid
                    );
                }
                continue;
            }
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
    )
    .await?;

    // Step 4: Handle different modes and keep process alive for log viewing
    //
    // `--json` implies detached mode: an agent wants a result, not a live log
    // stream it cannot consume.
    let detached = args.detached || crate::output::json_enabled();
    if detached || silent {
        if !silent {
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
    } else {
        setup_log_monitoring(&started_apps, silent).await?;
    }

    Ok(())
}
