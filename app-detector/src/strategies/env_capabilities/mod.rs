//! Environment Capability Detection Strategies
//!
//! These strategies identify HOW the application can be run:
//! - Local development (npm scripts, cargo commands, etc.)
//! - Docker containers
//! - OrbStack environments
//! - Kubernetes deployments
//!
//! Environment capability strategies run second (Phase 2) with priorities 400-599.
//! They can depend on app type detection results to extract context-aware commands.

pub mod docker;
pub mod kubernetes_env;
pub mod local_env;
pub mod orbstack_env;

// Re-export all environment capability strategies
pub use docker::DockerStrategy;
pub use kubernetes_env::KubernetesEnvStrategy;
pub use local_env::LocalEnvStrategy;
pub use orbstack_env::OrbStackEnvStrategy;
