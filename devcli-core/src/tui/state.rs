// State management for the TUI application
// Manages the current view state, selected items, and application data
// Follows an Elm-like architecture for predictable state updates

use crate::config::models::{App, Config};
use crate::process_manager_support::{find_process, state_store};
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use process_manager::StateStore;
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
    /// Navigation stack: pushed when entering a sub-view so the back (Esc)
    /// action can return to the exact previous view instead of always Main.
    pub view_history: Vec<ViewType>,
    /// Optional error message to display to the user
    pub error_message: Option<String>,
    /// Optional status message to display to the user
    pub status_message: Option<String>,
    /// Command execution request: the app it was made for and the command's
    /// index in that app's command list. The app processes it and shows the
    /// command popup.
    ///
    /// Every request names its app: they are handled on the next tick, and
    /// the selection can move in between (a worktree row vanishing, a config
    /// refresh), which used to send the action to whatever app was selected
    /// by then.
    pub command_execution_requested: Option<(AppKey, usize)>,
    /// Stop request for an app
    pub stop_requested: Option<AppKey>,
    /// Restart request for an app
    pub restart_requested: Option<AppKey>,
    /// Start request (with environment choice) for an app
    pub env_selection_requested: Option<AppKey>,
    /// Flag to indicate status was updated (for triggering UI redraw)
    pub status_updated: bool,
}

impl AppState {
    /// Creates a new AppState from the configuration
    /// Loads all projects and apps, and queries their running status
    pub fn from_config(config: &Config) -> Result<Self> {
        let store = state_store()?;
        Self::from_config_with_store(config, &store)
    }

    /// Same as [`from_config`], but against an explicit process state store.
    ///
    /// This constructor only *reads* the store. It deliberately does not call
    /// `cleanup_dead()`: that deletes entries for dead processes, and doing it
    /// here made a plain read path mutate (and delete from) the real state
    /// directory — including from unit tests, which made them race each other
    /// and could purge a user's own process records on `cargo test`.
    /// Callers that want the cleanup do it explicitly; the TUI already does,
    /// just before building the state.
    pub fn from_config_with_store(config: &Config, store: &StateStore) -> Result<Self> {
        Self::build(config, |project, app| {
            let process = find_process(store, project, app, None)?;
            Ok(RuntimeSnapshot::from_process(store, process.as_ref()))
        })
    }

    /// Re-read the app list after a config edit, keeping what the user sees.
    ///
    /// The config editor used to replace the whole state with a fresh one:
    /// collapsed projects expanded again, view history and messages were lost,
    /// the selection jumped when apps were added or removed (it was kept by
    /// index), and every process was probed again while the UI lock was held.
    /// Now only the app list is rebuilt; runtime data of apps that still exist
    /// is carried over (new apps get theirs at the next status poll), and the
    /// selection follows the selected app by name.
    ///
    /// Worktree rows (`proj@slug`) are not in the config: they are kept, with
    /// their project's new app list. They used to disappear until the next
    /// status poll, and the selection fell on another app meanwhile.
    pub fn refresh_from_config(&mut self, config: &Config) -> Result<()> {
        let fresh = Self::build(config, |_, _| Ok(RuntimeSnapshot::stopped()))?;
        let selected = self.selected_key();

        let mut projects = fresh.projects;
        for project in &mut projects {
            let Some(old_project) = self.projects.iter().find(|p| p.name == project.name) else {
                continue;
            };
            project.expanded = old_project.expanded;
            for app in &mut project.apps {
                if let Some(old) = old_project.apps.iter().find(|a| a.name == app.name) {
                    app.carry_runtime_from(old);
                }
            }
        }

        // The worktree rows, after their (still configured) project.
        for old in self.projects.iter().filter(|p| p.worktree.is_some()) {
            let Some(base_idx) = projects
                .iter()
                .position(|p| p.worktree.is_none() && Some(p.name.as_str()) == old.base_project())
            else {
                continue;
            };
            let listed: Vec<&str> = old.apps.iter().map(|a| a.name.as_str()).collect();
            let mut row = worktree_row(
                &projects[base_idx],
                &old.name,
                old.worktree.as_deref().unwrap_or_default(),
                |name| listed.contains(&name),
            );
            row.expanded = old.expanded;
            for app in &mut row.apps {
                if let Some(previous) = old.apps.iter().find(|a| a.name == app.name) {
                    app.carry_runtime_from(previous);
                    app.path = previous.path.clone();
                }
            }
            insert_worktree_row(&mut projects, base_idx, row);
        }

        self.projects = projects;
        self.restore_selection(selected);
        Ok(())
    }

    /// Select `selected` again after the project list changed.
    ///
    /// When it is gone, the selection stays close and predictable: the app at
    /// the same place in its project, or - its project gone too - the same
    /// app of the configured project (for a worktree row), else the project
    /// now at that place. It used to fall back to the old raw indices, which
    /// then designated some other app.
    fn restore_selection(&mut self, selected: Option<AppKey>) {
        if let Some(key) = &selected
            && self.select_app(key)
        {
            return;
        }
        let old_project_idx = self.selected_project_idx;
        let old_app_idx = self.selected_app_idx;
        let project_idx = selected.as_ref().and_then(|key| {
            self.projects
                .iter()
                .position(|p| p.name == key.project)
                .or_else(|| {
                    // A worktree row that vanished: its configured project.
                    let base = crate::worktrees::split(&key.project).0;
                    (base != key.project)
                        .then(|| {
                            self.projects
                                .iter()
                                .position(|p| p.worktree.is_none() && p.name == base)
                        })
                        .flatten()
                })
        });
        let (project_idx, app_idx) = match (project_idx, &selected) {
            (Some(idx), Some(key)) => {
                let apps = &self.projects[idx].apps;
                let app_idx = apps
                    .iter()
                    .position(|a| a.name == key.app)
                    .unwrap_or_else(|| {
                        if self.projects[idx].name == key.project {
                            // Same project: the app's neighbour.
                            old_app_idx.min(apps.len().saturating_sub(1))
                        } else {
                            0
                        }
                    });
                (idx, app_idx)
            }
            _ => (
                old_project_idx.min(self.projects.len().saturating_sub(1)),
                0,
            ),
        };
        self.selected_project_idx = project_idx;
        self.selected_app_idx = app_idx;
        self.status_updated = true;
    }

    /// List the git worktree instances devcli tracks (`proj@slug`) as
    /// projects of their own, right after their configured project, while
    /// any of their processes is tracked. Returns whether the list changed.
    ///
    /// They carry the configured project's apps that live in the worktree's
    /// repository (see [`TrackedInstance::repo_apps`]), so any of them can be
    /// started in that worktree from here; actions name the instance exactly
    /// (`proj@slug`).
    pub fn sync_worktree_projects(&mut self, tracked: &[TrackedInstance]) -> bool {
        let before: Vec<String> = self.projects.iter().map(|p| p.name.clone()).collect();
        let selected = self.selected_key();

        self.projects
            .retain(|p| p.worktree.is_none() || tracked.iter().any(|t| t.project == p.name));

        for t in tracked {
            if let Some(existing) = self.projects.iter_mut().find(|p| p.name == t.project) {
                if let Some(app) = existing.apps.iter_mut().find(|a| a.name == t.app) {
                    app.path = Some(t.path.clone());
                }
                continue;
            }
            let Some(base_idx) = self
                .projects
                .iter()
                .position(|p| p.worktree.is_none() && p.name == t.base_project)
            else {
                continue;
            };
            let mut row = worktree_row(&self.projects[base_idx], &t.project, &t.slug, |name| {
                name == t.app
                    || t.repo_apps
                        .as_ref()
                        .is_none_or(|apps| apps.iter().any(|a| a == name))
            });
            if let Some(app) = row.apps.iter_mut().find(|a| a.name == t.app) {
                // Known once it has run in this worktree.
                app.path = Some(t.path.clone());
            }
            insert_worktree_row(&mut self.projects, base_idx, row);
        }

        let changed = self.projects.iter().map(|p| &p.name).ne(before.iter());
        if changed {
            self.restore_selection(selected);
        }
        changed
    }

    /// Build the projects/apps from `config`, asking `runtime` for each app's
    /// runtime data.
    fn build(
        config: &Config,
        mut runtime: impl FnMut(&str, &str) -> Result<RuntimeSnapshot>,
    ) -> Result<Self> {
        let mut projects: Vec<ProjectState> = Vec::new();

        // Iterate through each project in the config
        for (project_name, project_config) in &config.projects {
            let mut apps: Vec<AppStateData> = Vec::new();

            // Iterate through each app in the project
            for (app_name, app_config) in &project_config.apps {
                // Status and runtime details (from process-manager state).
                let RuntimeSnapshot {
                    status,
                    active_stage,
                    restart_count,
                    last_exit_code,
                    health_status,
                } = runtime(project_name, app_name)?;

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
                worktree: None,
            });
        }

        // Sort projects by name for consistent display
        projects.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Self {
            projects,
            selected_project_idx: 0,
            selected_app_idx: 0,
            current_view: ViewType::Main,
            view_history: Vec::new(),
            error_message: None,
            status_message: None,
            command_execution_requested: None,
            stop_requested: None,
            restart_requested: None,
            env_selection_requested: None,
            status_updated: false,
        })
    }

    /// Extracts commands from app configuration
    /// Groups commands by environment (local, docker, orbstack, k8s, etc.)
    fn extract_commands(app: &App) -> HashMap<String, Vec<CommandInfo>> {
        let mut commands = HashMap::new();

        // Every environment: docker-compose and ci commands used to be left
        // out, so they could not be run from the TUI at all.
        for env in crate::config::models::Environment::all() {
            let env_key = env.as_str();
            if let Some(env_cmds) = app.commands.get(env_key) {
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
    ///
    /// A collapsed project is a single row: its apps are skipped (they used to
    /// be walked through invisibly, so the cursor seemed stuck).
    pub fn select_next(&mut self) {
        if let Some(project) = self.projects.get(self.selected_project_idx) {
            let last_app = project.apps.len().saturating_sub(1);
            if project.expanded && self.selected_app_idx < last_app {
                self.selected_app_idx += 1;
            } else if self.selected_project_idx < self.projects.len().saturating_sub(1) {
                // Move to next project
                self.selected_project_idx += 1;
                self.selected_app_idx = 0;
            }
        }
    }

    /// Moves selection to the previous item (collapsed projects: one row)
    pub fn select_previous(&mut self) {
        let expanded = self
            .projects
            .get(self.selected_project_idx)
            .is_some_and(|p| p.expanded);
        if expanded && self.selected_app_idx > 0 {
            self.selected_app_idx -= 1;
        } else if self.selected_project_idx > 0 {
            // Move to previous project: its last app, or its row if collapsed
            self.selected_project_idx -= 1;
            if let Some(project) = self.projects.get(self.selected_project_idx) {
                self.selected_app_idx = if project.expanded {
                    project.apps.len().saturating_sub(1)
                } else {
                    0
                };
            }
        }
    }

    /// Toggles the expansion state of the currently selected project
    pub fn toggle_project_expansion(&mut self) {
        if let Some(project) = self.projects.get_mut(self.selected_project_idx) {
            project.expanded = !project.expanded;
            if !project.expanded {
                // The selection collapses onto the project row.
                self.selected_app_idx = 0;
            }
        }
    }

    /// The identity of the selected app, if any.
    pub fn selected_key(&self) -> Option<AppKey> {
        self.selected_app().map(AppKey::of)
    }

    /// The app `key` designates, wherever it is listed.
    pub fn find_app(&self, key: &AppKey) -> Option<&AppStateData> {
        self.projects
            .iter()
            .find(|p| p.name == key.project)
            .and_then(|p| p.apps.iter().find(|a| a.name == key.app))
    }

    /// Select the app `key` designates. Returns whether it is listed.
    ///
    /// Its project is expanded when the app is not its first row.
    pub fn select_app(&mut self, key: &AppKey) -> bool {
        let Some((project_idx, app_idx)) = self.locate(key) else {
            return false;
        };
        self.selected_project_idx = project_idx;
        self.selected_app_idx = app_idx;
        if app_idx > 0 {
            self.projects[project_idx].expanded = true;
        }
        self.status_updated = true;
        true
    }

    /// `(project index, app index)` of the app `key` designates.
    fn locate(&self, key: &AppKey) -> Option<(usize, usize)> {
        let project_idx = self.projects.iter().position(|p| p.name == key.project)?;
        let app_idx = self.projects[project_idx]
            .apps
            .iter()
            .position(|a| a.name == key.app)?;
        Some((project_idx, app_idx))
    }

    /// Asks to run command `command_idx` of the selected app.
    pub fn set_command_execution_requested(&mut self, command_idx: usize) {
        self.command_execution_requested = self.selected_key().map(|key| (key, command_idx));
    }

    /// Clears the command execution request
    /// Called after the request has been processed
    pub fn clear_command_execution_request(&mut self) {
        self.command_execution_requested = None;
    }

    /// Asks to stop the selected app.
    pub fn set_stop_requested(&mut self) {
        self.stop_requested = self.selected_key();
    }

    /// Clears the stop request
    pub fn clear_stop_requested(&mut self) {
        self.stop_requested = None;
    }

    /// Asks to restart the selected app.
    pub fn set_restart_requested(&mut self) {
        self.restart_requested = self.selected_key();
    }

    /// Clears the restart request
    pub fn clear_restart_requested(&mut self) {
        self.restart_requested = None;
    }

    /// Asks to start the selected app (the popup offers the environment).
    pub fn set_env_selection_requested(&mut self) {
        self.env_selection_requested = self.selected_key();
    }

    /// Clears environment selection request
    pub fn clear_env_selection_requested(&mut self) {
        self.env_selection_requested = None;
    }
}

/// Which app a row is: its project as listed (`proj`, or `proj@slug` for a
/// worktree instance) and its name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppKey {
    pub project: String,
    pub app: String,
}

impl AppKey {
    pub fn new(project: impl Into<String>, app: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            app: app.into(),
        }
    }

    pub fn of(app: &AppStateData) -> Self {
        Self::new(app.project.clone(), app.name.clone())
    }
}

/// Everything about an app that changes while it runs, read from its
/// process-manager entry. Shared by the initial state and the status poller,
/// which used to refresh only `status` and left restarts, exit code, health
/// and active stage frozen at their values when the TUI started.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeSnapshot {
    pub status: AppStatus,
    pub active_stage: Option<String>,
    pub restart_count: u32,
    pub last_exit_code: Option<i32>,
    pub health_status: HealthStatus,
}

impl RuntimeSnapshot {
    /// Runtime data of an app with no process.
    pub fn stopped() -> Self {
        Self {
            status: AppStatus::Stopped,
            active_stage: None,
            restart_count: 0,
            last_exit_code: None,
            health_status: HealthStatus::Unknown,
        }
    }

    pub fn from_process(
        store: &StateStore,
        process: Option<&process_manager::state::ManagedProcess>,
    ) -> Self {
        let Some(info) = process else {
            return Self::stopped();
        };

        let health_status = if info.runtime.health_failures > 0 {
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
        };

        Self {
            status: AppStatus::from_process(store, info),
            active_stage: info.metadata.get("stage").cloned(),
            restart_count: info.runtime.restart_count,
            last_exit_code: info.runtime.last_exit_code,
            health_status,
        }
    }

    /// Equal apart from the running time, which differs on every poll and
    /// would otherwise make each poll look like a change.
    pub fn same_state(&self, other: &Self) -> bool {
        self.status.same_state(&other.status)
            && self.active_stage == other.active_stage
            && self.restart_count == other.restart_count
            && self.last_exit_code == other.last_exit_code
            && self.health_status == other.health_status
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
    /// Set for a git worktree instance of a configured project
    /// (`proj@slug`): listed while devcli tracks one of its processes.
    pub worktree: Option<String>,
}

impl ProjectState {
    /// The configured project of a worktree row (`proj` for `proj@slug`).
    pub fn base_project(&self) -> Option<&str> {
        let slug = self.worktree.as_deref()?;
        self.name.strip_suffix(slug)?.strip_suffix('@')
    }
}

/// The row of worktree instance `name` (`proj@slug`) of configured project
/// `base`, listing the apps of `base` that `keep` accepts, all stopped.
fn worktree_row(
    base: &ProjectState,
    name: &str,
    slug: &str,
    keep: impl Fn(&str) -> bool,
) -> ProjectState {
    let apps = base
        .apps
        .iter()
        .filter(|app| keep(&app.name))
        .cloned()
        .map(|mut app| {
            app.project = name.to_string();
            app.carry_runtime_from_snapshot(RuntimeSnapshot::stopped());
            // Known once it has run in this worktree.
            app.path = None;
            app
        })
        .collect();
    ProjectState {
        name: name.to_string(),
        alternative_name: base
            .alternative_name
            .as_ref()
            .map(|a| format!("{a}@{slug}")),
        apps,
        expanded: true,
        worktree: Some(slug.to_string()),
    }
}

/// Insert worktree `row` after its configured project at `base_idx`, among
/// that project's other worktree rows in name order.
fn insert_worktree_row(projects: &mut Vec<ProjectState>, base_idx: usize, row: ProjectState) {
    let mut at = base_idx + 1;
    while projects
        .get(at)
        .is_some_and(|p| p.worktree.is_some() && p.name < row.name)
    {
        at += 1;
    }
    projects.insert(at, row);
}

/// A git worktree instance of an app that devcli tracks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedInstance {
    /// `proj@slug`.
    pub project: String,
    /// `proj`.
    pub base_project: String,
    pub slug: String,
    pub app: String,
    /// Where it runs.
    pub path: String,
    /// The apps of `base_project` that live in the same git repository as
    /// this instance, which its row lists: an app of another repository has
    /// nothing to do with this worktree (its slug means nothing there).
    /// `None` when not worked out (the row already exists): every app.
    pub repo_apps: Option<Vec<String>>,
}

impl TrackedInstance {
    /// The apps of configured project `base_project` living in the git
    /// repository of `path` (always including `app`, which runs there).
    ///
    /// Runs git: call it without holding the state lock.
    pub fn apps_in_repository(
        config: &Config,
        base_project: &str,
        app: &str,
        path: &str,
    ) -> Vec<String> {
        use crate::worktrees::git::checkout_of;

        let here = checkout_of(std::path::Path::new(path)).map(|c| c.common_dir);
        let Some(project) = config.projects.get(base_project) else {
            return vec![app.to_string()];
        };
        project
            .apps
            .iter()
            .filter(|(name, config)| {
                *name == app
                    || here.as_ref().is_some_and(|here| {
                        checkout_of(&crate::utils::path::expand_path(&config.path))
                            .is_some_and(|c| &c.common_dir == here)
                    })
            })
            .map(|(name, _)| name.clone())
            .collect()
    }
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

impl AppStateData {
    /// Take the runtime data (status, restarts, health...) of `old`.
    fn carry_runtime_from(&mut self, old: &AppStateData) {
        self.status = old.status.clone();
        self.active_stage = old.active_stage.clone();
        self.restart_count = old.restart_count;
        self.last_exit_code = old.last_exit_code;
        self.health_status = old.health_status.clone();
    }

    fn carry_runtime_from_snapshot(&mut self, snapshot: RuntimeSnapshot) {
        self.status = snapshot.status;
        self.active_stage = snapshot.active_stage;
        self.restart_count = snapshot.restart_count;
        self.last_exit_code = snapshot.last_exit_code;
        self.health_status = snapshot.health_status;
    }
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
        /// Environment the process was started in (local, docker, …)
        environment: Option<String>,
        /// Command variant last used to start/run (e.g. start, test)
        command_variant: Option<String>,
    },
    /// App is not running
    Stopped,
    /// Status cannot be determined
    Unknown,
}

impl AppStatus {
    /// Build status from a managed process entry.
    pub fn from_process(
        store: &process_manager::StateStore,
        process: &process_manager::state::ManagedProcess,
    ) -> Self {
        if !store.is_running(process) {
            return AppStatus::Stopped;
        }

        AppStatus::Running {
            pid: process.pid,
            uptime: Utc::now().signed_duration_since(process.start_time),
            start_time: process.start_time,
            environment: process.metadata.get("environment").cloned(),
            command_variant: process.metadata.get("command_variant").cloned(),
        }
    }

    /// Same status, ignoring `uptime`.
    pub fn same_state(&self, other: &Self) -> bool {
        match (self, other) {
            (
                AppStatus::Running {
                    pid,
                    start_time,
                    environment,
                    command_variant,
                    ..
                },
                AppStatus::Running {
                    pid: other_pid,
                    start_time: other_start,
                    environment: other_env,
                    command_variant: other_variant,
                    ..
                },
            ) => {
                pid == other_pid
                    && start_time == other_start
                    && environment == other_env
                    && command_variant == other_variant
            }
            _ => self == other,
        }
    }

    /// Returns true if the app is running
    pub fn is_running(&self) -> bool {
        matches!(self, AppStatus::Running { .. })
    }

    /// Short status label (Running / Stopped / Unknown)
    pub fn as_str(&self) -> &str {
        match self {
            AppStatus::Running { .. } => "Running",
            AppStatus::Stopped => "Stopped",
            AppStatus::Unknown => "Unknown",
        }
    }

    /// Human-readable label including environment and command variant when running.
    pub fn display_label(&self) -> String {
        match self {
            AppStatus::Running {
                environment,
                command_variant,
                ..
            } => match (environment.as_deref(), command_variant.as_deref()) {
                (Some(env), Some(variant)) => format!("Running ({env}: {variant})"),
                (Some(env), None) => format!("Running ({env})"),
                (None, Some(variant)) => format!("Running ({variant})"),
                (None, None) => "Running".to_string(),
            },
            AppStatus::Stopped => "Stopped".to_string(),
            AppStatus::Unknown => "Unknown".to_string(),
        }
    }
}

/// Represents the different views in the TUI
#[derive(Debug, Clone, PartialEq)]
pub enum ViewType {
    /// Main view showing projects and apps
    Main,
    /// Log viewer showing contents of specific log files
    LogViewer {
        /// Paths of the log files it opens with (none: it opens on the log
        /// selector)
        log_paths: Vec<PathBuf>,
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
            environment: Some("local".to_string()),
            command_variant: Some("start".to_string()),
        };
        assert!(running.is_running());
        assert_eq!(running.display_label(), "Running (local: start)");

        let stopped = AppStatus::Stopped;
        assert!(!stopped.is_running());
    }

    #[test]
    fn test_app_status_as_str() {
        let running = AppStatus::Running {
            pid: 1234,
            uptime: Duration::seconds(60),
            start_time: Utc::now(),
            environment: None,
            command_variant: None,
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
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::test_utils::{AppBuilder, ConfigBuilder};

    fn state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = StateStore::new(dir.path().to_path_buf()).unwrap();
        let app = || AppBuilder::new("nodejs", "/tmp").build();
        let config = ConfigBuilder::new()
            .with_app("a", "a1", app())
            .with_app("a", "a2", app())
            .with_app("b", "b1", app())
            .with_app("b", "b2", app())
            .build();
        (
            AppState::from_config_with_store(&config, &store).unwrap(),
            dir,
        )
    }

    fn at(state: &AppState) -> (String, usize) {
        (
            state.projects[state.selected_project_idx].name.clone(),
            state.selected_app_idx,
        )
    }

    #[test]
    fn collapsed_projects_are_a_single_row() {
        let (mut state, _dir) = state();
        let a = state.projects.iter().position(|p| p.name == "a").unwrap();
        state.selected_project_idx = a;
        state.selected_app_idx = 1;

        state.toggle_project_expansion(); // collapse "a"
        assert_eq!(at(&state), ("a".to_string(), 0));

        state.select_next(); // skips a's hidden apps
        assert_eq!(at(&state).0, "b");

        state.select_previous(); // back onto a's row, not its last hidden app
        assert_eq!(at(&state), ("a".to_string(), 0));
    }

    #[test]
    fn config_refresh_keeps_ui_state_and_follows_the_selection() {
        let (mut state, _dir) = state();
        let b = state.projects.iter().position(|p| p.name == "b").unwrap();
        state.selected_project_idx = b;
        state.selected_app_idx = 1; // b2
        let a = state.projects.iter().position(|p| p.name == "a").unwrap();
        state.projects[a].expanded = false;
        state.view_history.push(ViewType::Main);
        state.projects[b].apps[1].restart_count = 7;

        // b gains an app sorted before b2.
        let app = || AppBuilder::new("nodejs", "/tmp").build();
        let config = ConfigBuilder::new()
            .with_app("a", "a1", app())
            .with_app("a", "a2", app())
            .with_app("b", "b0", app())
            .with_app("b", "b1", app())
            .with_app("b", "b2", app())
            .build();
        state.refresh_from_config(&config).unwrap();

        let selected = state.selected_app().unwrap();
        assert_eq!(
            (selected.project.as_str(), selected.name.as_str()),
            ("b", "b2")
        );
        assert_eq!(selected.restart_count, 7);
        let a = state.projects.iter().position(|p| p.name == "a").unwrap();
        assert!(!state.projects[a].expanded);
        assert_eq!(state.view_history.len(), 1);
    }

    #[test]
    fn commands_of_every_environment_reach_the_tui() {
        let dir = tempfile::TempDir::new().unwrap();
        let store = StateStore::new(dir.path().to_path_buf()).unwrap();
        let mut app = AppBuilder::new("nodejs", "/tmp")
            .with_local_command("start", "npm start")
            .build();
        app.commands.docker_compose =
            Some([("up".to_string(), "docker compose up".to_string())].into());
        app.commands.ci = Some([("test".to_string(), "npm test".to_string())].into());
        let config = ConfigBuilder::new().with_app("p", "api", app).build();

        let state = AppState::from_config_with_store(&config, &store).unwrap();
        let commands = &state.projects[0].apps[0].commands;
        assert!(commands.contains_key("docker-compose"), "{commands:?}");
        assert!(commands.contains_key("ci"), "{commands:?}");
    }

    fn tracked(project: &str, slug: &str, app: &str) -> TrackedInstance {
        TrackedInstance {
            project: format!("{project}@{slug}"),
            base_project: project.to_string(),
            slug: slug.to_string(),
            app: app.to_string(),
            path: format!("/w/{slug}"),
            repo_apps: None,
        }
    }

    #[test]
    fn worktree_instances_are_listed_after_their_project() {
        let (mut state, _dir) = state();
        let b = state.projects.iter().position(|p| p.name == "b").unwrap();
        state.selected_project_idx = b;
        state.selected_app_idx = 1; // b2

        let instances = [tracked("a", "z", "a1"), tracked("a", "feat", "a2")];
        assert!(state.sync_worktree_projects(&instances));
        let names: Vec<_> = state.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["a", "a@feat", "a@z", "b"]);
        assert_eq!(
            at(&state),
            ("b".to_string(), 1),
            "the selection stays on b2"
        );

        let feat = &state.projects[1];
        assert_eq!(feat.worktree.as_deref(), Some("feat"));
        assert_eq!(
            feat.apps.len(),
            2,
            "every app can be started in the worktree"
        );
        assert!(feat.apps.iter().all(|a| a.project == "a@feat"));
        let a2 = feat.apps.iter().find(|a| a.name == "a2").unwrap();
        assert_eq!(a2.path.as_deref(), Some("/w/feat"));
        assert_eq!(
            feat.apps.iter().find(|a| a.name == "a1").unwrap().path,
            None
        );

        // Nothing new: nothing changes.
        assert!(!state.sync_worktree_projects(&instances));

        // Their processes are gone: so are they.
        assert!(state.sync_worktree_projects(&[]));
        let names: Vec<_> = state.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
        assert_eq!(at(&state), ("b".to_string(), 1));
    }

    fn key(project: &str, app: &str) -> AppKey {
        AppKey::new(project, app)
    }

    /// The selected worktree row vanishes (its process stopped): the
    /// selection goes to the same app of the configured project, not to
    /// whatever app now sits at the old position.
    #[test]
    fn a_vanished_worktree_row_selects_its_configured_app() {
        let (mut state, _dir) = state();
        state.sync_worktree_projects(&[tracked("a", "feat", "a2")]);
        assert!(state.select_app(&key("a@feat", "a2")));

        assert!(state.sync_worktree_projects(&[]));
        assert_eq!(state.selected_key(), Some(key("a", "a2")));
    }

    /// An app removed from the config: its project's neighbour is selected.
    #[test]
    fn a_removed_app_selects_its_neighbour_in_the_same_project() {
        let (mut state, _dir) = state();
        assert!(state.select_app(&key("b", "b2")));
        let app = || AppBuilder::new("nodejs", "/tmp").build();
        let config = ConfigBuilder::new()
            .with_app("a", "a1", app())
            .with_app("a", "a2", app())
            .with_app("b", "b1", app())
            .build();
        state.refresh_from_config(&config).unwrap();
        assert_eq!(state.selected_key(), Some(key("b", "b1")));
    }

    /// A config save no longer drops the worktree rows until the next poll
    /// (the selection on one jumped to another app meanwhile).
    #[test]
    fn config_refresh_keeps_worktree_rows_and_their_selection() {
        let (mut state, _dir) = state();
        state.sync_worktree_projects(&[tracked("a", "feat", "a2")]);
        assert!(state.select_app(&key("a@feat", "a2")));
        let row = state
            .projects
            .iter_mut()
            .find(|p| p.name == "a@feat")
            .unwrap();
        row.apps[1].restart_count = 3;

        let app = || AppBuilder::new("nodejs", "/tmp").build();
        let config = ConfigBuilder::new()
            .with_app("a", "a1", app())
            .with_app("a", "a2", app())
            .with_app("b", "b1", app())
            .with_app("b", "b2", app())
            .build();
        state.refresh_from_config(&config).unwrap();

        let names: Vec<_> = state.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["a", "a@feat", "b"]);
        let selected = state.selected_app().unwrap();
        assert_eq!(AppKey::of(selected), key("a@feat", "a2"));
        assert_eq!(selected.restart_count, 3);
        assert_eq!(selected.path.as_deref(), Some("/w/feat"));
        // Still the same row for the poller: nothing changes.
        assert!(!state.sync_worktree_projects(&[tracked("a", "feat", "a2")]));
    }

    /// A worktree row lists the apps of its repository only.
    #[test]
    fn worktree_rows_list_the_apps_of_their_repository() {
        let (mut state, _dir) = state();
        let mut instance = tracked("a", "feat", "a2");
        instance.repo_apps = Some(vec!["a2".to_string()]);
        state.sync_worktree_projects(&[instance]);
        let row = state.projects.iter().find(|p| p.name == "a@feat").unwrap();
        let apps: Vec<_> = row.apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(apps, vec!["a2"]);
    }

    #[test]
    fn apps_in_repository_compares_common_dirs() {
        use crate::worktrees::git::tests::{init_repo, run_git};

        let dir = tempfile::TempDir::new().unwrap();
        let repo = init_repo(&dir.path().join("one"));
        let other = init_repo(&dir.path().join("two"));
        let feat = dir.path().join("feat");
        run_git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                feat.to_str().unwrap(),
                "-b",
                "feat",
            ],
        );
        let at = |path: &std::path::Path| AppBuilder::new("nodejs", path.to_str().unwrap()).build();
        let config = ConfigBuilder::new()
            .with_app("p", "api", at(&repo))
            .with_app("p", "web", at(&repo.join("apps/api")))
            .with_app("p", "db", at(&other))
            .with_app("p", "tmp", at(dir.path()))
            .build();
        let mut apps =
            TrackedInstance::apps_in_repository(&config, "p", "api", feat.to_str().unwrap());
        apps.sort();
        assert_eq!(apps, vec!["api", "web"]);
    }

    /// A pending action is for the app it was asked for.
    #[test]
    fn requests_name_the_app_they_were_made_for() {
        let (mut state, _dir) = state();
        assert!(state.select_app(&key("a", "a1")));
        state.set_stop_requested();
        assert!(state.select_app(&key("b", "b1")));
        assert_eq!(state.stop_requested, Some(key("a", "a1")));
    }
}
