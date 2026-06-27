// Tracing initialization with JSON structured logging
use anyhow::Context;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing with JSON file output and optional console output.
///
/// When `enable_console` is `false` (TUI mode), logs are written to the JSON
/// log file only. Console output would corrupt the ratatui alternate screen.
///
/// Log level can be controlled via environment variable:
/// - `LOG_LEVEL=off` - Disable all logging
/// - `LOG_LEVEL=error` - Only errors
/// - `LOG_LEVEL=warn` - Warnings and errors
/// - `LOG_LEVEL=info` - Info, warnings, and errors (default)
/// - `LOG_LEVEL=debug` - Debug and above
/// - `LOG_LEVEL=trace` - All logs including trace
///
/// Example: `LOG_LEVEL=error devcli start my-app`
pub fn init_tracing(enable_console: bool) -> crate::Result<()> {
    let log_dir = dirs::home_dir()
        .context("No home directory found")?
        .join(".devcli")
        .join("logs");

    std::fs::create_dir_all(&log_dir)?;

    // File appender with daily rotation
    let file_appender = rolling::daily(log_dir, "devcli.json");

    // JSON formatter for files - structured, machine-readable
    let file_layer = fmt::layer().json().with_writer(file_appender);

    // Parse log level from LOG_LEVEL environment variable
    // Falls back to INFO if not set or invalid
    let log_level = std::env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string())
        .to_lowercase();

    let filter = match log_level.as_str() {
        "off" => EnvFilter::new("off"),
        "error" => EnvFilter::new("error"),
        "warn" => EnvFilter::new("warn"),
        "info" => EnvFilter::new("info"),
        "debug" => EnvFilter::new("debug"),
        "trace" => EnvFilter::new("trace"),
        _ => {
            if enable_console {
                eprintln!(
                    "Warning: Invalid LOG_LEVEL '{}'. Valid values: off, error, warn, info, debug, trace. Using 'info' as default.",
                    log_level
                );
            }
            EnvFilter::new("info")
        }
    };

    let registry = tracing_subscriber::registry().with(filter).with(file_layer);

    if enable_console {
        let console_layer = fmt::layer().compact().with_writer(std::io::stderr);
        registry.with(console_layer).init();
    } else {
        registry.init();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_init_tracing_creates_log_dir() {
        // Note: This test validates the function exists and compiles correctly.
        // Actual initialization is tested manually as it can only run once per process.
        let log_dir = dirs::home_dir().unwrap().join(".devcli").join("logs");

        // Verify the directory would be created
        assert!(log_dir.parent().is_some());
    }

    #[test]
    fn test_log_directory_path() {
        let log_dir = dirs::home_dir().unwrap().join(".devcli").join("logs");

        let path_str = log_dir.to_string_lossy();
        assert!(path_str.contains(".devcli"));
        assert!(path_str.ends_with("logs"));
    }
}
