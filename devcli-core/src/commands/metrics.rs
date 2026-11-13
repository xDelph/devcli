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

/// How long to wait for the metrics server before computing the metrics here.
const API_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Metrics from a running metrics server, or `None` if none answers.
async fn fetch_from_api() -> Result<Option<crate::metrics::AllMetrics>> {
    let url = format!("http://{}/metrics", crate::metrics::server::metrics_addr());
    Ok(fetch_from(&url, API_TIMEOUT).await)
}

/// The metrics document served at `url`, or `None` when what answers there is
/// not devcli's metrics server.
///
/// Only a refused connection used to fall back to local metrics: whatever else
/// listened on the port (Prometheus' own 9090) failed the command with a JSON
/// decode error, and a listener that never answered hung it forever. Any
/// failure - timeout, error status, another content type, a document that does
/// not decode - now falls back.
async fn fetch_from(url: &str, timeout: std::time::Duration) -> Option<crate::metrics::AllMetrics> {
    let fallback = |reason: String| {
        tracing::debug!(url = %url, reason = %reason, "No devcli metrics server, collecting locally");
        None
    };

    let client = match reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout)
        .build()
    {
        Ok(client) => client,
        Err(e) => return fallback(e.to_string()),
    };
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(e) => return fallback(e.to_string()),
    };
    if !response.status().is_success() {
        return fallback(format!("status {}", response.status()));
    }
    let is_json = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if !is_json {
        return fallback("not a JSON response".to_string());
    }
    match response.json().await {
        Ok(metrics) => Some(metrics),
        Err(e) => fallback(e.to_string()),
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
    use super::*;
    use std::time::{Duration, Instant};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// A one-connection HTTP server answering `response` (or never, if `None`).
    async fn serve_once(response: Option<&'static str>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            match response {
                Some(response) => {
                    let _ = socket.write_all(response.as_bytes()).await;
                }
                None => tokio::time::sleep(Duration::from_secs(30)).await,
            }
        });
        format!("http://{addr}/metrics")
    }

    #[tokio::test]
    async fn another_server_on_the_port_falls_back() {
        let url = serve_once(Some(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\n\
             Content-Length: 12\r\nConnection: close\r\n\r\nup{job=\"x\"} 1",
        ))
        .await;
        assert!(fetch_from(&url, Duration::from_secs(2)).await.is_none());

        let url = serve_once(Some(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
             Content-Length: 12\r\nConnection: close\r\n\r\n{\"up\": true}",
        ))
        .await;
        assert!(fetch_from(&url, Duration::from_secs(2)).await.is_none());

        let url = serve_once(Some(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        ))
        .await;
        assert!(fetch_from(&url, Duration::from_secs(2)).await.is_none());
    }

    #[tokio::test]
    async fn a_silent_listener_times_out_and_falls_back() {
        let url = serve_once(None).await;
        let started = Instant::now();
        assert!(fetch_from(&url, Duration::from_millis(300)).await.is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
