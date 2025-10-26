// This is the main library file that exposes our modules to other crates
// The 'pub' keyword makes these modules publicly accessible

pub mod commands; // Contains CLI command implementations (start, status)
pub mod logging; // Handles log file writing
pub mod process; // Manages process spawning and tracking

// Re-export commonly used types from the anyhow crate for error handling
// This allows users of our library to use these types without importing anyhow directly
pub use anyhow::{Context, Result};
// Context: Adds helpful context messages to errors
// Result: A type that represents either success (Ok) or failure (Err)
