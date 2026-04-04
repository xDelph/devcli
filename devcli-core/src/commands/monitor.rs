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

    let metrics = Arc::new(MetricsCollector::new());
    let metrics_clone = metrics.clone();
    tokio::spawn(async move {
        if let Err(e) = start_metrics_server(metrics_clone).await {
            tracing::error!(error = %e, "Metrics server error");
        }
    });

    let base_dir = state_dir()?;
    let state = Arc::new(StateStore::new(base_dir.clone())?);
    let _ = state.cleanup_dead();

    let monitor = Monitor::new(
        state,
        Arc::new(HealthCheckEngine::new()),
        Arc::new(RestartCoordinator::new(base_dir.join("locks"))),
    );

    tracing::info!("Monitor daemon starting (process-manager)");
    monitor.run().await?;
    Ok(())
}
