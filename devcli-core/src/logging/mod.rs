// Module file for logging functionality

pub mod file_logger; // Contains the FileLogger implementation

// Re-export FileLogger so users can import it directly
// use devcli_core::logging::FileLogger; instead of
// use devcli_core::logging::file_logger::FileLogger;
pub use file_logger::FileLogger;
