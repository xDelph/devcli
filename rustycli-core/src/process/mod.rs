// Module file for process-related functionality
// Rust uses 'mod' to declare submodules and organize code

pub mod spawner; // Handles spawning child processes
pub mod tracker; // Tracks process PIDs and metadata
pub mod monitor; // Background monitor for process health

#[cfg(test)]
mod tracker_tests;

// Re-export important types and functions so users can import them directly
// Instead of: use rustycli_core::process::spawner::ProcessOptions;
// They can do: use rustycli_core::process::ProcessOptions;
pub use spawner::{spawn_process, ProcessOptions, SpawnedProcess};
pub use tracker::{ProcessInfo, ProcessTracker};
