// Data models for configuration management
// These structs map directly to the JSON structure in ~/.rustycli/config.json
//
// serde is a library that automatically converts between Rust structs and JSON
// - Serialize: Convert Rust struct → JSON
// - Deserialize: Convert JSON → Rust struct
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Top-level config structure
// Maps to: { "projects": { "project-name": {...}, ... } }
//
// #[derive(...)] automatically implements traits:
// - Debug: Allows printing with {:?} for debugging
// - Clone: Allows making copies with .clone()
// - Serialize/Deserialize: Convert to/from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // HashMap allows dynamic keys (project names can be anything)
    // String = project name, Project = project data
    pub projects: HashMap<String, Project>,
}

// Project groups related apps together
// Example: "my-project" contains apps like "api", "frontend", "worker"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    // String = app name, App = app configuration
    pub apps: HashMap<String, App>,
}

// App configuration for a single application
// Contains everything needed to run it: where it is, what commands to use, dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    // The JSON field is "type" but we call it app_type because "type" is a Rust keyword
    // #[serde(rename = "type")] tells serde to map "type" in JSON to app_type in Rust
    #[serde(rename = "type")]
    pub app_type: String, // e.g., "nodejs", "nx", "redis", etc.
    
    // Working directory where the app lives
    // Supports ~ expansion (e.g., "~/Projects/my-app")
    pub path: String,
    
    // Commands for different environments (local, docker)
    pub commands: Commands,
    
    // Apps that must be running before this one can start
    // #[serde(default)] = if missing in JSON, use Vec::new() (empty vec)
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    
    // Which commands to use by default for each environment
    pub defaults: Defaults,
}

// Commands for different environments
// Example: { "local": { "start": "npm start" }, "docker": {...}, "k8s": {...} }
// An app can have ANY combination of environments - doesn't need all of them
// Examples:
//   - Only local: { "local": {...} }
//   - Only docker: { "docker": {...} }
//   - All three: { "local": {...}, "docker": {...}, "k8s": {...} }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commands {
    // Local development commands (run directly on your machine)
    // Key = command name (e.g., "start", "test", "build")
    // Value = shell command to execute (e.g., "npm start")
    // OPTIONAL: Not all apps need local commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<HashMap<String, String>>,
    
    // Docker commands (run in containers)
    // Key = command name (e.g., "build", "run")
    // Value = docker command (e.g., "docker build -t myapp .")
    // OPTIONAL: Not all apps need docker commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docker: Option<HashMap<String, String>>,
    
    // Kubernetes commands (deploy to k8s cluster)
    // Key = command name (e.g., "apply", "delete", "restart")
    // Value = kubectl command (e.g., "kubectl apply -f k8s/")
    // OPTIONAL: Not all apps need k8s commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k8s: Option<HashMap<String, String>>,
}

// Represents a dependency on another app
// Example: API depends on redis → { "project": "infrastructure", "app": "redis" }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    // Which project contains the dependency
    pub project: String,
    
    // Name of the app we depend on
    pub app: String,
}

// Default command names to use when starting an app
// Points to command names defined in the Commands struct
// Example: { "local": "start", "docker": "run", "k8s": "apply" }
// Only needs defaults for environments that have commands defined
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    // Default command for local environment (must exist in commands.local if provided)
    // OPTIONAL: Only needed if app has local commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    
    // Default command for docker environment (must exist in commands.docker if provided)
    // OPTIONAL: Only needed if app has docker commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docker: Option<String>,
    
    // Default command for k8s environment (must exist in commands.k8s if provided)
    // OPTIONAL: Only needed if app has k8s commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k8s: Option<String>,
}

// User preferences stored in ~/.rustycli/preferences.json
// These are personal settings that don't belong in the main config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preferences {
    // Which environment to use by default: "local" or "docker"
    // Can be overridden with --env flag on any command
    // #[serde(default = "default_env")] = call default_env() if missing in JSON
    #[serde(default = "default_env")]
    pub default_env: String,
    
    // Whether processes show output in terminal (false) or run silently (true)
    // Note: All processes are detached (survive Ctrl+C), this only controls visibility
    // false = show colored output in terminal, true = silent background
    #[serde(default = "default_detached")]
    pub detached_mode: bool,
    
    // Whether to automatically start missing dependencies without prompting
    // true = auto-start dependencies when needed
    // false = error and require manual start or --skip-deps flag
    #[serde(default = "default_auto_start_deps")]
    pub auto_start_deps: bool,
}

// Helper function called by serde when default_env is missing from JSON
// Returns the default value: "local"
fn default_env() -> String {
    "local".to_string()
}

// Helper function: default is false (show output in terminal)
// This allows users to see what's happening by default
fn default_detached() -> bool {
    false
}

// Helper function: default is true (auto-start dependencies)
fn default_auto_start_deps() -> bool {
    true
}

// Implement the Default trait for Preferences
// This allows creating a Preferences with default values using Preferences::default()
impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_env: "local".to_string(),
            detached_mode: false,
            auto_start_deps: true,
        }
    }
}
