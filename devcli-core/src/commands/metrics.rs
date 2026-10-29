// Metrics command - display process and monitor metrics
//
// Queries the metrics HTTP API (`devcli monitor --daemon`) when it is up, and
// otherwise computes the same process/system metrics from the state store.

use crate::Result;

/// Execute the metrics command
///
/// Fetches metrics from the HTTP API and displays them in a
/// human-readable format
#[tracing::instrument]
pub async fn metrics_command() -> Result<()> {
    tracing::info!("Fetching metrics");

    let metrics = match fetch_from_api().await? {
        Some(metrics) => metrics,
        // No metrics server: pm-daemon (spawned by `start`) does not run one.
        // Every process/system figure comes from the state store anyway, so
        // compute it here instead of failing; only the in-memory performance
        // counters of a running `monitor --daemon` are unavailable.
        None => {
            crate::metrics::MetricsCollector::new()
                .collect_all()
                .await?
        }
    };

    tracing::debug!(
        total_processes = metrics.processes.total_processes,
        "Metrics collected"
    );

    // Agents get the raw metrics document; humans get the formatted view.
    if crate::output::json_enabled() {
        crate::output::print_json(&metrics)?;
        return Ok(());
    }

    display_metrics(&metrics);
    Ok(())
}

/// Metrics from a running metrics server, or `None` if none is listening.
async fn fetch_from_api() -> Result<Option<crate::metrics::AllMetrics>> {
    let url = format!("http://{}/metrics", crate::metrics::server::metrics_addr());

    let response = match reqwest::get(&url).await {
        Ok(response) => response,
        Err(e) if e.is_connect() => {
            tracing::debug!(url = %url, "No metrics server, collecting locally");
            return Ok(None);
        }
        Err(e) => anyhow::bail!("Failed to fetch metrics from {}: {}", url, e),
    };

    if !response.status().is_success() {
        anyhow::bail!("Metrics API at {} returned {}", url, response.status());
    }
    Ok(Some(response.json().await?))
}

/// Display metrics in a human-readable format
fn display_metrics(metrics: &crate::metrics::AllMetrics) {
    use colored::Colorize;

    println!();
    println!("{}", "=== Process Metrics ===".bold().cyan());
    println!(
        "Total Processes:       {}",
        metrics.processes.total_processes
    );
    println!(
        "  Running:             {} {}",
        metrics.processes.running_processes,
        "●".green()
    );
    println!(
        "  Stopped:             {} {}",
        metrics.processes.stopped_processes,
        "○".bright_black()
    );
    println!();
    println!(
        "Total Restarts:        {}",
        metrics.processes.total_restarts
    );
    println!(
        "Restarts (last hour):  {}",
        metrics.processes.restarts_last_hour
    );
    println!(
        "Health Check Success:  {:.1}%",
        metrics.processes.health_check_success_rate * 100.0
    );

    // Show exit code distribution if there are any
    if !metrics.processes.exit_code_distribution.is_empty() {
        println!();
        println!("Exit Code Distribution:");
        let mut codes: Vec<_> = metrics.processes.exit_code_distribution.iter().collect();
        codes.sort_by_key(|(code, _)| **code);
        for (code, count) in codes {
            let color = if *code == 0 { "green" } else { "red" };
            println!(
                "  Exit {}: {} times {}",
                code,
                count,
                if color == "green" {
                    "✓".green()
                } else {
                    "✗".red()
                }
            );
        }
    }

    println!();
    println!("{}", "=== System Metrics ===".bold().cyan());
    println!(
        "Monitor Uptime:        {} seconds ({} minutes)",
        metrics.system.monitor_uptime_seconds,
        metrics.system.monitor_uptime_seconds / 60
    );
    println!(
        "Loop Iterations:       {}",
        metrics.system.monitor_loop_iterations
    );
    println!("DevCLI Version:        {}", metrics.system.devcli_version);
    println!(
        "Apps Configured:       {}",
        metrics.system.total_apps_configured
    );

    println!();
    println!("{}", "=== Performance Metrics ===".bold().cyan());
    println!(
        "Avg Startup Time:      {:.2} ms",
        metrics.performance.avg_startup_time_ms
    );
    println!(
        "Avg Health Check:      {:.2} ms",
        metrics.performance.avg_health_check_duration_ms
    );
    println!(
        "Avg Restart Time:      {:.2} ms",
        metrics.performance.avg_restart_duration_ms
    );

    // Show recent operations (last 5)
    if !metrics.performance.recent_operations.is_empty() {
        println!();
        println!("Recent Operations (last 5):");
        let recent = if metrics.performance.recent_operations.len() > 5 {
            &metrics.performance.recent_operations
                [metrics.performance.recent_operations.len() - 5..]
        } else {
            &metrics.performance.recent_operations[..]
        };

        for op in recent {
            let status_icon = if op.success {
                "✓".green()
            } else {
                "✗".red()
            };
            println!(
                "  {} {} - {} ({} ms)",
                status_icon, op.operation, op.app, op.duration_ms
            );
        }
    }

    // Show app details if not too many
    if metrics.processes.apps.len() <= 10 && !metrics.processes.apps.is_empty() {
        println!();
        println!("{}", "=== Running Apps ===".bold().cyan());
        for app in &metrics.processes.apps {
            if app.status == "running" {
                let health_icon = match app.health_status.as_str() {
                    "healthy" => "✓".green(),
                    "unhealthy" => "✗".red(),
                    _ => "?".yellow(),
                };

                let uptime = if let Some(secs) = app.uptime_seconds {
                    format!("{}s", secs)
                } else {
                    "-".to_string()
                };

                println!(
                    "  {} {}/{} (uptime: {}, restarts: {})",
                    health_icon, app.project, app.name, uptime, app.restart_count
                );
            }
        }
    }

    println!();
    println!("Timestamp: {}", metrics.timestamp);
    println!();
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_metrics_command_exists() {
        // Basic compilation test
        // No assertions needed - if this compiles, the test passes
    }
}
