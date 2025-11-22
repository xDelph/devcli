// Environment detection module
// Detects how apps can be run in different environments (local, docker, k8s, orbstack)

pub mod local;
pub mod docker;
pub mod k8s;
pub mod orbstack;
pub mod env_files;

// Test modules
#[cfg(test)]
mod orbstack_stage_test;

// Re-export main functions for convenience
pub use local::detect_local_commands;
pub use docker::detect_docker_commands;
pub use k8s::detect_k8s_commands;
pub use orbstack::{detect_orbstack_commands, load_env_vars_for_runtime, find_env_file, resolve_env_file_path};
pub use env_files::{detect_env_files, build_env_files_map, build_env_files_map_interactive};