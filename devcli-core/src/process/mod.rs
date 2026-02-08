// Module file for process-related functionality
// Rust uses 'mod' to declare submodules and organize code

pub mod spawner; // Handles spawning child processes
pub mod tracker; // Tracks process PIDs and metadata
pub mod monitor; // Background monitor for process health
pub mod health_check; // Health check engine for monitoring process health
pub mod restart_coordinator; // Coordinates restart operations to prevent duplicates

#[cfg(test)]
mod tracker_tests;

// Re-export important types and functions so users can import them directly
// Instead of: use devcli_core::process::spawner::ProcessOptions;
// They can do: use devcli_core::process::ProcessOptions;
pub use spawner::{spawn_process, ProcessOptions, SpawnedProcess};
pub use tracker::{ProcessInfo, ProcessTracker, RestartEvent, RestartReason};
pub use health_check::HealthCheckEngine;
pub use restart_coordinator::RestartCoordinator;
