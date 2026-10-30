// Tracing initialization with JSON structured logging
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
    // Next to the apps' logs, in the devcli directory (DEVCLI_CONFIG_DIR is
    // honoured: it used to be ~/.devcli/logs whatever the override).
    let log_dir = crate::config::loader::config_home().join("logs");

    // File appender with daily rotation, JSON for machines. A log directory
    // that cannot be written (a read-only devcli directory) used to make
    // every command panic at start-up; devcli now runs without its log file.
    let file_layer = std::fs::create_dir_all(&log_dir)
        .ok()
        .and_then(|()| {
            rolling::RollingFileAppender::builder()
                .rotation(rolling::Rotation::DAILY)
                .filename_prefix("devcli.json")
                .build(&log_dir)
                .ok()
        })
        .map(|appender| fmt::layer().json().with_writer(appender));

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
    fn the_log_directory_follows_the_devcli_directory() {
        let dir = crate::test_utils::TempConfigDir::new();
        assert_eq!(
            crate::config::loader::config_home().join("logs"),
            dir.path().join("logs")
        );
    }
}
