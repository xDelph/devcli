// Data models for configuration management
// These structs map directly to the JSON structure in ~/.devcli/config.json
//
// serde is a library that automatically converts between Rust structs and JSON
// - Serialize: Convert Rust struct → JSON
// - Deserialize: Convert JSON → Rust struct
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported deployment stages
/// Defines the lifecycle stages for application deployment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Dev,
    Qa,
    Preprod,
    Prod,
}

impl Stage {
    /// Returns all stages in order
    pub const fn all() -> &'static [Stage] {
        &[Stage::Dev, Stage::Qa, Stage::Preprod, Stage::Prod]
    }

    /// Returns the string key used in config JSON
    pub const fn as_str(&self) -> &'static str {
        match self {
            Stage::Dev => "dev",
            Stage::Qa => "qa",
            Stage::Preprod => "preprod",
            Stage::Prod => "prod",
        }
    }

    /// Parse from string key
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "dev" => Some(Stage::Dev),
            "qa" => Some(Stage::Qa),
            "preprod" => Some(Stage::Preprod),
            "prod" => Some(Stage::Prod),
            _ => None,
        }
    }

    /// Returns a comma-separated list of all valid stage names
    /// Used in error messages
    pub fn all_names() -> String {
        Self::all()
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Supported execution environments
/// Defines the order in which environments are displayed in the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Local,
    Docker,
    OrbStack,
    DockerCompose,
    K8s,
    Ci,
}

impl Environment {
    /// Returns all environments in display order
    pub const fn all() -> &'static [Environment] {
        &[
            Environment::Local,
            Environment::Docker,
            Environment::OrbStack,
            Environment::DockerCompose,
            Environment::K8s,
            Environment::Ci,
        ]
    }

    /// Returns the string key used in config JSON
    pub const fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Docker => "docker",
            Environment::OrbStack => "orbstack",
            Environment::DockerCompose => "docker-compose",
            Environment::K8s => "k8s",
            Environment::Ci => "ci",
        }
    }

    /// Returns the display name for UI
    pub const fn display_name(&self) -> &'static str {
        match self {
            Environment::Local => "LOCAL",
            Environment::Docker => "DOCKER",
            Environment::OrbStack => "ORBSTACK",
            Environment::DockerCompose => "COMPOSE",
            Environment::K8s => "K8S",
            Environment::Ci => "CI",
        }
    }

    /// Parse from string key
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "local" => Some(Environment::Local),
            "docker" => Some(Environment::Docker),
            "orbstack" => Some(Environment::OrbStack),
            "docker-compose" | "compose" => Some(Environment::DockerCompose),
            "k8s" | "kubernetes" => Some(Environment::K8s),
            "ci" => Some(Environment::Ci),
            _ => None,
        }
    }

    /// Returns a comma-separated list of all valid environment names
    /// Used in error messages
    pub fn all_names() -> String {
        Self::all()
            .iter()
            .map(|e| e.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

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

    // Alternative display name for privacy in screenshots or logs
    // When present, this name is shown in TUI and config commands
    // OPTIONAL: If not set, the actual project name is used
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alternative_name: Option<String>,
}

// App configuration for a single application
// Contains everything needed to run it: where it is, what commands to use, dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    // The JSON field is "type" but we call it app_type because "type" is a Rust keyword
    // #[serde(rename = "type")] tells serde to map "type" in JSON to app_type in Rust
    #[serde(rename = "type")]
    pub app_type: String, // e.g., "nodejs", "nx", "redis", etc.

    // Alternative display name for privacy in screenshots or logs
    // When present, this name is shown in TUI and can be used in commands
    // OPTIONAL: If not set, the actual app name is used
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alternative_name: Option<String>,

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

    // Path to Dockerfile relative to app path (e.g., "Dockerfile", "docker/Dockerfile")
    // Used to determine which .env file to use (prioritizes .env at Dockerfile level)
    // OPTIONAL: Only set if Dockerfile exists
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dockerfile_path: Option<String>,

    // Environment files mapped by stage and context
    // Structure: { "dev": { "local": ".env.dev", "docker": "docker/.env.dev" }, ... }
    // Allows different env files for different runtime contexts within the same stage
    // OPTIONAL: If not set, auto-detects .env files
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_files: Option<HashMap<String, HashMap<String, String>>>,

    // Default stage to use for each environment when starting the app
    // Structure: { "local": "dev", "docker": "qa", "orbstack": "qa", "k8s": "prod" }
    // Similar to defaults for commands, but for stages
    // OPTIONAL: Falls back to preferences.default_stage or no stage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_stages: Option<HashMap<String, String>>,

    // Health check configuration for monitoring process health
    // Supports HTTP endpoints, TCP ports, custom commands, and process-alive checks
    // OPTIONAL: If not set, only process existence is checked
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_check: Option<HealthCheck>,

    // Restart policy for automatic recovery from failures
    // Controls when and how to restart crashed or unhealthy processes
    // OPTIONAL: If not set, no automatic restarts occur
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,
}

// Commands for different environments
// Example: { "local": { "start": "npm start" }, "docker": {...}, "orbstack": {...}, "k8s": {...} }
// An app can have ANY combination of environments - doesn't need all of them
// Examples:
//   - Only local: { "local": {...} }
//   - Only docker: { "docker": {...} }
//   - All four: { "local": {...}, "docker": {...}, "orbstack": {...}, "k8s": {...} }
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    // OrbStack commands (run in containers with OrbStack context)
    // Key = command name (e.g., "build", "run")
    // Value = docker command with orbstack context (e.g., "docker --context orbstack build -t myapp .")
    // OPTIONAL: Not all apps need orbstack commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orbstack: Option<HashMap<String, String>>,

    // Kubernetes commands (deploy to k8s cluster)
    // Key = command name (e.g., "apply", "delete", "restart")
    // Value = kubectl command (e.g., "kubectl apply -f k8s/")
    // OPTIONAL: Not all apps need k8s commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k8s: Option<HashMap<String, String>>,

    // Docker Compose commands
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "docker-compose")]
    pub docker_compose: Option<HashMap<String, String>>,

    // CI pipeline commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci: Option<HashMap<String, String>>,
}

// Implement Default manually or via derive (we used derive above)
// This allows creating an empty Commands struct easily: Commands::default()
// Useful for tests and initialization

impl Commands {
    /// Create a new empty Commands instance
    /// Wrapper around default() for better ergonomics
    pub fn new() -> Self {
        Self::default()
    }
}

impl Commands {
    /// Get commands for a specific environment
    pub fn get(&self, env: &str) -> Option<&HashMap<String, String>> {
        match env {
            "local" => self.local.as_ref(),
            "docker" => self.docker.as_ref(),
            "orbstack" => self.orbstack.as_ref(),
            "k8s" | "kubernetes" => self.k8s.as_ref(),
            "docker-compose" | "compose" => self.docker_compose.as_ref(),
            "ci" => self.ci.as_ref(),
            _ => None,
        }
    }

    /// Get mutable commands for a specific environment
    pub fn get_mut(&mut self, env: &str) -> Option<&mut HashMap<String, String>> {
        match env {
            "local" => self.local.as_mut(),
            "docker" => self.docker.as_mut(),
            "orbstack" => self.orbstack.as_mut(),
            "k8s" | "kubernetes" => self.k8s.as_mut(),
            "docker-compose" | "compose" => self.docker_compose.as_mut(),
            "ci" => self.ci.as_mut(),
            _ => None,
        }
    }

    /// Returns all environment keys that have commands defined, in a consistent order
    pub fn available_envs(&self) -> Vec<&'static str> {
        let mut envs = Vec::new();
        // Check in a consistent order
        if self.local.is_some() {
            envs.push("local");
        }
        if self.docker.is_some() {
            envs.push("docker");
        }
        if self.orbstack.is_some() {
            envs.push("orbstack");
        }
        if self.docker_compose.is_some() {
            envs.push("docker-compose");
        }
        if self.k8s.is_some() {
            envs.push("k8s");
        }
        if self.ci.is_some() {
            envs.push("ci");
        }
        envs
    }
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
// Example: { "local": "start", "docker": "run", "orbstack": "run", "k8s": "apply" }
// Only needs defaults for environments that have commands defined
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Defaults {
    // Default command for local environment (must exist in commands.local if provided)
    // OPTIONAL: Only needed if app has local commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,

    // Default command for docker environment (must exist in commands.docker if provided)
    // OPTIONAL: Only needed if app has docker commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docker: Option<String>,

    // Default command for orbstack environment (must exist in commands.orbstack if provided)
    // OPTIONAL: Only needed if app has orbstack commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orbstack: Option<String>,

    // Default command for k8s environment (must exist in commands.k8s if provided)
    // OPTIONAL: Only needed if app has k8s commands
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k8s: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none", rename = "docker-compose")]
    pub docker_compose: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci: Option<String>,
}

// Implement Default manually or via derive (we used derive above)
// This allows creating an empty Defaults struct easily: Defaults::default()

impl Defaults {
    /// Create a new empty Defaults instance
    /// Wrapper around default() for better ergonomics
    pub fn new() -> Self {
        Self::default()
    }
}

impl Defaults {
    /// Get default command for a specific environment
    pub fn get(&self, env: &str) -> Option<&String> {
        match env {
            "local" => self.local.as_ref(),
            "docker" => self.docker.as_ref(),
            "orbstack" => self.orbstack.as_ref(),
            "k8s" | "kubernetes" => self.k8s.as_ref(),
            "docker-compose" | "compose" => self.docker_compose.as_ref(),
            "ci" => self.ci.as_ref(),
            _ => None,
        }
    }

    /// Get mutable default command for a specific environment
    pub fn get_mut(&mut self, env: &str) -> &mut Option<String> {
        match env {
            "local" => &mut self.local,
            "docker" => &mut self.docker,
            "orbstack" => &mut self.orbstack,
            "k8s" | "kubernetes" => &mut self.k8s,
            "docker-compose" | "compose" => &mut self.docker_compose,
            "ci" => &mut self.ci,
            _ => &mut self.local, // Fallback (shouldn't happen)
        }
    }
}

impl App {
    /// Get the env file path for a specific stage and environment
    /// Falls back to stage-only or base .env if context-specific not found
    pub fn get_env_file(&self, stage: Option<&str>, env: &str) -> Option<String> {
        // Check env_files structure
        if let Some(ref env_files) = self.env_files {
            if let Some(stage_name) = stage {
                // Try stage + environment specific
                if let Some(stage_map) = env_files.get(stage_name) {
                    if let Some(path) = stage_map.get(env) {
                        return Some(path.clone());
                    }
                    // Try stage with "all" context
                    if let Some(path) = stage_map.get("all") {
                        return Some(path.clone());
                    }
                }
            }
        }

        None
    }

    /// Get the default stage for a specific environment
    /// Falls back to preferences default_stage
    pub fn get_default_stage(
        &self,
        env: &str,
        preferences_default: Option<&str>,
    ) -> Option<String> {
        // Check default_stages structure
        if let Some(ref default_stages) = self.default_stages {
            if let Some(stage) = default_stages.get(env) {
                return Some(stage.clone());
            }
        }

        // Fall back to preferences default
        preferences_default.map(|s| s.to_string())
    }

    /// Set env file for a specific stage and environment
    pub fn set_env_file(&mut self, stage: &str, env: &str, path: String) {
        if self.env_files.is_none() {
            self.env_files = Some(HashMap::new());
        }

        let env_files = self
            .env_files
            .as_mut()
            .expect("env_files should be Some after initialization");
        if !env_files.contains_key(stage) {
            env_files.insert(stage.to_string(), HashMap::new());
        }

        env_files
            .get_mut(stage)
            .expect("stage should exist after insertion")
            .insert(env.to_string(), path);
    }

    /// Set default stage for a specific environment
    pub fn set_default_stage(&mut self, env: &str, stage: String) {
        if self.default_stages.is_none() {
            self.default_stages = Some(HashMap::new());
        }

        self.default_stages
            .as_mut()
            .expect("default_stages should be Some after initialization")
            .insert(env.to_string(), stage);
    }
}

// User preferences stored in ~/.devcli/preferences.json
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

    // Docker platform to use for docker and orbstack commands
    // Default: "linux/amd64" for cross-platform compatibility
    // Can be set to "linux/arm64" for ARM-based systems
    #[serde(default = "default_docker_platform")]
    pub docker_platform: String,

    // Default deployment stage to use when starting apps
    // Can be any string (dev, qa, preprod, prod, staging, etc.)
    // OPTIONAL: If not set, no default stage is applied
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_stage: Option<String>,
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

// Helper function: default docker platform is linux/amd64
// This ensures compatibility across different architectures
fn default_docker_platform() -> String {
    "linux/amd64".to_string()
}

// Implement the Default trait for Preferences
// This allows creating a Preferences with default values using Preferences::default()
impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_env: "local".to_string(),
            detached_mode: false,
            auto_start_deps: true,
            docker_platform: "linux/amd64".to_string(),
            default_stage: None,
        }
    }
}

// Health check configuration
// Tagged enum that serializes with a "type" field to distinguish variants
// Example JSON: { "type": "http", "url": "http://localhost:3000/health", ... }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HealthCheck {
    // HTTP health check - makes GET request to URL and checks status code
    Http {
        url: String,
        #[serde(default = "default_http_timeout")]
        timeout_secs: u64,
        #[serde(default = "default_http_status")]
        expected_status: u16,
    },
    // TCP health check - attempts to connect to host:port
    Tcp {
        host: String,
        port: u16,
        #[serde(default = "default_tcp_timeout")]
        timeout_secs: u64,
    },
    // Command health check - runs shell command and checks exit code
    Command {
        command: String,
        #[serde(default = "default_command_timeout")]
        timeout_secs: u64,
        #[serde(default = "default_command_exit_code")]
        expected_exit_code: i32,
    },
    // Process health check - just checks if process is alive (default behavior)
    Process {},
}

// Helper functions for HealthCheck defaults
fn default_http_timeout() -> u64 {
    5
}

fn default_http_status() -> u16 {
    200
}

fn default_tcp_timeout() -> u64 {
    5
}

fn default_command_timeout() -> u64 {
    5
}

fn default_command_exit_code() -> i32 {
    0
}

impl HealthCheck {
    /// Validate health check configuration
    pub fn validate(&self) -> Result<(), String> {
        match self {
            HealthCheck::Http {
                url, timeout_secs, ..
            } => {
                if url.is_empty() {
                    return Err("HTTP health check URL cannot be empty".to_string());
                }
                if *timeout_secs == 0 {
                    return Err("HTTP health check timeout must be greater than 0".to_string());
                }
            }
            HealthCheck::Tcp {
                host,
                port,
                timeout_secs,
            } => {
                if host.is_empty() {
                    return Err("TCP health check host cannot be empty".to_string());
                }
                if *port == 0 {
                    return Err("TCP health check port must be greater than 0".to_string());
                }
                if *timeout_secs == 0 {
                    return Err("TCP health check timeout must be greater than 0".to_string());
                }
            }
            HealthCheck::Command {
                command,
                timeout_secs,
                ..
            } => {
                if command.is_empty() {
                    return Err("Command health check command cannot be empty".to_string());
                }
                if *timeout_secs == 0 {
                    return Err("Command health check timeout must be greater than 0".to_string());
                }
            }
            HealthCheck::Process {} => {
                // Process health check has no configuration to validate
            }
        }
        Ok(())
    }
}

// Restart policy configuration
// Controls automatic restart behavior for crashed or unhealthy processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartPolicy {
    // Whether automatic restart is enabled
    #[serde(default = "default_restart_enabled")]
    pub enabled: bool,

    // Maximum number of restarts within the window (0 = unlimited)
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,

    // Time window in seconds for counting restarts
    #[serde(default = "default_restart_window")]
    pub restart_window_secs: u64,

    // Initial backoff delay in seconds before first restart
    #[serde(default = "default_initial_backoff")]
    pub initial_backoff_secs: u64,

    // Maximum backoff delay in seconds (caps exponential growth)
    #[serde(default = "default_max_backoff")]
    pub max_backoff_secs: u64,

    // Multiplier for exponential backoff (each retry multiplies delay by this)
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,

    // Exit codes that trigger restart (None = any non-zero code)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart_on_exit_codes: Option<Vec<i32>>,
}

// Helper functions for RestartPolicy defaults
fn default_restart_enabled() -> bool {
    true
}

fn default_max_restarts() -> u32 {
    5
}

fn default_restart_window() -> u64 {
    300 // 5 minutes
}

fn default_initial_backoff() -> u64 {
    1
}

fn default_max_backoff() -> u64 {
    60
}

fn default_backoff_multiplier() -> f64 {
    2.0
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_restarts: 5,
            restart_window_secs: 300,
            initial_backoff_secs: 1,
            max_backoff_secs: 60,
            backoff_multiplier: 2.0,
            restart_on_exit_codes: None,
        }
    }
}

impl RestartPolicy {
    /// Validate restart policy configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.restart_window_secs == 0 {
            return Err("Restart window must be greater than 0".to_string());
        }
        if self.initial_backoff_secs == 0 {
            return Err("Initial backoff must be greater than 0".to_string());
        }
        if self.max_backoff_secs < self.initial_backoff_secs {
            return Err("Max backoff must be >= initial backoff".to_string());
        }
        if self.backoff_multiplier <= 0.0 {
            return Err("Backoff multiplier must be greater than 0".to_string());
        }
        Ok(())
    }
}
