// Test utilities and mock data generators
// Provides builder patterns for creating test fixtures
// This centralizes test data creation and makes tests easier to maintain

use crate::config::models::*;
use std::collections::HashMap;

/// Builder for creating App test fixtures
/// Provides sensible defaults and allows overriding specific fields
/// 
/// # Example
/// ```
/// use rustycli_core::test_utils::AppBuilder;
/// 
/// let app = AppBuilder::new("nodejs", "/tmp/app")
///     .with_stage("qa")
///     .with_local_command("start", "npm start")
///     .build();
/// ```
pub struct AppBuilder {
    app_type: String,
    path: String,
    local_commands: HashMap<String, String>,
    docker_commands: HashMap<String, String>,
    orbstack_commands: HashMap<String, String>,
    k8s_commands: HashMap<String, String>,
    dependencies: Vec<Dependency>,
    local_default: Option<String>,
    docker_default: Option<String>,
    orbstack_default: Option<String>,
    k8s_default: Option<String>,
    dockerfile_path: Option<String>,
}

impl AppBuilder {
    /// Create a new AppBuilder with required fields
    pub fn new(app_type: &str, path: &str) -> Self {
        Self {
            app_type: app_type.to_string(),
            path: path.to_string(),
            local_commands: HashMap::new(),
            docker_commands: HashMap::new(),
            orbstack_commands: HashMap::new(),
            k8s_commands: HashMap::new(),
            dependencies: Vec::new(),
            local_default: None,
            docker_default: None,
            orbstack_default: None,
            k8s_default: None,
            dockerfile_path: None,
        }
    }

    /// Add a local command
    pub fn with_local_command(mut self, name: &str, command: &str) -> Self {
        self.local_commands.insert(name.to_string(), command.to_string());
        self
    }

    /// Add a docker command
    pub fn with_docker_command(mut self, name: &str, command: &str) -> Self {
        self.docker_commands.insert(name.to_string(), command.to_string());
        self
    }

    /// Add an orbstack command
    pub fn with_orbstack_command(mut self, name: &str, command: &str) -> Self {
        self.orbstack_commands.insert(name.to_string(), command.to_string());
        self
    }

    /// Add a k8s command
    pub fn with_k8s_command(mut self, name: &str, command: &str) -> Self {
        self.k8s_commands.insert(name.to_string(), command.to_string());
        self
    }

    /// Add a dependency
    pub fn with_dependency(mut self, project: &str, app: &str) -> Self {
        self.dependencies.push(Dependency {
            project: project.to_string(),
            app: app.to_string(),
        });
        self
    }

    /// Set local default command
    pub fn with_local_default(mut self, default: &str) -> Self {
        self.local_default = Some(default.to_string());
        self
    }

    /// Set docker default command
    pub fn with_docker_default(mut self, default: &str) -> Self {
        self.docker_default = Some(default.to_string());
        self
    }

    /// Set orbstack default command
    pub fn with_orbstack_default(mut self, default: &str) -> Self {
        self.orbstack_default = Some(default.to_string());
        self
    }

    /// Set k8s default command
    pub fn with_k8s_default(mut self, default: &str) -> Self {
        self.k8s_default = Some(default.to_string());
        self
    }

    /// Set dockerfile path
    pub fn with_dockerfile_path(mut self, path: &str) -> Self {
        self.dockerfile_path = Some(path.to_string());
        self
    }

    /// Build the App
    pub fn build(self) -> App {
        App {
            app_type: self.app_type,
            path: self.path,
            commands: Commands {
                local: if self.local_commands.is_empty() {
                    None
                } else {
                    Some(self.local_commands)
                },
                docker: if self.docker_commands.is_empty() {
                    None
                } else {
                    Some(self.docker_commands)
                },
                orbstack: if self.orbstack_commands.is_empty() {
                    None
                } else {
                    Some(self.orbstack_commands)
                },
                k8s: if self.k8s_commands.is_empty() {
                    None
                } else {
                    Some(self.k8s_commands)
                },
            },
            dependencies: self.dependencies,
            defaults: Defaults {
                local: self.local_default,
                docker: self.docker_default,
                orbstack: self.orbstack_default,
                k8s: self.k8s_default,
            },
            dockerfile_path: self.dockerfile_path,
            env_files: None, // Test utils don't set env_files by default
            default_stages: None, // Test utils don't set default_stages by default
        }
    }
}

/// Builder for creating Config test fixtures
/// 
/// # Example
/// ```
/// use rustycli_core::test_utils::{ConfigBuilder, AppBuilder};
/// 
/// let config = ConfigBuilder::new()
///     .with_app("my-project", "api", AppBuilder::new("nodejs", "/tmp/api")
///         .with_local_command("start", "npm start")
///         .build())
///     .build();
/// ```
pub struct ConfigBuilder {
    projects: HashMap<String, HashMap<String, App>>,
}

impl ConfigBuilder {
    /// Create a new ConfigBuilder
    pub fn new() -> Self {
        Self {
            projects: HashMap::new(),
        }
    }

    /// Add an app to a project
    pub fn with_app(mut self, project: &str, app_name: &str, app: App) -> Self {
        self.projects
            .entry(project.to_string())
            .or_default()
            .insert(app_name.to_string(), app);
        self
    }

    /// Build the Config
    pub fn build(self) -> Config {
        Config {
            projects: self
                .projects
                .into_iter()
                .map(|(name, apps)| (name, Project { apps }))
                .collect(),
        }
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating Preferences test fixtures
/// 
/// # Example
/// ```
/// use rustycli_core::test_utils::PreferencesBuilder;
/// 
/// let prefs = PreferencesBuilder::new()
///     .with_default_env("docker")
///     .with_default_stage("qa")
///     .build();
/// ```
pub struct PreferencesBuilder {
    default_env: String,
    detached_mode: bool,
    auto_start_deps: bool,
    docker_platform: String,
    default_stage: Option<String>,
}

impl PreferencesBuilder {
    /// Create a new PreferencesBuilder with defaults
    pub fn new() -> Self {
        Self {
            default_env: "local".to_string(),
            detached_mode: false,
            auto_start_deps: true,
            docker_platform: "linux/amd64".to_string(),
            default_stage: None,
        }
    }

    /// Set default environment
    pub fn with_default_env(mut self, env: &str) -> Self {
        self.default_env = env.to_string();
        self
    }

    /// Set detached mode
    pub fn with_detached_mode(mut self, detached: bool) -> Self {
        self.detached_mode = detached;
        self
    }

    /// Set auto start deps
    pub fn with_auto_start_deps(mut self, auto_start: bool) -> Self {
        self.auto_start_deps = auto_start;
        self
    }

    /// Set docker platform
    pub fn with_docker_platform(mut self, platform: &str) -> Self {
        self.docker_platform = platform.to_string();
        self
    }

    /// Set default stage
    pub fn with_default_stage(mut self, stage: &str) -> Self {
        self.default_stage = Some(stage.to_string());
        self
    }

    /// Build the Preferences
    pub fn build(self) -> Preferences {
        Preferences {
            default_env: self.default_env,
            detached_mode: self.detached_mode,
            auto_start_deps: self.auto_start_deps,
            docker_platform: self.docker_platform,
            default_stage: self.default_stage,
        }
    }
}

impl Default for PreferencesBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick helper to create a simple nodejs app for testing
pub fn mock_nodejs_app(path: &str) -> App {
    AppBuilder::new("nodejs", path)
        .with_local_command("start", "npm start")
        .with_local_default("start")
        .build()
}

/// Quick helper to create a simple redis app for testing
pub fn mock_redis_app(path: &str) -> App {
    AppBuilder::new("redis", path)
        .with_local_command("start", "redis-server")
        .with_local_default("start")
        .build()
}

/// Quick helper to create an app with dependencies
pub fn mock_app_with_deps(path: &str, deps: Vec<(&str, &str)>) -> App {
    let mut builder = AppBuilder::new("nodejs", path)
        .with_local_command("start", "npm start")
        .with_local_default("start");
    
    for (project, app) in deps {
        builder = builder.with_dependency(project, app);
    }
    
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_builder_minimal() {
        let app = AppBuilder::new("nodejs", "/tmp/app").build();
        
        assert_eq!(app.app_type, "nodejs");
        assert_eq!(app.path, "/tmp/app");
        assert!(app.commands.local.is_none());
        assert!(app.dependencies.is_empty());
        assert!(app.env_files.is_none());
    }

    #[test]
    fn test_app_builder_with_commands() {
        let app = AppBuilder::new("nodejs", "/tmp/app")
            .with_local_command("start", "npm start")
            .with_local_command("test", "npm test")
            .with_local_default("start")
            .build();
        
        assert!(app.commands.local.is_some());
        let local = app.commands.local.unwrap();
        assert_eq!(local.get("start").unwrap(), "npm start");
        assert_eq!(local.get("test").unwrap(), "npm test");
        assert_eq!(app.defaults.local.unwrap(), "start");
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .with_app("project1", "app1", mock_nodejs_app("/tmp/app1"))
            .with_app("project1", "app2", mock_redis_app("/tmp/app2"))
            .build();
        
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects.get("project1").unwrap().apps.len(), 2);
    }

    #[test]
    fn test_preferences_builder() {
        let prefs = PreferencesBuilder::new()
            .with_default_env("docker")
            .with_default_stage("qa")
            .with_detached_mode(true)
            .build();
        
        assert_eq!(prefs.default_env, "docker");
        assert_eq!(prefs.default_stage.unwrap(), "qa");
        assert!(prefs.detached_mode);
    }

    #[test]
    fn test_mock_helpers() {
        let app = mock_nodejs_app("/tmp/app");
        assert_eq!(app.app_type, "nodejs");
        
        let app = mock_redis_app("/tmp/redis");
        assert_eq!(app.app_type, "redis");
        
        let app = mock_app_with_deps("/tmp/app", vec![("project", "dep1"), ("project", "dep2")]);
        assert_eq!(app.dependencies.len(), 2);
    }
}
