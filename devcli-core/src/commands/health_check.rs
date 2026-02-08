// Health check command - manually check the health of a running application
// Executes the configured health check and displays the result

use crate::config::{load_config, resolve_app};
use crate::process::{HealthCheckEngine, ProcessTracker};
use crate::Result;
use anyhow::Context;

/// Arguments for the health-check command
#[derive(Debug)]
pub struct HealthCheckArgs {
    /// Name of the app to check (can be in format "project/app" or just "app")
    pub app_name: String,
    /// Optional environment to check (local, docker, orbstack, k8s)
    pub environment: Option<String>,
}

/// Execute a manual health check on a running application
#[tracing::instrument(skip(args), fields(app_name = %args.app_name, environment = ?args.environment))]
pub async fn health_check_command(args: HealthCheckArgs) -> Result<()> {
    // Load configuration
    let config = load_config()?;

    // Resolve the app name to find the correct project/app
    let resolved = resolve_app(&config, &args.app_name, args.environment.as_deref())
        .context("Failed to resolve app")?;

    tracing::info!(
        project = %resolved.project,
        app = %resolved.app_name,
        "Checking health of application"
    );
    println!("Checking health of: {}/{}", resolved.project, resolved.app_name);

    // Get app configuration
    let project_config = config.projects.get(&resolved.project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved.project))?;

    let app_config = project_config.apps.get(&resolved.app_name)
        .ok_or_else(|| anyhow::anyhow!("App '{}' not found", resolved.app_name))?;

    // Check if health check is configured
    let health_check = app_config.health_check.as_ref()
        .ok_or_else(|| anyhow::anyhow!(
            "No health check configured for {}/{}",
            resolved.project,
            resolved.app_name
        ))?;

    // Check if process is running
    let tracker = ProcessTracker::new()?;
    let process_info = tracker.get_process(
        &resolved.project,
        &resolved.app_name,
        None, // Will find any running instance regardless of environment
    )?;

    if process_info.is_none() {
        anyhow::bail!(
            "App {}/{} is not currently running",
            resolved.project,
            resolved.app_name
        );
    }

    let process_info = process_info.unwrap();

    tracing::debug!(
        pid = process_info.pid,
        health_check = ?health_check,
        "Starting health check execution"
    );

    println!("Process ID: {}", process_info.pid);
    println!("Health check type: {:?}", health_check);
    println!();
    println!("Executing health check...");

    // Execute the health check
    let engine = HealthCheckEngine::new();
    let start = std::time::Instant::now();

    match engine.check(health_check).await {
        Ok(true) => {
            let duration = start.elapsed();
            tracing::info!(
                project = %resolved.project,
                app = %resolved.app_name,
                duration_secs = %duration.as_secs_f64(),
                result = "passed",
                "Health check passed"
            );

            println!("✅ Health check PASSED ({:.2}s)", duration.as_secs_f64());
            println!();
            println!("Status: Healthy");

            // Show current health statistics from process info
            if process_info.health_check_failures > 0 {
                println!(
                    "Note: Process has {} recent failure(s)",
                    process_info.health_check_failures
                );
            }

            Ok(())
        }
        Ok(false) => {
            let duration = start.elapsed();
            tracing::warn!(
                project = %resolved.project,
                app = %resolved.app_name,
                duration_secs = %duration.as_secs_f64(),
                failures = process_info.health_check_failures,
                result = "failed",
                "Health check failed"
            );

            println!("❌ Health check FAILED ({:.2}s)", duration.as_secs_f64());
            println!();
            println!("Status: Unhealthy");

            // Show current health statistics from process info
            if process_info.health_check_failures > 0 {
                println!(
                    "Consecutive failures: {}",
                    process_info.health_check_failures
                );
            }

            if let Some(last_check) = process_info.last_health_check {
                println!("Last check: {}", last_check.format("%Y-%m-%d %H:%M:%S"));
            }

            anyhow::bail!("Health check failed");
        }
        Err(e) => {
            let duration = start.elapsed();
            tracing::error!(
                project = %resolved.project,
                app = %resolved.app_name,
                duration_secs = %duration.as_secs_f64(),
                error = %e,
                result = "error",
                "Health check encountered error"
            );

            println!("⚠️  Health check ERROR ({:.2}s)", duration.as_secs_f64());
            println!();
            println!("Error: {}", e);

            anyhow::bail!("Health check encountered an error: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_args_creation() {
        let args = HealthCheckArgs {
            app_name: "test-app".to_string(),
            environment: Some("local".to_string()),
        };

        assert_eq!(args.app_name, "test-app");
        assert_eq!(args.environment, Some("local".to_string()));
    }

    #[test]
    fn test_health_check_args_no_env() {
        let args = HealthCheckArgs {
            app_name: "test-app".to_string(),
            environment: None,
        };

        assert_eq!(args.app_name, "test-app");
        assert!(args.environment.is_none());
    }
}
