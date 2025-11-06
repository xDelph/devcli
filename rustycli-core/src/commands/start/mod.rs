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

mod resolver;
mod dependencies;
mod executor;
mod logging;

// Test modules (in separate files as per task requirements)
#[cfg(test)]
mod resolver_test;
#[cfg(test)]
mod dependencies_test;
#[cfg(test)]
mod executor_test;
#[cfg(test)]
mod logging_test;

// Re-export public API to maintain compatibility
pub use resolver::{StartCommandArgs, resolve_apps_to_start, AppToStart, get_available_environments, validate_and_get_command};
pub use dependencies::{handle_dependencies};
pub use executor::{start_apps_in_parallel, start_single_app_internal};
pub use logging::{setup_log_monitoring};

use crate::Result;

/// Main implementation of the start command
/// 
/// This is async because we do I/O operations (files, processes).
/// Handles multiple apps and keeps process alive for log viewing in non-detached mode.
/// 
/// Flow: Load config → Resolve all apps → Check deps → Start apps in parallel → Show logs (if not detached)
pub async fn start_command(args: StartCommandArgs) -> Result<()> {
    // Validate input: we need at least one app name
    if args.app_names.is_empty() {
        anyhow::bail!("At least one app name must be provided");
    }

    // Step 1: Resolve all apps to start and validate them
    let (apps_to_start, environment) = resolve_apps_to_start(args.clone()).await?;
    
    // Step 2: Handle dependencies for all apps collectively
    if !args.skip_deps {
        handle_dependencies(&apps_to_start, &environment).await?;
    }
    
    // Step 3: Start all apps in parallel
    let started_apps = start_apps_in_parallel(apps_to_start, &environment).await?;
    
    // Step 4: Handle different modes and keep process alive for log viewing
    setup_log_monitoring(&started_apps).await?;
    
    // Step 5: Ensure background monitor is running
    let binary_path = crate::process::monitor::get_rustycli_binary_path()?;
    let _ = crate::process::monitor::spawn_monitor_if_needed(&binary_path);
    
    Ok(())
}