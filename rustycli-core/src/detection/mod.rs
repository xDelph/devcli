// App detection module - Automatically discovers app configurations
// Scans directories to identify app types (Node.js, Nx, Python, Redis, Traefik)
// and detect available environments (local, Docker, Kubernetes)
//
// Detection is two-phase:
// 1. App Type Detection: What is the app? (nodejs, nx, python, etc.)
// 2. Environment Detection: How can we run it? (local commands, docker, k8s)
//
// Module Organization:
// - app_types.rs: App type detection and name extraction
// - environments/: Environment-specific detection (local, docker, k8s)
// - nx.rs: Nx monorepo specific logic
// - utils.rs: Shared file finding utilities
// - dockerfile.rs: Dockerfile parsing (existing)

mod app_types;
mod environments;
mod nx;
mod utils;
mod dockerfile;

#[cfg(test)]
mod detection_tests;

use crate::Result;
use crate::utils::path::contract_tilde;
use std::collections::HashMap;
use std::path::Path;

// Re-export main functions to maintain public API
pub use app_types::{detect_app_type, extract_app_name};
pub use environments::{detect_local_commands, detect_docker_commands, detect_k8s_commands};
pub use environments::local::suggest_local_default;
pub use environments::docker::suggest_docker_default;
pub use nx::{detect_nx_apps, detect_single_nx_app};

// Structure holding all detection results for an app
// Contains app metadata, commands for different environments, and suggested defaults
#[derive(Debug, Clone)]
pub struct DetectedApp {
    pub app_type: String,               // Type of app: nodejs, nx, python, redis, traefik
    pub app_name: String,               // Name extracted from config files or directory
    pub path: String,                   // Absolute path to app directory
    pub local_commands: Option<HashMap<String, String>>,  // Commands for local environment
    pub docker_commands: Option<HashMap<String, String>>, // Commands for Docker environment
    pub k8s_commands: Option<HashMap<String, String>>,    // Commands for Kubernetes
    pub suggested_local_default: Option<String>,          // Suggested default local command
    pub suggested_docker_default: Option<String>,         // Suggested default docker command
}

/// Main detection function for a single app
/// Runs two-phase detection: type first, then environments
/// 
/// # Arguments
/// * `path` - Directory to scan for app configuration
/// 
/// # Returns
/// Complete detection results with all environments
pub fn detect_app(path: &Path) -> Result<DetectedApp> {
    // Phase 1: Detect what type of app this is
    let app_type = detect_app_type(path)?;
    
    // Extract app name from configuration files or directory name
    let app_name = extract_app_name(path, &app_type)?;
    
    // Phase 2: Detect how we can run this app (local, docker, k8s)
    let local_commands = detect_local_commands(path, &app_type)?;
    let docker_commands = detect_docker_commands(path, &app_type)?;
    let k8s_commands = detect_k8s_commands(path)?;
    
    // Suggest sensible defaults based on common command names
    let suggested_local_default = suggest_local_default(&app_type, &local_commands);
    let suggested_docker_default = suggest_docker_default(&docker_commands);
    
    // Build and return the complete detection result
    Ok(DetectedApp {
        app_type,
        app_name,
        path: contract_tilde(path),
        local_commands,
        docker_commands,
        k8s_commands,
        suggested_local_default,
        suggested_docker_default,
    })
}














