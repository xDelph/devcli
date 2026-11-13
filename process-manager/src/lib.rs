// Process groups, signals, setsid and `ps` are the whole mechanism: there is
// no fallback that would behave correctly elsewhere, and stubs that compiled
// (every PID alive, nothing ever running) were silently wrong.
#[cfg(not(unix))]
compile_error!("process-manager supports Unix-like systems only (Linux, macOS)");

pub mod engine;
pub mod health;
pub mod model;
pub mod monitor;
pub mod restart;
pub mod state;

pub use health::HealthCheckEngine;
pub use model::{HealthCheck, ProcessMetadata, RestartPolicy, Task};
pub use monitor::Monitor;
pub use restart::RestartCoordinator;
pub use state::{ManagedProcess, StateStore};

#[cfg(test)]
mod health_tests;
#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod monitor_tests;
#[cfg(test)]
mod restart_tests;
#[cfg(test)]
mod state_tests;
