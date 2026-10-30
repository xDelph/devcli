//! Dependency handling and checking
//!
//! This module handles:
//! - Resolving dependency chains for all apps collectively
//! - Checking which dependencies are running
//! - Auto-starting missing dependencies, in parallel waves that respect the
//!   order of dependency chains

use super::resolver::StartCommandArgs;
use crate::config::{
    dependencies::{check_dependencies_running, dedup_dependencies, resolve_dependency_chain},
    load_config, load_preferences, Config,
};
use crate::process_manager_support::state_store;
use crate::Result;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// How long to wait for a started dependency's health check before moving on.
const DEPENDENCY_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Handle dependencies for all apps collectively
///
/// This function is generic and reusable by multiple commands (start, run, etc.).
/// It accepts a slice of `ResolvedApp` references, which allows it to process
/// any collection of resolved applications, regardless of how they were resolved.
///
/// # Arguments
/// * `apps` - Slice of resolved apps to check dependencies for
/// * `environment` - The target environment (e.g., "local", "docker")
/// * `silent` - If true, suppresses output messages
/// * `owner` - PID of the attached `devcli start` that owns the dependencies it
///   starts (`None` when they must outlive the command)
#[tracing::instrument(skip(apps), fields(app_count = apps.len(), environment = %environment))]
pub async fn handle_dependencies(
    apps: &[&crate::config::resolver::ResolvedApp],
    environment: &str,
    silent: bool,
    owner: Option<u32>,
) -> Result<()> {
    ensure_dependencies(apps, environment, silent, owner).await?;
    share_dependencies_owned_by_others(apps, owner)
}

/// A dependency owned by another attached session is now needed by these
/// apps too: mark it shared, so that session (or pm-daemon, if it dies)
/// releases it instead of stopping it under them.
fn share_dependencies_owned_by_others(
    apps: &[&crate::config::resolver::ResolvedApp],
    owner: Option<u32>,
) -> Result<()> {
    let config = load_config()?;
    let store = state_store()?;
    for app in apps {
        for dep in resolve_dependency_chain(&config, app)? {
            let Some(process) = crate::process_manager_support::find_process(
                &store,
                &dep.project,
                &dep.app_name,
                None,
            )?
            else {
                continue;
            };
            if process.owner_pid().is_some() && process.owner_pid() != owner {
                store.mark_shared(&process.id)?;
            }
        }
    }
    Ok(())
}

async fn ensure_dependencies(
    apps: &[&crate::config::resolver::ResolvedApp],
    environment: &str,
    silent: bool,
    owner: Option<u32>,
) -> Result<()> {
    let config = load_config()?;
    let preferences = load_preferences()?;
    let store = state_store()?;

    // Collect dependencies for all apps
    let mut all_dependencies = Vec::new();

    for app in apps {
        // Resolve the full dependency chain for this app
        let dependencies = resolve_dependency_chain(&config, app)?;
        all_dependencies.extend(dependencies);
    }

    if all_dependencies.is_empty() {
        tracing::debug!("No dependencies to check");
        return Ok(());
    }

    dedup_dependencies(&mut all_dependencies);

    tracing::debug!(
        dependency_count = all_dependencies.len(),
        "Checking dependencies"
    );

    if crate::output::human(silent) {
        println!("Checking dependencies for all apps...");
    }

    // Check which dependencies are NOT running
    let missing = check_dependencies_running(&store, &all_dependencies)?;

    if missing.is_empty() {
        tracing::info!("All dependencies are running");
        if crate::output::human(silent) {
            println!("✓ All dependencies are running");
        }
        return Ok(());
    }

    tracing::warn!(
        missing_count = missing.len(),
        missing = ?missing,
        auto_start = preferences.auto_start_deps,
        "Missing dependencies detected"
    );

    // Handle missing dependencies based on preferences
    if preferences.auto_start_deps {
        let waves = start_waves(&config, &missing)?;
        for (index, wave) in waves.iter().enumerate() {
            start_missing_dependencies(wave.clone(), environment, silent, owner).await?;
            // Later waves depend on this one: give it the chance to come up.
            if index + 1 < waves.len() {
                wait_until_ready(&config, wave, silent).await;
            }
        }
        if crate::output::human(silent) {
            println!("\n✓ All dependencies started successfully");
            // Flush stdout to ensure message is captured by TUI
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    } else {
        anyhow::bail!(
            "Missing dependencies: {}. Start them first or use --skip-deps",
            missing.join(", ")
        );
    }

    Ok(())
}

/// Split `missing` ("project/app" keys) into waves that can start in parallel.
///
/// Every app comes after the missing apps it (transitively) depends on. Starting
/// a whole chain at once made, for Worker → API → Redis, API's own dependency
/// check run before Redis existed and fail with "Missing dependencies: redis".
pub(super) fn start_waves(config: &Config, missing: &[String]) -> Result<Vec<Vec<String>>> {
    let missing_set: HashSet<&str> = missing.iter().map(String::as_str).collect();

    let mut deps_of: HashMap<&str, Vec<String>> = HashMap::new();
    for key in missing {
        let Some((project, app)) = key.split_once('/') else {
            continue;
        };
        let resolved =
            crate::worktrees::resolve(config, app, Some(project), crate::worktrees::Mode::Exact)?;
        let chain = resolve_dependency_chain(config, &resolved)?;
        let blocking = chain
            .iter()
            .map(|dep| format!("{}/{}", dep.project, dep.app_name))
            .filter(|dep| missing_set.contains(dep.as_str()) && dep != key)
            .collect();
        deps_of.insert(key.as_str(), blocking);
    }

    let mut done: HashSet<String> = HashSet::new();
    let mut remaining: Vec<String> = missing.to_vec();
    let mut waves = Vec::new();
    while !remaining.is_empty() {
        let (ready, blocked): (Vec<String>, Vec<String>) = remaining.into_iter().partition(|key| {
            deps_of
                .get(key.as_str())
                .is_none_or(|deps| deps.iter().all(|d| done.contains(d)))
        });
        if ready.is_empty() {
            anyhow::bail!(
                "Circular dependency between: {}. Check app dependencies in config.",
                blocked.join(", ")
            );
        }
        done.extend(ready.iter().cloned());
        waves.push(ready);
        remaining = blocked;
    }
    Ok(waves)
}

/// Wait (bounded) for the health checks of a freshly started wave to pass.
///
/// Apps without a health check are only known to be spawned; nothing to wait
/// for. A dependency that is still not healthy after the timeout is reported
/// but does not abort the start: the dependent app may well retry on its own.
async fn wait_until_ready(config: &Config, wave: &[String], silent: bool) {
    let engine = process_manager::HealthCheckEngine::new();
    for key in wave {
        let Some((project, app)) = key.split_once('/') else {
            continue;
        };
        let Ok(resolved) =
            crate::worktrees::resolve(config, app, Some(project), crate::worktrees::Mode::Exact)
        else {
            continue;
        };
        // The check the dependency was just started with: in a git worktree
        // it targets the instance's shifted port, not the configured one.
        let started = crate::process_manager_support::state_store()
            .ok()
            .and_then(|store| {
                crate::process_manager_support::find_process(
                    &store,
                    &resolved.project,
                    &resolved.app_name,
                    None,
                )
                .ok()
                .flatten()
            });
        let check = match started {
            Some(process) => process.task.health_check,
            None => crate::process_manager_support::to_pm_health_check(
                resolved.app.health_check.as_ref(),
            ),
        };
        if matches!(check, process_manager::HealthCheck::Process {}) {
            continue;
        }

        let deadline = tokio::time::Instant::now() + DEPENDENCY_READY_TIMEOUT;
        let mut healthy = false;
        while tokio::time::Instant::now() < deadline {
            if engine.check(&check).await.unwrap_or(false) {
                healthy = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        if !healthy {
            tracing::warn!(dependency = %key, "Dependency not healthy before timeout");
            if crate::output::human(silent) {
                println!(
                    "⚠ {} is not healthy after {}s, continuing",
                    key,
                    DEPENDENCY_READY_TIMEOUT.as_secs()
                );
            }
        }
    }
}

/// Start missing dependencies in parallel
#[tracing::instrument(skip(missing), fields(dependency_count = missing.len(), environment = %environment))]
async fn start_missing_dependencies(
    missing: Vec<String>,
    environment: &str,
    silent: bool,
    owner: Option<u32>,
) -> Result<()> {
    tracing::info!(
        dependency_count = missing.len(),
        dependencies = ?missing,
        "Starting missing dependencies in parallel"
    );

    if crate::output::human(silent) {
        println!("⚠ Missing dependencies: {}", missing.join(", "));
        println!("Starting dependencies in parallel...\n");
        // Flush stdout to ensure messages are immediately visible when captured by TUI
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }

    let preferences = load_preferences()?;
    let show_output = crate::output::human(silent) && !preferences.detached_mode;

    // Collect all dependency start tasks to run in parallel
    let mut tasks = Vec::new();

    for dep_name in &missing {
        // Parse "project/app" format
        let parts: Vec<&str> = dep_name.split('/').collect();
        if parts.len() == 2 {
            let dep_project = parts[0].to_string();
            let dep_app = parts[1].to_string();
            let env = environment.to_string();

            if crate::output::human(silent) {
                println!("→ Starting: {}/{}", dep_project, dep_app);
                // Flush stdout to ensure message is captured immediately by TUI
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }

            // Spawn async task for this dependency
            let task = tokio::spawn(async move {
                let dep_args = StartCommandArgs {
                    app_names: vec![dep_app.clone()],
                    project: Some(dep_project.clone()),
                    env: Some(env),
                    skip_deps: false,
                    silent,
                    stage: None, // Dependencies use their own configured stage, not parent's override
                    detached: false,
                    owner_override: None,
                    output_tx: None,
                };

                super::executor::start_single_app_internal(dep_args, show_output, owner)
                    .await
                    .map_err(|e| format!("{}/{}: {}", dep_project, dep_app, e))
            });

            tasks.push(task);
        }
    }

    // Wait for all dependency tasks to complete
    let results = futures::future::join_all(tasks).await;

    // Collect any errors that occurred
    let mut errors = Vec::new();
    for result in results {
        match result {
            Ok(Ok(_)) => {}                                       // Success - do nothing
            Ok(Err(e)) => errors.push(e),                         // App failed to start
            Err(e) => errors.push(format!("Task failed: {}", e)), // Task itself failed
        }
    }

    // If any dependencies failed to start, abort with detailed error
    if !errors.is_empty() {
        tracing::error!(
            error_count = errors.len(),
            errors = ?errors,
            "Failed to start dependencies"
        );
        anyhow::bail!(
            "Failed to start dependencies. Cannot proceed with main application.\n\nDependency failures:\n  {}",
            errors.join("\n  ")
        );
    }

    tracing::info!(
        dependency_count = missing.len(),
        "All dependencies started successfully"
    );

    Ok(())
}
