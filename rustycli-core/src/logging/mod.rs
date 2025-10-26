// Module file for logging functionality

pub mod file_logger; // Contains the FileLogger implementation

// Re-export FileLogger so users can import it directly
// use rustycli_core::logging::FileLogger; instead of
// use rustycli_core::logging::file_logger::FileLogger;
pub use file_logger::FileLogger;
