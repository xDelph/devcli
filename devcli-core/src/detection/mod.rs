// App detection — powered exclusively by the `app-detector` crate via `app_detector_support`.
// Env file discovery uses env-flow (`env_files` module). Nx monorepo handling in `nx.rs`.

mod env_files;
mod nx;

#[cfg(test)]
mod tests;

pub use env_files::{
    build_env_files_map, build_env_files_map_interactive, detect_env_files, DetectedEnvFile,
};
pub use nx::{detect_nx_apps, detect_single_nx_app};

use std::collections::HashMap;
use std::path::Path;
use crate::Result;

/// Complete detection results for an app.
#[derive(Debug, Clone)]
pub struct DetectedApp {
    pub app_type: String,
    pub app_name: String,
    pub path: String,
    pub local_commands: Option<HashMap<String, String>>,
    pub docker_commands: Option<HashMap<String, String>>,
    pub orbstack_commands: Option<HashMap<String, String>>,
    pub k8s_commands: Option<HashMap<String, String>>,
    pub suggested_local_default: Option<String>,
    pub suggested_docker_default: Option<String>,
    pub suggested_orbstack_default: Option<String>,
    pub dockerfile_path: Option<String>,
    pub env_files: Option<HashMap<String, HashMap<String, String>>>,
}

/// Detect a single app directory.
pub fn detect_app(path: &Path) -> Result<DetectedApp> {
    crate::app_detector_support::detect_app(path)
}

/// Detect app type only (Phase 1).
pub fn detect_app_type(path: &Path) -> Result<String> {
    crate::app_detector_support::detect_app_type(path)
}

/// Extract app name from detection metadata.
pub fn extract_app_name(path: &Path, app_type: &str) -> Result<String> {
    crate::app_detector_support::extract_app_name(path, app_type)
}

#[cfg(test)]
#[path = "app_types_tests.rs"]
mod app_types_tests;
