// Metrics command - display metrics from the monitor daemon
//
// Queries the metrics HTTP API at localhost:9090 and displays
// formatted metrics information

use crate::Result;

/// Execute the metrics command
///
/// Fetches metrics from the HTTP API and displays them in a
/// human-readable format
#[tracing::instrument]
pub async fn metrics_command() -> Result<()> {
    tracing::info!("Fetching metrics from API");

    let url = "http://127.0.0.1:9090/metrics";

    // Try to fetch metrics
    match reqwest::get(url).await {
        Ok(response) => {
            if !response.status().is_success() {
                anyhow::bail!(
                    "Metrics API returned error: {}. Is the monitor running?",
                    response.status()
                );
            }

            let metrics: crate::metrics::AllMetrics = response.json().await?;

            tracing::debug!(
                total_processes = metrics.processes.total_processes,
                "Metrics fetched successfully"
            );

            // Display metrics in a formatted way
            display_metrics(&metrics);

            Ok(())
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch metrics");

            // Check if it's a connection error
            if e.is_connect() {
                anyhow::bail!(
                    "Failed to connect to metrics API at {}.\n\
                    Is the monitor daemon running? Try: devcli monitor --daemon",
                    url
                );
            } else {
                anyhow::bail!("Failed to fetch metrics: {}", e);
            }
        }
    }
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
