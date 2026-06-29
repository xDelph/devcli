// Environment detection module
// Detects how apps can be run in different environments (local, docker, k8s, orbstack)

pub mod docker;
pub mod env_files;
pub mod k8s;
pub mod local;
pub mod orbstack;

// Test modules

// Re-export main functions for convenience
pub use docker::detect_docker_commands;
pub use env_files::{
    build_env_files_map, build_env_files_map_interactive, detect_env_files,
};
pub use k8s::detect_k8s_commands;
pub use local::detect_local_commands;
pub use orbstack::detect_orbstack_commands;
