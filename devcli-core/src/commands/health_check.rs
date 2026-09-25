// Health check command - manually check the health of a running application

use crate::config::{load_config, resolve_app};
use crate::process_manager_support::{find_process, state_store, to_pm_health_check};
use crate::Result;
use anyhow::Context;
use chrono::Utc;
use process_manager::HealthCheckEngine;

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
    let config = load_config()?;
    let resolved = resolve_app(&config, &args.app_name, None).context("Failed to resolve app")?;

    tracing::info!(
        project = %resolved.project,
        app = %resolved.app_name,
        "Checking health of application"
    );
    println!(
        "Checking health of: {}/{}",
        resolved.project, resolved.app_name
    );

    let project_config = config
        .projects
        .get(&resolved.project)
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", resolved.project))?;
    let app_config = project_config
        .apps
        .get(&resolved.app_name)
        .ok_or_else(|| anyhow::anyhow!("App '{}' not found", resolved.app_name))?;
    let health_check = app_config.health_check.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "No health check configured for {}/{}",
            resolved.project,
            resolved.app_name
        )
    })?;

    let store = state_store()?;
    store.cleanup_dead()?;
    let mut process = find_process(
        &store,
        &resolved.project,
        &resolved.app_name,
        args.environment.as_deref(),
    )?
    .ok_or_else(|| {
        anyhow::anyhow!(
            "App {}/{} is not currently running",
            resolved.project,
            resolved.app_name
        )
    })?;

    if !store.is_running(&process) {
        store.delete(&process.id)?;
        anyhow::bail!(
            "App {}/{} is not currently running",
            resolved.project,
            resolved.app_name
        );
    }

    let pm_check = to_pm_health_check(Some(health_check));

    tracing::debug!(
        pid = process.pid,
        health_check = ?pm_check,
        "Starting health check execution"
    );

    println!("Process ID: {}", process.pid);
    println!("Health check type: {:?}", pm_check);
    println!();
    println!("Executing health check...");

    let engine = HealthCheckEngine::new();
    let start = std::time::Instant::now();
    let check_result = engine.check(&pm_check).await;
    let duration = start.elapsed();

    process.runtime.last_health_check = Some(Utc::now());
    match check_result {
        Ok(true) => {
            process.runtime.health_failures = 0;
            store.save(&process)?;

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
            Ok(())
        }
        Ok(false) => {
            process.runtime.health_failures += 1;
            store.save(&process)?;

            tracing::warn!(
                project = %resolved.project,
                app = %resolved.app_name,
                duration_secs = %duration.as_secs_f64(),
                failures = process.runtime.health_failures,
                result = "failed",
                "Health check failed"
            );

            println!("❌ Health check FAILED ({:.2}s)", duration.as_secs_f64());
            println!();
            println!("Status: Unhealthy");
            println!("Consecutive failures: {}", process.runtime.health_failures);
            if let Some(last_check) = process.runtime.last_health_check {
                println!("Last check: {}", last_check.format("%Y-%m-%d %H:%M:%S"));
            }
            anyhow::bail!("Health check failed");
        }
        Err(e) => {
            process.runtime.health_failures += 1;
            store.save(&process)?;

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
