// State management for the TUI application
// Manages the current view state, selected items, and application data
// Follows an Elm-like architecture for predictable state updates

use crate::config::models::{App, Config};
use crate::process_manager_support::{find_process, state_store};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::path::PathBuf;

/// The main application state that drives the TUI
/// Contains all data needed to render the interface and handle user interactions
#[derive(Debug, Clone)]
pub struct AppState {
    /// List of projects with their applications
    pub projects: Vec<ProjectState>,
    /// Index of the currently selected project
    pub selected_project_idx: usize,
    /// Index of the currently selected app within the selected project
    pub selected_app_idx: usize,
    /// The current view being displayed
    pub current_view: ViewType,
    /// Optional error message to display to the user
    pub error_message: Option<String>,
    /// Optional status message to display to the user
    pub status_message: Option<String>,
    /// Command execution request (command_idx) - set when user wants to execute a command
    /// The app will process this and show the command popup
    pub command_execution_requested: Option<usize>,
    /// Stop command request - set when user wants to stop an app
    pub stop_requested: bool,
    /// Restart command request - set when user wants to restart an app
    pub restart_requested: bool,
    /// Environment selection request - set when user wants to start with environment choice
    pub env_selection_requested: bool,
    /// Flag to indicate status was updated (for triggering UI redraw)
    pub status_updated: bool,
}

impl AppState {
    /// Creates a new AppState from the configuration
    /// Loads all projects and apps, and queries their running status
    pub fn from_config(config: &Config) -> Result<Self> {
        let store = state_store()?;
        let _ = store.cleanup_dead();
        let mut projects: Vec<ProjectState> = Vec::new();

        // Iterate through each project in the config
        for (project_name, project_config) in &config.projects {
            let mut apps: Vec<AppStateData> = Vec::new();

            // Iterate through each app in the project
            for (app_name, app_config) in &project_config.apps {
                // Determine status and runtime details from process-manager state.
                let process_info = find_process(&store, project_name, app_name, None)?;
                let status = if let Some(info) = &process_info {
                    if store.is_running(info) {
                        AppStatus::Running {
                            pid: info.pid,
                            uptime: Utc::now().signed_duration_since(info.start_time),
                            start_time: info.start_time,
                        }
                    } else {
                        AppStatus::Stopped
                    }
                } else {
                    AppStatus::Stopped
                };

                // Get the active stage from the running process (if any)
                let active_stage = process_info
                    .as_ref()
                    .and_then(|info| info.metadata.get("stage").cloned());

                // Extract restart count and exit code
                let restart_count = process_info
                    .as_ref()
                    .map(|info| info.runtime.restart_count)
                    .unwrap_or(0);
                let last_exit_code = process_info
                    .as_ref()
                    .and_then(|info| info.runtime.last_exit_code);

                // Determine health status
                let health_status = if let Some(info) = process_info.as_ref() {
                    if info.runtime.health_failures > 0 {
                        HealthStatus::Unhealthy {
                            failures: info.runtime.health_failures,
                            last_check: info.runtime.last_health_check.unwrap_or_else(Utc::now),
                        }
                    } else if info.runtime.last_health_check.is_some() {
                        // Health check has run and is passing
                        HealthStatus::Healthy
                    } else {
                        // No health check has run yet
                        HealthStatus::Unknown
                    }
                } else {
                    // No process info available
                    HealthStatus::Unknown
                };

                let app_state = AppStateData {
                    name: app_name.clone(),
                    alternative_name: app_config.alternative_name.clone(),
                    project: project_name.clone(),
                    app_type: app_config.app_type.clone(),
                    status,
                    commands: Self::extract_commands(app_config),
                    path: Some(app_config.path.clone()),
                    // Extract dependency names from Dependency structs
                    // Format as "project/app" for clarity
                    dependencies: app_config
                        .dependencies
                        .iter()
                        .map(|dep| format!("{}/{}", dep.project, dep.app))
                        .collect(),
                    // Stage from configuration (no longer stored in config, use default_stages)
                    stage: None,
                    // Active stage from running process (may differ if overridden)
                    active_stage,
                    // Environment files configuration
                    env_files: app_config.env_files.clone(),
                    // Restart tracking
                    restart_count,
                    last_exit_code,
                    health_status,
                };

                apps.push(app_state);
            }

            // Sort apps by name for consistent display
            apps.sort_by(|a, b| a.name.cmp(&b.name));

            projects.push(ProjectState {
                name: project_name.clone(),
                alternative_name: project_config.alternative_name.clone(),
                apps,
                expanded: true, // Start with all projects expanded
            });
        }

        // Sort projects by name for consistent display
        projects.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Self {
            projects,
            selected_project_idx: 0,
            selected_app_idx: 0,
            current_view: ViewType::Main,
            error_message: None,
            status_message: None,
            command_execution_requested: None,
            stop_requested: false,
            restart_requested: false,
            env_selection_requested: false,
            status_updated: false,
        })
    }

    /// Extracts commands from app configuration
    /// Groups commands by environment (local, docker, orbstack, k8s, etc.)
    fn extract_commands(app: &App) -> HashMap<String, Vec<CommandInfo>> {
        let mut commands = HashMap::new();

        // Extract commands for each environment dynamically from the app's commands
        // Check all possible environment fields
        let env_maps = [
            ("local", &app.commands.local),
            ("docker", &app.commands.docker),
            ("orbstack", &app.commands.orbstack),
            ("k8s", &app.commands.k8s),
        ];

        for (env_key, env_cmds_opt) in env_maps {
            if let Some(env_cmds) = env_cmds_opt {
                let mut cmd_list: Vec<CommandInfo> = env_cmds
                    .iter()
                    .map(|(name, cmd)| CommandInfo {
                        name: name.clone(),
                        command: cmd.clone(),
                    })
                    .collect();
                // Sort commands alphabetically by name
                cmd_list.sort_by(|a, b| a.name.cmp(&b.name));
                if !cmd_list.is_empty() {
                    commands.insert(env_key.to_string(), cmd_list);
                }
            }
        }

        commands
    }

    /// Gets the currently selected project, if any
    pub fn selected_project(&self) -> Option<&ProjectState> {
        self.projects.get(self.selected_project_idx)
    }

    /// Gets the currently selected app, if any
    pub fn selected_app(&self) -> Option<&AppStateData> {
        self.selected_project()
            .and_then(|p| p.apps.get(self.selected_app_idx))
    }

    /// Moves selection to the next item
    pub fn select_next(&mut self) {
        if let Some(project) = self.projects.get(self.selected_project_idx) {
            if self.selected_app_idx < project.apps.len().saturating_sub(1) {
                self.selected_app_idx += 1;
            } else if self.selected_project_idx < self.projects.len().saturating_sub(1) {
                // Move to next project
                self.selected_project_idx += 1;
                self.selected_app_idx = 0;
            }
        }
    }

    /// Moves selection to the previous item
    pub fn select_previous(&mut self) {
        if self.selected_app_idx > 0 {
            self.selected_app_idx -= 1;
        } else if self.selected_project_idx > 0 {
            // Move to previous project
            self.selected_project_idx -= 1;
            if let Some(project) = self.projects.get(self.selected_project_idx) {
                self.selected_app_idx = project.apps.len().saturating_sub(1);
            }
        }
    }

    /// Toggles the expansion state of the currently selected project
    pub fn toggle_project_expansion(&mut self) {
        if let Some(project) = self.projects.get_mut(self.selected_project_idx) {
            project.expanded = !project.expanded;
        }
    }

    /// Sets a command execution request
    /// This signals that the user wants to execute a command at the given index
    pub fn set_command_execution_requested(&mut self, command_idx: usize) {
        self.command_execution_requested = Some(command_idx);
    }

    /// Clears the command execution request
    /// Called after the request has been processed
    pub fn clear_command_execution_request(&mut self) {
        self.command_execution_requested = None;
    }

    /// Sets a stop request
    pub fn set_stop_requested(&mut self) {
        self.stop_requested = true;
    }

    /// Clears the stop request
    pub fn clear_stop_requested(&mut self) {
        self.stop_requested = false;
    }

    /// Sets a restart request
    pub fn set_restart_requested(&mut self) {
        self.restart_requested = true;
    }

    /// Clears the restart request
    pub fn clear_restart_requested(&mut self) {
        self.restart_requested = false;
    }

    /// Sets environment selection request
    pub fn set_env_selection_requested(&mut self) {
        self.env_selection_requested = true;
    }

    /// Clears environment selection request
    pub fn clear_env_selection_requested(&mut self) {
        self.env_selection_requested = false;
    }
}

/// Health status of an application
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    /// Health status is unknown (no health check configured or no data yet)
    Unknown,
    /// Application is healthy (health checks passing)
    Healthy,
    /// Application is unhealthy (health checks failing)
    Unhealthy {
        /// Number of consecutive failures
        failures: u32,
        /// Timestamp of the last health check
        last_check: DateTime<Utc>,
    },
}

impl HealthStatus {
    /// Returns true if the app is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Healthy)
    }

    /// Returns a human-readable status string
    pub fn as_str(&self) -> &str {
        match self {
            HealthStatus::Unknown => "Unknown",
            HealthStatus::Healthy => "Healthy",
            HealthStatus::Unhealthy { .. } => "Unhealthy",
        }
    }
}

/// Represents a project with its applications
#[derive(Debug, Clone)]
pub struct ProjectState {
    /// Name of the project
    pub name: String,
    /// Alternative display name for privacy
    pub alternative_name: Option<String>,
    /// List of applications in this project
    pub apps: Vec<AppStateData>,
    /// Whether the project is expanded in the UI
    pub expanded: bool,
}

/// Represents an individual application's state
#[derive(Debug, Clone)]
pub struct AppStateData {
    /// Name of the application
    pub name: String,
    /// Alternative display name for privacy
    pub alternative_name: Option<String>,
    /// Project this app belongs to
    pub project: String,
    /// Type of application (nodejs, python, etc.)
    pub app_type: String,
    /// Current running status
    pub status: AppStatus,
    /// Available commands grouped by environment
    pub commands: HashMap<String, Vec<CommandInfo>>,
    /// Path to the application directory
    pub path: Option<String>,
    /// List of dependency app names
    pub dependencies: Vec<String>,
    /// Deployment stage (dev, qa, preprod, prod) if configured
    /// This is the stage from the app configuration
    pub stage: Option<String>,
    /// Active stage for running processes
    /// This is the stage that was actually used when starting the process
    /// (may differ from configured stage if overridden with --stage flag)
    pub active_stage: Option<String>,
    /// Environment files configuration
    /// Maps stage -> context -> file path
    /// Example: { "qa": { "local": ".env.qa", "docker": ".env.qa" } }
    pub env_files:
        Option<std::collections::HashMap<String, std::collections::HashMap<String, String>>>,
    /// Number of times this app has been restarted
    pub restart_count: u32,
    /// Exit code from the last time the process exited
    pub last_exit_code: Option<i32>,
    /// Current health status of the application
    pub health_status: HealthStatus,
}

/// Information about a command
#[derive(Debug, Clone)]
pub struct CommandInfo {
    /// Name of the command (e.g., "start", "test")
    pub name: String,
    /// The actual command to execute
    pub command: String,
}

/// Represents the running status of an application
#[derive(Debug, Clone, PartialEq)]
pub enum AppStatus {
    /// App is currently running
    Running {
        /// Process ID
        pid: u32,
        /// How long the app has been running
        uptime: Duration,
        /// When the app was started
        start_time: DateTime<Utc>,
    },
    /// App is not running
    Stopped,
    /// Status cannot be determined
    Unknown,
}

impl AppStatus {
    /// Returns true if the app is running
    pub fn is_running(&self) -> bool {
        matches!(self, AppStatus::Running { .. })
    }

    /// Returns a human-readable status string
    pub fn as_str(&self) -> &str {
        match self {
            AppStatus::Running { .. } => "Running",
            AppStatus::Stopped => "Stopped",
            AppStatus::Unknown => "Unknown",
        }
    }
}

/// Represents the different views in the TUI
#[derive(Debug, Clone, PartialEq)]
pub enum ViewType {
    /// Main view showing projects and apps
    Main,
    /// Command list view for a specific app
    CommandList {
        /// Project name
        project: String,
        /// App name
        app: String,
    },
    /// Log browser view for a specific app
    LogBrowser {
        /// Project name
        project: String,
        /// App name
        app: String,
    },
    /// Log viewer showing contents of specific log files
    LogViewer {
        /// Paths to the log files being viewed
        log_paths: Vec<PathBuf>,
        /// Index of the currently active log panel
        active_index: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_status_is_running() {
        let running = AppStatus::Running {
            pid: 1234,
            uptime: Duration::seconds(60),
            start_time: Utc::now(),
        };
        assert!(running.is_running());

        let stopped = AppStatus::Stopped;
        assert!(!stopped.is_running());
    }

    #[test]
    fn test_app_status_as_str() {
        let running = AppStatus::Running {
            pid: 1234,
            uptime: Duration::seconds(60),
            start_time: Utc::now(),
        };
        assert_eq!(running.as_str(), "Running");

        let stopped = AppStatus::Stopped;
        assert_eq!(stopped.as_str(), "Stopped");
    }

    #[test]
    fn test_view_type_equality() {
        let main1 = ViewType::Main;
        let main2 = ViewType::Main;
        assert_eq!(main1, main2);

        let cmd1 = ViewType::CommandList {
            project: "test".to_string(),
            app: "app1".to_string(),
        };
        let cmd2 = ViewType::CommandList {
            project: "test".to_string(),
            app: "app1".to_string(),
        };
        assert_eq!(cmd1, cmd2);
    }
}
