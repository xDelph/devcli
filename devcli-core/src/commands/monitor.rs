// Monitor command - foreground monitor loop backed by process-manager

use crate::metrics::{start_metrics_server, MetricsCollector};
use crate::process_manager_support::{state_dir, state_store};
use crate::Result;
use process_manager::{HealthCheckEngine, Monitor, RestartCoordinator, StateStore};
use std::sync::Arc;

/// Main entry point for monitoring.
///
/// - `daemon = false`: run a one-shot cleanup of dead state entries.
/// - `daemon = true`: run the process-manager monitor loop in foreground.
#[tracing::instrument]
pub async fn monitor_command(daemon: bool) -> Result<()> {
    if !daemon {
        let store = state_store()?;
        let cleaned = store.cleanup_dead()?;

        if crate::output::json_enabled() {
            return crate::output::print_json(&serde_json::json!({ "cleaned": cleaned }));
        }
        if cleaned.is_empty() {
            println!("No dead processes found");
        } else {
            println!("Cleaned up {} dead process(es):", cleaned.len());
            for id in cleaned {
                println!("  - {}", id);
            }
        }
        return Ok(());
    }

    let base_dir = state_dir()?;
    let state = Arc::new(StateStore::new(base_dir.clone())?);

    // pm-daemon (spawned by `start`) holds the same lock. A second monitor loop
    // next to it would restart every crashed app twice.
    let Some(_lock) = state.try_lock_daemon()? else {
        anyhow::bail!(
            "A monitor (pm-daemon) is already running for {}",
            base_dir.display()
        );
    };

    let metrics = Arc::new(MetricsCollector::new());
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        if let Err(e) = start_metrics_server(metrics_clone).await {
            tracing::error!(error = %e, "Metrics server error");
        }
    });

    let _ = state.cleanup_dead();

    // Long-running: with --json, one document saying it runs, nothing else.
    if crate::output::json_enabled() {
        crate::output::print_json(&serde_json::json!({
            "monitor": "running",
            "state_dir": base_dir,
            "pid": std::process::id(),
        }))?;
    }

    let monitor = Monitor::new(
        state,
        Arc::new(HealthCheckEngine::new()),
        Arc::new(RestartCoordinator::new(base_dir.join("locks"))),
    );

    tracing::info!("Monitor daemon starting (process-manager)");
    monitor.run().await?;
    Ok(())
}
