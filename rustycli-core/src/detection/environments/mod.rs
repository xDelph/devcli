// Environment detection module
// Detects how apps can be run in different environments (local, docker, k8s)

pub mod local;
pub mod docker;
pub mod k8s;

// Re-export main functions for convenience
pub use local::detect_local_commands;
pub use docker::detect_docker_commands;
pub use k8s::detect_k8s_commands;