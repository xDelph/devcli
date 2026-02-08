//! Dependency handling and checking
//!
//! This module handles:
//! - Resolving dependency chains for all apps collectively
//! - Checking which dependencies are running
//! - Auto-starting missing dependencies in parallel

use super::resolver::StartCommandArgs;
use crate::config::{
    dependencies::{check_dependencies_running, resolve_dependency_chain},
    load_config, load_preferences,
};
use crate::process::ProcessTracker;
use crate::Result;

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
#[tracing::instrument(skip(apps), fields(app_count = apps.len(), environment = %environment))]
pub async fn handle_dependencies(
    apps: &[&crate::config::resolver::ResolvedApp],
    environment: &str,
    silent: bool,
) -> Result<()> {
    let config = load_config()?;
    let preferences = load_preferences()?;
    let tracker = ProcessTracker::new()?;

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

    // Remove duplicates by app name
    all_dependencies.sort_by(|a, b| a.app_name.cmp(&b.app_name));
    all_dependencies.dedup_by(|a, b| a.app_name == b.app_name);

    tracing::debug!(
        dependency_count = all_dependencies.len(),
        "Checking dependencies"
    );

    if !silent {
        println!("Checking dependencies for all apps...");
    }

    // Check which dependencies are NOT running
    let missing = check_dependencies_running(&tracker, &all_dependencies)?;

    if missing.is_empty() {
        tracing::info!("All dependencies are running");
        if !silent {
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
        start_missing_dependencies(missing, environment, silent).await?;
        if !silent {
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

/// Start missing dependencies in parallel
#[tracing::instrument(skip(missing), fields(dependency_count = missing.len(), environment = %environment))]
async fn start_missing_dependencies(
    missing: Vec<String>,
    environment: &str,
    silent: bool,
) -> Result<()> {
    tracing::info!(
        dependency_count = missing.len(),
        dependencies = ?missing,
        "Starting missing dependencies in parallel"
    );

    if !silent {
        println!("⚠ Missing dependencies: {}", missing.join(", "));
        println!("Starting dependencies in parallel...\n");
        // Flush stdout to ensure messages are immediately visible when captured by TUI
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }

    let preferences = load_preferences()?;
    let show_output = !silent && !preferences.detached_mode;

    // Collect all dependency start tasks to run in parallel
    let mut tasks = Vec::new();

    for dep_name in &missing {
        // Parse "project/app" format
        let parts: Vec<&str> = dep_name.split('/').collect();
        if parts.len() == 2 {
            let dep_project = parts[0].to_string();
            let dep_app = parts[1].to_string();
            let env = environment.to_string();

            if !silent {
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
                };

                super::executor::start_single_app_internal(dep_args, show_output)
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
