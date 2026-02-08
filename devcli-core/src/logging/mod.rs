// Module file for logging functionality

pub mod file_logger; // Contains the FileLogger implementation
pub mod monitor_logger; // Contains the MonitorLogger implementation for monitor daemon

// Re-export FileLogger so users can import it directly
// use devcli_core::logging::FileLogger; instead of
// use devcli_core::logging::file_logger::FileLogger;
pub use file_logger::FileLogger;
pub use monitor_logger::MonitorLogger;
