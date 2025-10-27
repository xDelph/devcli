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
// Example: { "local": { "start": "npm start", "test": "npm test" }, "docker": {...} }
// An app can have ANY combination of environments - doesn't need all of them
// Examples:
//   - Only local: { "local": {...} }
//   - Only docker: { "docker": {...} }
//   - Both: { "local": {...}, "docker": {...} }
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
// Example: { "local": "start", "docker": "run" }
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
}

// Helper function called by serde when default_env is missing from JSON
// Returns the default value: "local"
fn default_env() -> String {
    "local".to_string()
}

// Implement the Default trait for Preferences
// This allows creating a Preferences with default values using Preferences::default()
impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_env: "local".to_string(),
        }
    }
}
