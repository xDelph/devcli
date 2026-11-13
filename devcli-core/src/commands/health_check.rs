// Health check command - manually check the health of a running application

use crate::config::load_config;
use crate::process_manager_support::{find_process, state_store, to_pm_health_check};
use crate::Result;
use chrono::Utc;
use process_manager::HealthCheckEngine;
use serde_json::json;

/// Arguments for the health-check command
#[derive(Debug)]
pub struct HealthCheckArgs {
    /// Name of the app to check (can be in format "project/app" or just "app")
    pub app_name: String,
    /// Project of the app, when its name is ambiguous
    pub project: Option<String>,
    /// Optional environment to check (local, docker, orbstack, k8s)
    pub environment: Option<String>,
}

/// Execute a manual health check on a running application
#[tracing::instrument(skip(args), fields(app_name = %args.app_name, environment = ?args.environment))]
pub async fn health_check_command(args: HealthCheckArgs) -> Result<()> {
    let config = load_config()?;
    let resolved = crate::worktrees::resolve(
        &config,
        &args.app_name,
        args.project.as_deref(),
        crate::worktrees::Mode::FromCwd,
    )?;

    tracing::info!(
        project = %resolved.project,
        app = %resolved.app_name,
        "Checking health of application"
    );
    let json = crate::output::json_enabled();
    if !json {
        println!(
            "Checking health of: {}/{}",
            resolved.project, resolved.app_name
        );
    }

    // `resolved.project` is `proj@slug` for a worktree instance, which the
    // config does not list: the resolved app carries the configuration.
    resolved.app.health_check.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "No health check configured for {}/{}",
            resolved.project,
            resolved.app_name
        )
    })?;

    let store = state_store()?;
    store.cleanup_dead()?;
    let process = find_process(
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
        // pm-daemon restarts it shortly: deleting the entry would cancel that.
        if !store.awaiting_restart(&process) {
            store.forget(&process.id, process.pid)?;
        }
        anyhow::bail!(
            "App {}/{} is not currently running",
            resolved.project,
            resolved.app_name
        );
    }

    // The check the process was started with: in a git worktree it targets
    // the instance's shifted port, not the configured one.
    let pm_check = match &process.task.health_check {
        process_manager::HealthCheck::Process {} => {
            to_pm_health_check(resolved.app.health_check.as_ref())
        }
        started_with => started_with.clone(),
    };

    tracing::debug!(
        pid = process.pid,
        health_check = ?pm_check,
        "Starting health check execution"
    );

    if !json {
        println!("Process ID: {}", process.pid);
        println!("Health check type: {:?}", pm_check);
        println!();
        println!("Executing health check...");
    }

    let engine = HealthCheckEngine::new();
    let start = std::time::Instant::now();
    let check_result = engine.check(&pm_check).await;
    let duration = start.elapsed();
    let checked_at = Utc::now();

    // Record the result on the entry as it is now, not on the copy read
    // before the check: a `stop` or a restart may have happened meanwhile,
    // and writing that copy back would undo it. Returns the failure count.
    let record = |healthy: bool| -> Result<u32> {
        let updated = store.update_if_current(&process.id, process.pid, |current| {
            current.runtime.last_health_check = Some(checked_at);
            if healthy {
                current.runtime.health_failures = 0;
            } else {
                current.runtime.health_failures += 1;
            }
        })?;
        Ok(match updated {
            Some(current) => current.runtime.health_failures,
            None if healthy => 0,
            None => process.runtime.health_failures + 1,
        })
    };

    match check_result {
        Ok(true) => {
            record(true)?;

            tracing::info!(
                project = %resolved.project,
                app = %resolved.app_name,
                duration_secs = %duration.as_secs_f64(),
                result = "passed",
                "Health check passed"
            );

            if json {
                crate::output::print_json(&json!({
                    "project": resolved.project,
                    "app": resolved.app_name,
                    "pid": process.pid,
                    "healthy": true,
                    "duration_seconds": duration.as_secs_f64(),
                }))?;
            } else {
                println!("✅ Health check PASSED ({:.2}s)", duration.as_secs_f64());
                println!();
                println!("Status: Healthy");
            }
            crate::output::set_exit_code(0);
            Ok(())
        }
        Ok(false) => {
            let failures = record(false)?;

            tracing::warn!(
                project = %resolved.project,
                app = %resolved.app_name,
                duration_secs = %duration.as_secs_f64(),
                failures,
                result = "failed",
                "Health check failed"
            );

            if json {
                crate::output::print_json(&json!({
                    "project": resolved.project,
                    "app": resolved.app_name,
                    "pid": process.pid,
                    "healthy": false,
                    "duration_seconds": duration.as_secs_f64(),
                    "failures": failures,
                }))?;
            } else {
                println!("❌ Health check FAILED ({:.2}s)", duration.as_secs_f64());
                println!();
                println!("Status: Unhealthy");
                println!("Consecutive failures: {}", failures);
                println!("Last check: {}", checked_at.format("%Y-%m-%d %H:%M:%S"));
            }
            // Exit code 2 = unhealthy (distinct from 1 = error).
            crate::output::set_exit_code(2);
            Ok(())
        }
        Err(e) => {
            record(false)?;

            tracing::error!(
                project = %resolved.project,
                app = %resolved.app_name,
                duration_secs = %duration.as_secs_f64(),
                error = %e,
                result = "error",
                "Health check encountered error"
            );

            if json {
                crate::output::print_json(&json!({
                    "project": resolved.project,
                    "app": resolved.app_name,
                    "pid": process.pid,
                    "healthy": false,
                    "error": e.to_string(),
                }))?;
            } else {
                println!("⚠️  Health check ERROR ({:.2}s)", duration.as_secs_f64());
                println!();
                println!("Error: {}", e);
            }
            // Exit code 1 = execution error.
            crate::output::set_exit_code(1);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_check_args_creation() {
        let args = HealthCheckArgs {
            project: None,
            app_name: "test-app".to_string(),
            environment: Some("local".to_string()),
        };

        assert_eq!(args.app_name, "test-app");
        assert_eq!(args.environment, Some("local".to_string()));
    }

    #[test]
    fn test_health_check_args_no_env() {
        let args = HealthCheckArgs {
            project: None,
            app_name: "test-app".to_string(),
            environment: None,
        };

        assert_eq!(args.app_name, "test-app");
        assert!(args.environment.is_none());
    }
}
