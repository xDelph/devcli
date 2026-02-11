// Metrics HTTP server - exposes metrics via JSON API
//
// Provides:
// - HTTP server on localhost:9090
// - GET /metrics endpoint returning JSON
// - Non-blocking async server
// - Error handling and logging

use super::collector::MetricsCollector;
use crate::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Start the metrics HTTP server on localhost:9090
///
/// This function spawns a background task that listens for HTTP requests
/// and serves metrics data as JSON. The server runs indefinitely until
/// the process exits.
#[tracing::instrument(skip(collector))]
pub async fn start_metrics_server(collector: Arc<MetricsCollector>) -> Result<()> {
    let addr = "127.0.0.1:9090";
    let listener = TcpListener::bind(addr).await?;

    tracing::info!(
        address = %addr,
        "Metrics API server started"
    );

    loop {
        let (mut socket, remote_addr) = listener.accept().await?;
        let collector = collector.clone();

        // Spawn a new task for each connection
        tokio::spawn(async move {
            let mut buffer = [0; 2048];

            match socket.read(&mut buffer).await {
                Ok(n) if n > 0 => {
                    let request = String::from_utf8_lossy(&buffer[..n]);

                    tracing::debug!(
                        remote_addr = %remote_addr,
                        request_line = %request.lines().next().unwrap_or(""),
                        "Received HTTP request"
                    );

                    // Parse request line
                    if request.starts_with("GET /metrics") {
                        handle_metrics_request(&mut socket, collector).await;
                    } else if request.starts_with("GET /") {
                        handle_root_request(&mut socket).await;
                    } else {
                        handle_not_found(&mut socket).await;
                    }
                }
                Ok(_) => {
                    tracing::debug!("Empty request received");
                }
                Err(e) => {
                    tracing::error!(error = %e, "Error reading from socket");
                }
            }
        });
    }
}

/// Handle GET /metrics request
async fn handle_metrics_request(
    socket: &mut tokio::net::TcpStream,
    collector: Arc<MetricsCollector>,
) {
    match collector.collect_all().await {
        Ok(metrics) => match serde_json::to_string_pretty(&metrics) {
            Ok(json) => {
                tracing::debug!(size_bytes = json.len(), "Sending metrics response");

                let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\n\r\n{}",
                        json.len(),
                        json
                    );

                if let Err(e) = socket.write_all(response.as_bytes()).await {
                    tracing::error!(error = %e, "Failed to write response");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to serialize metrics");
                send_error_response(socket, 500, "Internal Server Error").await;
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to collect metrics");
            send_error_response(socket, 500, "Internal Server Error").await;
        }
    }
}

/// Handle GET / request (root)
async fn handle_root_request(socket: &mut tokio::net::TcpStream) {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>DevCLI Metrics API</title>
    <style>
        body { font-family: sans-serif; max-width: 800px; margin: 50px auto; padding: 20px; }
        code { background: #f4f4f4; padding: 2px 6px; border-radius: 3px; }
        pre { background: #f4f4f4; padding: 15px; border-radius: 5px; overflow-x: auto; }
    </style>
</head>
<body>
    <h1>DevCLI Metrics API</h1>
    <p>Welcome to the DevCLI metrics API server.</p>

    <h2>Available Endpoints</h2>
    <ul>
        <li><code>GET /metrics</code> - Get all metrics as JSON</li>
    </ul>

    <h2>Example Usage</h2>
    <pre>curl http://localhost:9090/metrics | jq</pre>

    <h2>Quick Links</h2>
    <ul>
        <li><a href="/metrics">View Metrics JSON</a></li>
    </ul>
</body>
</html>"#;

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
        html.len(),
        html
    );

    let _ = socket.write_all(response.as_bytes()).await;
}

/// Handle 404 Not Found
async fn handle_not_found(socket: &mut tokio::net::TcpStream) {
    send_error_response(socket, 404, "Not Found").await;
}

/// Send an error response
async fn send_error_response(socket: &mut tokio::net::TcpStream, code: u16, message: &str) {
    let json = serde_json::json!({
        "error": message,
        "code": code
    });

    let body = json.to_string();
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        code,
        message,
        body.len(),
        body
    );

    let _ = socket.write_all(response.as_bytes()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_server_module_exists() {
        // Basic module structure test
        let collector = Arc::new(MetricsCollector::new());
        assert!(Arc::strong_count(&collector) >= 1);
    }
}
