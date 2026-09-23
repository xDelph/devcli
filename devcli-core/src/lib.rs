// This is the main library file that exposes our modules to other crates
// The 'pub' keyword makes these modules publicly accessible

pub mod commands;
pub mod config;
pub mod detection;
pub mod logging;
pub mod metrics;
pub mod app_detector_support;
pub mod config_manager_support;
pub mod env_flow_support;
pub mod process_manager_support;
pub mod tui;
pub mod utils;

// Test utilities for creating mock data in tests
// Available for unit tests and integration tests
#[cfg(test)]
pub mod test_utils;

// Integration tests
#[cfg(test)]
mod integration_tests;

// Re-export commonly used types from the anyhow crate for error handling
// This allows users of our library to use these types without importing anyhow directly
pub use anyhow::{Context, Result};
// Context: Adds helpful context messages to errors
// Result: A type that represents either success (Ok) or failure (Err)
