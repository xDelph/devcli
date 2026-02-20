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
mod state_tests;
