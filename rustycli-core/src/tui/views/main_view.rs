// Main view implementation with split-panel layout
// Inspired by GitUI's clean tab-based interface
// Features: Status, Commands, and Logs tabs with project/app tree on left and details on right

use crate::config::loader::{load_config, save_config};
use crate::config::models::{App, Commands, Defaults, Project};
use crate::tui::log_manager::LogManager;
use crate::tui::state::{AppState, AppStateData};
use crate::tui::theme::Theme;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Main view with tab-based navigation and split-panel layout
/// Left panel (30%): Project/app tree with status indicators
/// Right panel (70%): Details based on selected tab
pub struct MainView {
    /// Currently active tab
    pub(crate) active_tab: MainTab,
    /// Which panel has focus (left app list or right details)
    pub(crate) focus: PanelFocus,
    /// Scroll offset for the left panel list
    /// Reserved for future use when implementing scrolling in the app list
    #[allow(dead_code)]
    pub(crate) list_scroll: usize,
    /// Scroll offset for the right panel details
    pub(crate) detail_scroll: usize,
    /// Selected command index in the Commands tab
    /// Tracks which command is highlighted in the detail panel
    pub(crate) selected_command_idx: usize,
    /// Selected log file index in the Logs tab
    /// Tracks which log file is highlighted in the detail panel
    pub(crate) selected_log_idx: usize,
    /// Log manager for discovering log files
    pub(crate) log_manager: LogManager,
    /// Config editor mode (view, add, edit)
    pub(crate) config_mode: ConfigMode,
    /// Form data for adding/editing apps
    pub(crate) config_form: ConfigForm,
    /// Currently focused field in config form
    pub(crate) config_focused_field: ConfigField,
    /// Selected command index in Config tab view mode
    pub(crate) selected_config_command_idx: usize,
    /// Selected dependency index in dependencies popup
    pub(crate) selected_dependency_idx: usize,
    /// Selected project index when adding dependency
    pub(crate) selected_add_dep_project_idx: usize,
    /// Selected app index when adding dependency
    pub(crate) selected_add_dep_app_idx: usize,
}

/// Config editor modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigMode {
    /// View mode - list all apps
    View,
    /// Add mode - form to add new app
    Add,
    /// Edit mode - form to edit existing app
    Edit,
    /// Edit command mode - edit a specific command
    EditCommand,
    /// Edit dependencies mode - manage dependencies
    EditDependencies,
    /// Add dependency mode - select project and app
    AddDependency,
}

/// Form data for config editor
#[derive(Debug, Clone, Default)]
pub struct ConfigForm {
    pub project_name: String,
    pub app_name: String,
    pub app_type: String,
    pub path: String,
    pub local_start_cmd: String,
    pub docker_start_cmd: String,
    // For editing a specific command
    pub edit_command_env: String,
    pub edit_command_name: String,
    pub edit_command_value: String,
    // Cursor positions for each field
    pub cursor_project_name: usize,
    pub cursor_app_name: usize,
    pub cursor_app_type: usize,
    pub cursor_path: usize,
    pub cursor_local_start_cmd: usize,
    pub cursor_docker_start_cmd: usize,
    pub cursor_edit_command_name: usize,
    pub cursor_edit_command_value: usize,
}

/// Config form fields
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    ProjectName,
    AppName,
    AppType,
    Path,
    LocalStartCmd,
    DockerStartCmd,
    EditCommandName,
    EditCommandValue,
}

/// The four main tabs in the interface
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    /// Status tab - shows app details and running status
    Status,
    /// Commands tab - shows available commands for selected app
    Commands,
    /// Logs tab - shows log files for selected app
    Logs,
    /// Config tab - manage app configurations
    Config,
}

/// Indicates which panel currently has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    /// Left panel with project/app tree
    AppList,
    /// Right panel with details/commands/logs
    DetailPanel,
}

impl MainView {
    /// Creates a new MainView with default settings
    pub fn new() -> Self {
        Self {
            active_tab: MainTab::Status,
            focus: PanelFocus::AppList,
            list_scroll: 0,
            detail_scroll: 0,
            selected_command_idx: 0,
            selected_log_idx: 0,
            log_manager: LogManager::default(),
            config_mode: ConfigMode::View,
            config_form: ConfigForm::default(),
            config_focused_field: ConfigField::ProjectName,
            selected_config_command_idx: 0,
            selected_dependency_idx: 0,
            selected_add_dep_project_idx: 0,
            selected_add_dep_app_idx: 0,
        }
    }

    /// Handles keyboard input for the main view
    /// Returns true if the event was handled, false otherwise
    pub fn handle_input(&mut self, key: KeyEvent, state: &Arc<Mutex<AppState>>) -> Result<bool> {
        // Lock the state for modification
        // Use expect instead of context since PoisonError doesn't implement StdError
        let mut state = state.lock().expect("Failed to lock state");
        match key.code {
            // Tab switching with number keys (1-3)
            // Also support keyboard layout variants (e.g., French AZERTY: &=1, é=2, "=3)
            KeyCode::Char('1') | KeyCode::Char('&') => {
                self.active_tab = MainTab::Status;
                self.detail_scroll = 0; // Reset scroll when switching tabs
                self.selected_command_idx = 0; // Reset command selection
                Ok(true)
            }
            KeyCode::Char('2') | KeyCode::Char('é') => {
                self.active_tab = MainTab::Commands;
                self.detail_scroll = 0;
                self.selected_command_idx = 0; // Reset command selection
                Ok(true)
            }
            KeyCode::Char('3') | KeyCode::Char('"') => {
                self.active_tab = MainTab::Logs;
                self.detail_scroll = 0;
                self.selected_command_idx = 0; // Reset command selection
                self.selected_log_idx = 0; // Reset log selection
                Ok(true)
            }
            KeyCode::Char('4') | KeyCode::Char('\'') => {
                self.active_tab = MainTab::Config;
                self.detail_scroll = 0;
                self.config_mode = ConfigMode::View;
                self.selected_config_command_idx = 0;
                Ok(true)
            }
            // Tab key to cycle through tabs or form fields
            KeyCode::Tab => {
                // In command edit mode, Tab switches between name and value
                if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditCommand {
                    self.config_focused_field = match self.config_focused_field {
                        ConfigField::EditCommandName => ConfigField::EditCommandValue,
                        ConfigField::EditCommandValue => ConfigField::EditCommandName,
                        _ => ConfigField::EditCommandName,
                    };
                } else if self.active_tab == MainTab::Config && matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
                    self.config_focused_field = match self.config_focused_field {
                        ConfigField::ProjectName => ConfigField::AppName,
                        ConfigField::AppName => ConfigField::AppType,
                        ConfigField::AppType => ConfigField::Path,
                        ConfigField::Path => ConfigField::LocalStartCmd,
                        ConfigField::LocalStartCmd => ConfigField::DockerStartCmd,
                        ConfigField::DockerStartCmd => ConfigField::ProjectName,
                        _ => ConfigField::ProjectName,
                    };
                } else {
                    // Cycle to next tab
                    self.active_tab = match self.active_tab {
                        MainTab::Status => MainTab::Commands,
                        MainTab::Commands => MainTab::Logs,
                        MainTab::Logs => MainTab::Config,
                        MainTab::Config => MainTab::Status,
                    };
                    self.detail_scroll = 0;
                    self.selected_command_idx = 0;
                    if self.active_tab == MainTab::Logs {
                        self.selected_log_idx = 0;
                    }
                    if self.active_tab == MainTab::Config {
                        self.config_mode = ConfigMode::View;
                    }
                }
                Ok(true)
            }
            // Left arrow - move cursor or switch panel
            KeyCode::Left => {
                // In edit modes, move cursor left
                if self.active_tab == MainTab::Config && matches!(
                    self.config_mode,
                    ConfigMode::Add | ConfigMode::Edit | ConfigMode::EditCommand
                ) {
                    self.move_cursor_left();
                    return Ok(true);
                }
                
                // Otherwise switch panel
                self.focus = match self.focus {
                    PanelFocus::AppList => PanelFocus::DetailPanel,
                    PanelFocus::DetailPanel => PanelFocus::AppList,
                };
                Ok(true)
            }
            // Right arrow - move cursor or switch panel
            KeyCode::Right => {
                // In edit modes, move cursor right
                if self.active_tab == MainTab::Config && matches!(
                    self.config_mode,
                    ConfigMode::Add | ConfigMode::Edit | ConfigMode::EditCommand
                ) {
                    self.move_cursor_right();
                    return Ok(true);
                }
                
                // Otherwise switch panel
                self.focus = match self.focus {
                    PanelFocus::AppList => PanelFocus::DetailPanel,
                    PanelFocus::DetailPanel => PanelFocus::AppList,
                };
                Ok(true)
            }
            // Shift+Up - jump up by 10
            KeyCode::Up if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                match self.focus {
                    PanelFocus::AppList => {
                        for _ in 0..10 {
                            state.select_previous();
                        }
                        self.selected_command_idx = 0;
                        self.selected_log_idx = 0;
                        self.selected_config_command_idx = 0;
                    }
                    PanelFocus::DetailPanel => {
                        if self.active_tab == MainTab::Commands {
                            self.selected_command_idx = self.selected_command_idx.saturating_sub(10);
                        } else if self.active_tab == MainTab::Logs {
                            self.selected_log_idx = self.selected_log_idx.saturating_sub(10);
                        } else if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View {
                            self.selected_config_command_idx = self.selected_config_command_idx.saturating_sub(10);
                        } else {
                            self.detail_scroll = self.detail_scroll.saturating_sub(10);
                        }
                    }
                }
                Ok(true)
            }
            // Shift+Down - jump down by 10
            KeyCode::Down if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                match self.focus {
                    PanelFocus::AppList => {
                        for _ in 0..10 {
                            state.select_next();
                        }
                        self.selected_command_idx = 0;
                        self.selected_log_idx = 0;
                        self.selected_config_command_idx = 0;
                    }
                    PanelFocus::DetailPanel => {
                        if self.active_tab == MainTab::Commands {
                            if let Some(app) = state.selected_app() {
                                let total_commands = Self::count_total_commands(app);
                                self.selected_command_idx = (self.selected_command_idx + 10).min(total_commands.saturating_sub(1));
                            }
                        } else if self.active_tab == MainTab::Logs {
                            if let Some(app) = state.selected_app() {
                                if let Ok(log_files) = self.log_manager.list_logs_for_app(&app.name) {
                                    self.selected_log_idx = (self.selected_log_idx + 10).min(log_files.len().saturating_sub(1));
                                }
                            }
                        } else if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View {
                            if let Some(app) = state.selected_app() {
                                let total_commands = Self::count_total_commands(app);
                                self.selected_config_command_idx = (self.selected_config_command_idx + 10).min(total_commands.saturating_sub(1));
                            }
                        } else {
                            self.detail_scroll = self.detail_scroll.saturating_add(10);
                        }
                    }
                }
                Ok(true)
            }
            // Navigation keys - behavior depends on which panel has focus
            KeyCode::Up | KeyCode::Char('k') => {
                // Dependencies popup navigation
                if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditDependencies {
                    self.selected_dependency_idx = self.selected_dependency_idx.saturating_sub(1);
                    return Ok(true);
                }
                // Add dependency navigation
                if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::AddDependency {
                    self.selected_add_dep_project_idx = self.selected_add_dep_project_idx.saturating_sub(1);
                    self.selected_add_dep_app_idx = 0;
                    return Ok(true);
                }
                // In config form mode, Up navigates between fields
                if self.active_tab == MainTab::Config && matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
                    self.config_focused_field = match self.config_focused_field {
                        ConfigField::ProjectName => ConfigField::DockerStartCmd,
                        ConfigField::AppName => ConfigField::ProjectName,
                        ConfigField::AppType => ConfigField::AppName,
                        ConfigField::Path => ConfigField::AppType,
                        ConfigField::LocalStartCmd => ConfigField::Path,
                        ConfigField::DockerStartCmd => ConfigField::LocalStartCmd,
                        _ => ConfigField::ProjectName,
                    };
                } else {
                    match self.focus {
                        PanelFocus::AppList => {
                            state.select_previous();
                            // Reset command and log selection when changing apps
                            self.selected_command_idx = 0;
                            self.selected_log_idx = 0;
                            self.selected_config_command_idx = 0;
                        }
                        PanelFocus::DetailPanel => {
                            // In Commands tab, navigate through commands
                            if self.active_tab == MainTab::Commands {
                                self.selected_command_idx = self.selected_command_idx.saturating_sub(1);
                            } else if self.active_tab == MainTab::Logs {
                                // In Logs tab, navigate through log files
                                self.selected_log_idx = self.selected_log_idx.saturating_sub(1);
                            } else if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View {
                                // In Config tab view mode, navigate through commands
                                self.selected_config_command_idx = self.selected_config_command_idx.saturating_sub(1);
                            } else {
                                // Scroll up in detail panel for other tabs
                                self.detail_scroll = self.detail_scroll.saturating_sub(1);
                            }
                        }
                    }
                }
                Ok(true)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                // Dependencies popup navigation
                if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditDependencies {
                    if let Some(app) = state.selected_app() {
                        if self.selected_dependency_idx < app.dependencies.len().saturating_sub(1) {
                            self.selected_dependency_idx += 1;
                        }
                    }
                    return Ok(true);
                }
                // Add dependency navigation
                if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::AddDependency {
                    if let Ok(config) = load_config() {
                        if self.selected_add_dep_project_idx < config.projects.len().saturating_sub(1) {
                            self.selected_add_dep_project_idx += 1;
                            self.selected_add_dep_app_idx = 0;
                        }
                    }
                    return Ok(true);
                }
                // In config form mode, Down navigates between fields
                if self.active_tab == MainTab::Config && matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
                    self.config_focused_field = match self.config_focused_field {
                        ConfigField::ProjectName => ConfigField::AppName,
                        ConfigField::AppName => ConfigField::AppType,
                        ConfigField::AppType => ConfigField::Path,
                        ConfigField::Path => ConfigField::LocalStartCmd,
                        ConfigField::LocalStartCmd => ConfigField::DockerStartCmd,
                        ConfigField::DockerStartCmd => ConfigField::ProjectName,
                        _ => ConfigField::ProjectName,
                    };
                } else {
                    match self.focus {
                        PanelFocus::AppList => {
                            state.select_next();
                            // Reset command and log selection when changing apps
                            self.selected_command_idx = 0;
                            self.selected_log_idx = 0;
                            self.selected_config_command_idx = 0;
                        }
                        PanelFocus::DetailPanel => {
                            // In Commands tab, navigate through commands
                            if self.active_tab == MainTab::Commands {
                                // Get total command count to limit navigation
                                if let Some(app) = state.selected_app() {
                                    let total_commands = Self::count_total_commands(app);
                                    if self.selected_command_idx < total_commands.saturating_sub(1) {
                                        self.selected_command_idx += 1;
                                    }
                                }
                            } else if self.active_tab == MainTab::Logs {
                                // In Logs tab, navigate through log files
                                if let Some(app) = state.selected_app() {
                                    // Get log files to determine max index
                                    if let Ok(log_files) = self.log_manager.list_logs_for_app(&app.name) {
                                        if self.selected_log_idx < log_files.len().saturating_sub(1) {
                                            self.selected_log_idx += 1;
                                        }
                                    }
                                }
                            } else if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View {
                                // In Config tab view mode, navigate through commands
                                if let Some(app) = state.selected_app() {
                                    let total_commands = Self::count_total_commands(app);
                                    if self.selected_config_command_idx < total_commands.saturating_sub(1) {
                                        self.selected_config_command_idx += 1;
                                    }
                                }
                            } else {
                                // Scroll down in detail panel for other tabs
                                self.detail_scroll = self.detail_scroll.saturating_add(1);
                            }
                        }
                    }
                }
                Ok(true)
            }
            // Space to expand/collapse projects (only in app list)
            KeyCode::Char(' ') if self.focus == PanelFocus::AppList => {
                state.toggle_project_expansion();
                Ok(true)
            }
            // Enter key - trigger actions based on context
            KeyCode::Enter => {
                // Add dependency
                if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::AddDependency {
                    if let Some(app) = state.selected_app() {
                        let project = app.project.clone();
                        let app_name = app.name.clone();
                        drop(state);
                        if let Err(e) = self.add_dependency(&project, &app_name) {
                            eprintln!("Error adding dependency: {}", e);
                        }
                    }
                    return Ok(true);
                }
                
                if self.focus == PanelFocus::DetailPanel {
                    match self.active_tab {
                        MainTab::Commands => {
                            // Signal that we want to execute a command
                            // The actual execution will be handled by the app
                            state.set_command_execution_requested(self.selected_command_idx);
                        }
                        MainTab::Logs => {
                            // Open the selected log file in the log viewer
                            if let Some(app) = state.selected_app() {
                                if let Ok(log_files) = self.log_manager.list_logs_for_app(&app.name) {
                                    if let Some(log_file) = log_files.get(self.selected_log_idx) {
                                        // Transition to log viewer view
                                        state.current_view = crate::tui::state::ViewType::LogViewer {
                                            log_path: log_file.path.clone(),
                                        };
                                    }
                                }
                            }
                        }
                        MainTab::Config => {
                            if matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
                                // Save the app form
                                drop(state); // Release lock before saving
                                if let Err(e) = self.save_config_form() {
                                    eprintln!("Error saving config: {}", e);
                                }
                                return Ok(true);
                            } else if self.config_mode == ConfigMode::EditCommand {
                                // Save the command edit
                                if let Some(app) = state.selected_app() {
                                    let project = app.project.clone();
                                    let app_name = app.name.clone();
                                    drop(state); // Release lock before saving
                                    if let Err(e) = self.save_command_edit(&project, &app_name) {
                                        eprintln!("Error saving command: {}", e);
                                    }
                                }
                                return Ok(true);
                            }
                        }
                        MainTab::Status => {
                            // Enter key in Status tab - start the app if stopped
                            if let Some(app) = state.selected_app() {
                                if !app.status.is_running() {
                                    // Request command execution for the default start command (index 0)
                                    state.set_command_execution_requested(0);
                                }
                            }
                        }
                    }
                }
                Ok(true)
            }
            // Config tab specific keys
            KeyCode::Char('a') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View => {
                self.config_mode = ConfigMode::Add;
                self.config_form = ConfigForm::default();
                self.config_focused_field = ConfigField::ProjectName;
                // Reset all cursors to end of fields (which is 0 for empty fields)
                Ok(true)
            }
            KeyCode::Char('e') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View => {
                // Edit selected command (when detail panel has focus)
                if self.focus == PanelFocus::DetailPanel {
                    if let Some(app) = state.selected_app() {
                        let project = app.project.clone();
                        let app_name = app.name.clone();
                        drop(state); // Release lock before loading config
                        if let Err(e) = self.load_command_into_form(&project, &app_name) {
                            eprintln!("Error loading command: {}", e);
                        }
                    }
                }
                Ok(true)
            }
            KeyCode::Char('E') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View => {
                // Edit entire app (Shift+E)
                if let Some(app) = state.selected_app() {
                    self.load_app_into_form(&app.project, &app.name, app);
                    self.config_mode = ConfigMode::Edit;
                    self.config_focused_field = ConfigField::ProjectName;
                }
                Ok(true)
            }
            KeyCode::Char('D') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View => {
                // Edit dependencies (Shift+D)
                self.config_mode = ConfigMode::EditDependencies;
                self.selected_dependency_idx = 0;
                Ok(true)
            }
            KeyCode::Char('s') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View && self.focus == PanelFocus::DetailPanel => {
                // Set selected command as default
                if let Some(app) = state.selected_app() {
                    let project = app.project.clone();
                    let app_name = app.name.clone();
                    drop(state);
                    if let Err(e) = self.set_command_as_default(&project, &app_name) {
                        eprintln!("Error setting default command: {}", e);
                    }
                }
                Ok(true)
            }
            KeyCode::Char('d') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View => {
                // Delete selected app
                if let Some(app) = state.selected_app() {
                    let project = app.project.clone();
                    let app_name = app.name.clone();
                    drop(state); // Release lock before deleting
                    if let Err(e) = self.delete_app(&project, &app_name) {
                        eprintln!("Error deleting app: {}", e);
                    }
                }
                Ok(true)
            }
            // Dependencies popup actions
            KeyCode::Char('a') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditDependencies => {
                // Add dependency
                self.config_mode = ConfigMode::AddDependency;
                self.selected_add_dep_project_idx = 0;
                self.selected_add_dep_app_idx = 0;
                Ok(true)
            }
            KeyCode::Char('d') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditDependencies => {
                // Delete selected dependency
                if let Some(app) = state.selected_app() {
                    let project = app.project.clone();
                    let app_name = app.name.clone();
                    drop(state);
                    if let Err(e) = self.remove_dependency(&project, &app_name, self.selected_dependency_idx) {
                        eprintln!("Error removing dependency: {}", e);
                    }
                }
                Ok(true)
            }
            KeyCode::Esc if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditDependencies => {
                self.config_mode = ConfigMode::View;
                Ok(true)
            }
            KeyCode::Esc if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::AddDependency => {
                self.config_mode = ConfigMode::EditDependencies;
                Ok(true)
            }
            // Form input handling
            KeyCode::Char(c) if self.active_tab == MainTab::Config && matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit | ConfigMode::EditCommand) => {
                self.add_char_to_field(c);
                Ok(true)
            }
            KeyCode::Backspace if self.active_tab == MainTab::Config && matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit | ConfigMode::EditCommand) => {
                self.remove_char_from_field();
                Ok(true)
            }
            KeyCode::Esc if self.active_tab == MainTab::Config && matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit | ConfigMode::EditCommand) => {
                self.config_mode = ConfigMode::View;
                Ok(true)
            }
            // Start/Stop app with 's' key in Status tab (toggle behavior)
            KeyCode::Char('s') if self.active_tab == MainTab::Status => {
                if let Some(app) = state.selected_app() {
                    if app.status.is_running() {
                        // App is running - request stop with popup
                        state.set_stop_requested();
                    } else {
                        // App is stopped - request environment selection
                        state.set_env_selection_requested();
                    }
                }
                Ok(true)
            }
            // Restart app with 'r' key in Status tab
            KeyCode::Char('r') if self.active_tab == MainTab::Status => {
                if let Some(app) = state.selected_app() {
                    if app.status.is_running() {
                        // Request restart with popup
                        state.set_restart_requested();
                    }
                }
                Ok(true)
            }
            _ => Ok(false), // Event not handled
        }
    }

    /// Loads an app into the config form for editing
    fn load_app_into_form(&mut self, project: &str, app_name: &str, app: &AppStateData) {
        self.config_form.project_name = project.to_string();
        self.config_form.app_name = app_name.to_string();
        self.config_form.app_type = app.app_type.clone();
        self.config_form.path = app.path.clone().unwrap_or_default();

        // Load commands if available
        if let Some(local) = app.commands.get("local") {
            if let Some(start_cmd) = local.first() {
                self.config_form.local_start_cmd = start_cmd.command.clone();
            }
        }
        if let Some(docker) = app.commands.get("docker") {
            if let Some(start_cmd) = docker.first() {
                self.config_form.docker_start_cmd = start_cmd.command.clone();
            }
        }
        
        // Set cursors to end of each field
        self.config_form.cursor_project_name = self.config_form.project_name.len();
        self.config_form.cursor_app_name = self.config_form.app_name.len();
        self.config_form.cursor_app_type = self.config_form.app_type.len();
        self.config_form.cursor_path = self.config_form.path.len();
        self.config_form.cursor_local_start_cmd = self.config_form.local_start_cmd.len();
        self.config_form.cursor_docker_start_cmd = self.config_form.docker_start_cmd.len();
    }

    /// Loads a specific command into the form for editing
    fn load_command_into_form(&mut self, project: &str, app_name: &str) -> Result<()> {
        let config = load_config()?;
        
        if let Some(proj) = config.projects.get(project) {
            if let Some(app) = proj.apps.get(app_name) {
                // Find the selected command
                let mut global_idx = 0;
                
                // Check local commands
                if let Some(local) = &app.commands.local {
                    let mut sorted: Vec<_> = local.iter().collect();
                    sorted.sort_by_key(|(name, _)| *name);
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        let (name, cmd) = sorted[idx];
                        self.config_form.edit_command_env = "local".to_string();
                        self.config_form.edit_command_name = name.clone();
                        self.config_form.edit_command_value = cmd.clone();
                        self.config_form.cursor_edit_command_name = name.len();
                        self.config_form.cursor_edit_command_value = cmd.len();
                        self.config_focused_field = ConfigField::EditCommandName;
                        self.config_mode = ConfigMode::EditCommand;
                        return Ok(());
                    }
                    global_idx += sorted.len();
                }
                
                // Check docker commands
                if let Some(docker) = &app.commands.docker {
                    let mut sorted: Vec<_> = docker.iter().collect();
                    sorted.sort_by_key(|(name, _)| *name);
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        let (name, cmd) = sorted[idx];
                        self.config_form.edit_command_env = "docker".to_string();
                        self.config_form.edit_command_name = name.clone();
                        self.config_form.edit_command_value = cmd.clone();
                        self.config_form.cursor_edit_command_name = name.len();
                        self.config_form.cursor_edit_command_value = cmd.len();
                        self.config_focused_field = ConfigField::EditCommandName;
                        self.config_mode = ConfigMode::EditCommand;
                        return Ok(());
                    }
                    global_idx += sorted.len();
                }
                
                // Check k8s commands
                if let Some(k8s) = &app.commands.k8s {
                    let mut sorted: Vec<_> = k8s.iter().collect();
                    sorted.sort_by_key(|(name, _)| *name);
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        let (name, cmd) = sorted[idx];
                        self.config_form.edit_command_env = "k8s".to_string();
                        self.config_form.edit_command_name = name.clone();
                        self.config_form.edit_command_value = cmd.clone();
                        self.config_form.cursor_edit_command_name = name.len();
                        self.config_form.cursor_edit_command_value = cmd.len();
                        self.config_focused_field = ConfigField::EditCommandName;
                        self.config_mode = ConfigMode::EditCommand;
                        return Ok(());
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Adds a character to the currently focused field
    fn add_char_to_field(&mut self, c: char) {
        let (field, cursor) = match self.config_focused_field {
            ConfigField::ProjectName => (&mut self.config_form.project_name, &mut self.config_form.cursor_project_name),
            ConfigField::AppName => (&mut self.config_form.app_name, &mut self.config_form.cursor_app_name),
            ConfigField::AppType => (&mut self.config_form.app_type, &mut self.config_form.cursor_app_type),
            ConfigField::Path => (&mut self.config_form.path, &mut self.config_form.cursor_path),
            ConfigField::LocalStartCmd => (&mut self.config_form.local_start_cmd, &mut self.config_form.cursor_local_start_cmd),
            ConfigField::DockerStartCmd => (&mut self.config_form.docker_start_cmd, &mut self.config_form.cursor_docker_start_cmd),
            ConfigField::EditCommandName => (&mut self.config_form.edit_command_name, &mut self.config_form.cursor_edit_command_name),
            ConfigField::EditCommandValue => (&mut self.config_form.edit_command_value, &mut self.config_form.cursor_edit_command_value),
        };
        field.insert(*cursor, c);
        *cursor += 1;
    }

    /// Removes a character from the currently focused field
    fn remove_char_from_field(&mut self) {
        let (field, cursor) = match self.config_focused_field {
            ConfigField::ProjectName => (&mut self.config_form.project_name, &mut self.config_form.cursor_project_name),
            ConfigField::AppName => (&mut self.config_form.app_name, &mut self.config_form.cursor_app_name),
            ConfigField::AppType => (&mut self.config_form.app_type, &mut self.config_form.cursor_app_type),
            ConfigField::Path => (&mut self.config_form.path, &mut self.config_form.cursor_path),
            ConfigField::LocalStartCmd => (&mut self.config_form.local_start_cmd, &mut self.config_form.cursor_local_start_cmd),
            ConfigField::DockerStartCmd => (&mut self.config_form.docker_start_cmd, &mut self.config_form.cursor_docker_start_cmd),
            ConfigField::EditCommandName => (&mut self.config_form.edit_command_name, &mut self.config_form.cursor_edit_command_name),
            ConfigField::EditCommandValue => (&mut self.config_form.edit_command_value, &mut self.config_form.cursor_edit_command_value),
        };
        if *cursor > 0 {
            *cursor -= 1;
            field.remove(*cursor);
        }
    }

    /// Moves cursor left in the current field
    fn move_cursor_left(&mut self) {
        let cursor = match self.config_focused_field {
            ConfigField::ProjectName => &mut self.config_form.cursor_project_name,
            ConfigField::AppName => &mut self.config_form.cursor_app_name,
            ConfigField::AppType => &mut self.config_form.cursor_app_type,
            ConfigField::Path => &mut self.config_form.cursor_path,
            ConfigField::LocalStartCmd => &mut self.config_form.cursor_local_start_cmd,
            ConfigField::DockerStartCmd => &mut self.config_form.cursor_docker_start_cmd,
            ConfigField::EditCommandName => &mut self.config_form.cursor_edit_command_name,
            ConfigField::EditCommandValue => &mut self.config_form.cursor_edit_command_value,
        };
        *cursor = cursor.saturating_sub(1);
    }

    /// Moves cursor right in the current field
    fn move_cursor_right(&mut self) {
        let (field, cursor) = match self.config_focused_field {
            ConfigField::ProjectName => (&self.config_form.project_name, &mut self.config_form.cursor_project_name),
            ConfigField::AppName => (&self.config_form.app_name, &mut self.config_form.cursor_app_name),
            ConfigField::AppType => (&self.config_form.app_type, &mut self.config_form.cursor_app_type),
            ConfigField::Path => (&self.config_form.path, &mut self.config_form.cursor_path),
            ConfigField::LocalStartCmd => (&self.config_form.local_start_cmd, &mut self.config_form.cursor_local_start_cmd),
            ConfigField::DockerStartCmd => (&self.config_form.docker_start_cmd, &mut self.config_form.cursor_docker_start_cmd),
            ConfigField::EditCommandName => (&self.config_form.edit_command_name, &mut self.config_form.cursor_edit_command_name),
            ConfigField::EditCommandValue => (&self.config_form.edit_command_value, &mut self.config_form.cursor_edit_command_value),
        };
        if *cursor < field.len() {
            *cursor += 1;
        }
    }

    /// Saves the config form
    fn save_config_form(&mut self) -> Result<()> {
        // Validate required fields
        if self.config_form.project_name.is_empty()
            || self.config_form.app_name.is_empty()
            || self.config_form.app_type.is_empty()
            || self.config_form.path.is_empty()
        {
            return Ok(()); // Silently ignore invalid forms
        }

        // Load config
        let mut config = load_config()?;

        // Create the app
        let app = self.create_app_from_form();

        // Get or create the project
        let project = config
            .projects
            .entry(self.config_form.project_name.clone())
            .or_insert_with(|| Project {
                apps: HashMap::new(),
            });

        // Add or update the app
        project.apps.insert(self.config_form.app_name.clone(), app);

        // Save config
        save_config(&config)?;

        // Return to view mode
        self.config_mode = ConfigMode::View;

        Ok(())
    }

    /// Saves a command edit
    fn save_command_edit(&mut self, project: &str, app_name: &str) -> Result<()> {
        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            if let Some(app) = proj.apps.get_mut(app_name) {
                // Update the command in the appropriate environment
                match self.config_form.edit_command_env.as_str() {
                    "local" => {
                        if let Some(local) = &mut app.commands.local {
                            local.insert(
                                self.config_form.edit_command_name.clone(),
                                self.config_form.edit_command_value.clone(),
                            );
                        }
                    }
                    "docker" => {
                        if let Some(docker) = &mut app.commands.docker {
                            docker.insert(
                                self.config_form.edit_command_name.clone(),
                                self.config_form.edit_command_value.clone(),
                            );
                        }
                    }
                    "k8s" => {
                        if let Some(k8s) = &mut app.commands.k8s {
                            k8s.insert(
                                self.config_form.edit_command_name.clone(),
                                self.config_form.edit_command_value.clone(),
                            );
                        }
                    }
                    _ => {}
                }

                save_config(&config)?;
            }
        }

        // Return to view mode
        self.config_mode = ConfigMode::View;

        Ok(())
    }

    /// Creates an App from the form data
    fn create_app_from_form(&self) -> App {
        let mut local_cmds = HashMap::new();
        if !self.config_form.local_start_cmd.is_empty() {
            local_cmds.insert("start".to_string(), self.config_form.local_start_cmd.clone());
        }

        let mut docker_cmds = HashMap::new();
        if !self.config_form.docker_start_cmd.is_empty() {
            docker_cmds.insert(
                "start".to_string(),
                self.config_form.docker_start_cmd.clone(),
            );
        }

        App {
            app_type: self.config_form.app_type.clone(),
            path: self.config_form.path.clone(),
            commands: Commands {
                local: if local_cmds.is_empty() {
                    None
                } else {
                    Some(local_cmds)
                },
                docker: if docker_cmds.is_empty() {
                    None
                } else {
                    Some(docker_cmds)
                },
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: if !self.config_form.local_start_cmd.is_empty() {
                    Some("start".to_string())
                } else {
                    None
                },
                docker: if !self.config_form.docker_start_cmd.is_empty() {
                    Some("start".to_string())
                } else {
                    None
                },
                k8s: None,
            },
        }
    }

    /// Deletes an app from the config
    fn delete_app(&self, project: &str, app: &str) -> Result<()> {
        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            proj.apps.remove(app);

            // Remove project if it has no apps
            if proj.apps.is_empty() {
                config.projects.remove(project);
            }
        }

        save_config(&config)?;

        Ok(())
    }

    /// Sets the selected command as default for its environment
    fn set_command_as_default(&self, project: &str, app_name: &str) -> Result<()> {
        let mut config = load_config()?;

        if let Some(proj) = config.projects.get_mut(project) {
            if let Some(app) = proj.apps.get_mut(app_name) {
                // Find which command is selected
                let mut global_idx = 0;
                
                // Check local commands
                if let Some(local) = &app.commands.local {
                    let mut sorted: Vec<_> = local.keys().cloned().collect();
                    sorted.sort();
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        app.defaults.local = Some(sorted[idx].clone());
                        save_config(&config)?;
                        return Ok(());
                    }
                    global_idx += sorted.len();
                }
                
                // Check docker commands
                if let Some(docker) = &app.commands.docker {
                    let mut sorted: Vec<_> = docker.keys().cloned().collect();
                    sorted.sort();
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        app.defaults.docker = Some(sorted[idx].clone());
                        save_config(&config)?;
                        return Ok(());
                    }
                    global_idx += sorted.len();
                }
                
                // Check k8s commands
                if let Some(k8s) = &app.commands.k8s {
                    let mut sorted: Vec<_> = k8s.keys().cloned().collect();
                    sorted.sort();
                    if self.selected_config_command_idx < global_idx + sorted.len() {
                        let idx = self.selected_config_command_idx - global_idx;
                        app.defaults.k8s = Some(sorted[idx].clone());
                        save_config(&config)?;
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    /// Removes a dependency from an app
    fn remove_dependency(&mut self, project: &str, app_name: &str, dep_idx: usize) -> Result<()> {
        let mut config = load_config()?;

        let mut new_len = 0;
        if let Some(proj) = config.projects.get_mut(project) {
            if let Some(app) = proj.apps.get_mut(app_name) {
                if dep_idx < app.dependencies.len() {
                    app.dependencies.remove(dep_idx);
                    new_len = app.dependencies.len();
                }
            }
        }

        save_config(&config)?;

        // Adjust selected index if needed
        if self.selected_dependency_idx >= new_len && self.selected_dependency_idx > 0 {
            self.selected_dependency_idx -= 1;
        }

        Ok(())
    }

    /// Adds a dependency to an app
    fn add_dependency(&mut self, project: &str, app_name: &str) -> Result<()> {
        let mut config = load_config()?;

        // Get the selected project and app to add as dependency
        let projects: Vec<_> = config.projects.keys().cloned().collect();
        if let Some(dep_project) = projects.get(self.selected_add_dep_project_idx) {
            if let Some(dep_proj) = config.projects.get(dep_project) {
                let apps: Vec<_> = dep_proj.apps.keys().cloned().collect();
                if let Some(dep_app) = apps.get(self.selected_add_dep_app_idx) {
                    // Add the dependency
                    if let Some(proj) = config.projects.get_mut(project) {
                        if let Some(app) = proj.apps.get_mut(app_name) {
                            let dep = crate::config::models::Dependency {
                                project: dep_project.clone(),
                                app: dep_app.clone(),
                            };
                            
                            // Check if dependency already exists
                            if !app.dependencies.iter().any(|d| d.project == dep.project && d.app == dep.app) {
                                app.dependencies.push(dep);
                                save_config(&config)?;
                            }
                        }
                    }
                    
                    // Return to dependencies list
                    self.config_mode = ConfigMode::EditDependencies;
                }
            }
        }

        Ok(())
    }

    /// Counts the total number of commands for an app across all environments
    /// Used to limit command navigation in the Commands tab
    fn count_total_commands(app: &AppStateData) -> usize {
        app.commands.values().map(|cmds| cmds.len()).sum()
    }

    /// Gets the currently selected command info based on the selected index
    /// Returns (environment, command_info) tuple if a valid command is selected
    /// 
    /// Commands are indexed sequentially across environments in order: local, docker, k8s
    /// For example, if local has 3 commands and docker has 2:
    /// - Index 0-2: local commands
    /// - Index 3-4: docker commands
    pub fn get_selected_command<'a>(
        &self,
        app: &'a AppStateData,
    ) -> Option<(&'a str, &'a crate::tui::state::CommandInfo)> {
        self.get_command_by_index(app, self.selected_command_idx)
    }

    /// Returns (environment, command_info) tuple for a specific command index
    /// 
    /// Commands are indexed sequentially across environments in order: local, docker, k8s
    /// For example, if local has 3 commands and docker has 2:
    /// - Index 0-2: local commands
    /// - Index 3-4: docker commands
    pub fn get_command_by_index<'a>(
        &self,
        app: &'a AppStateData,
        command_idx: usize,
    ) -> Option<(&'a str, &'a crate::tui::state::CommandInfo)> {
        let mut current_idx = 0;
        
        // Iterate through environments in order: local, docker, k8s
        for env in &["local", "docker", "k8s"] {
            if let Some(commands) = app.commands.get(*env) {
                // Check if the requested index falls within this environment's commands
                if command_idx < current_idx + commands.len() {
                    let cmd_idx = command_idx - current_idx;
                    return Some((env, &commands[cmd_idx]));
                }
                current_idx += commands.len();
            }
        }
        
        None
    }

    /// Renders the main view with all its components
    pub fn render(&self, frame: &mut Frame, state: &AppState, theme: &Theme) {
        let size = frame.area();

        // Main layout: tab bar, content area, footer
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Min(0),    // Content area
                Constraint::Length(3), // Footer with shortcuts
            ])
            .split(size);

        // Render tab bar at the top
        self.render_tab_bar(frame, main_chunks[0], theme);

        // Split content area into left (app list) and right (details) panels
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Left panel - app list
                Constraint::Percentage(70), // Right panel - details
            ])
            .split(main_chunks[1]);

        // Render left panel (project/app tree)
        self.render_app_list(frame, content_chunks[0], state, theme);

        // Render right panel based on active tab
        match self.active_tab {
            MainTab::Status => self.render_status_panel(frame, content_chunks[1], state, theme),
            MainTab::Commands => self.render_commands_panel(frame, content_chunks[1], state, theme),
            MainTab::Logs => self.render_logs_panel(frame, content_chunks[1], state, theme),
            MainTab::Config => self.render_config_panel(frame, content_chunks[1], state, theme),
        }

        // Render footer with contextual keyboard shortcuts
        self.render_footer(frame, main_chunks[2], theme);
    }

    /// Renders the tab bar at the top of the screen
    /// Shows Status, Commands, Logs, and Config tabs with the active one highlighted
    /// Visual polish: Clear tab indicators with consistent spacing
    fn render_tab_bar(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        // Tab titles with keyboard shortcuts for quick access
        // Visual consistency: Uniform formatting across all tabs
        let tab_titles = vec!["[1] Status", "[2] Commands", "[3] Logs", "[4] Config"];
        
        // Determine which tab index is active (0, 1, 2, or 3)
        let selected_idx = match self.active_tab {
            MainTab::Status => 0,
            MainTab::Commands => 1,
            MainTab::Logs => 2,
            MainTab::Config => 3,
        };

        let tabs = Tabs::new(tab_titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
            )
            .select(selected_idx)
            .style(Style::default().fg(theme.text))
            .highlight_style(
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            );

        frame.render_widget(tabs, area);
    }

    /// Renders the left panel with the project/app tree
    /// Shows projects with expand/collapse and apps with status indicators
    /// Visual polish: Clear hierarchy with consistent indentation and spacing
    fn render_app_list(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let mut lines = Vec::new();

        // Build the tree structure with visual hierarchy
        // Optimization: Pre-allocate capacity for better performance
        for (proj_idx, project) in state.projects.iter().enumerate() {
            // Project header with expansion indicator
            // Visual consistency: Unicode arrows for expand/collapse state
            let expansion_icon = if project.expanded { "▼" } else { "▶" };
            let project_line = format!("{} {}", expansion_icon, project.name);
            
            // Highlight if this project is selected and we're in the app list panel
            let is_project_selected = proj_idx == state.selected_project_idx 
                && self.focus == PanelFocus::AppList;
            
            let style = if is_project_selected {
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            
            lines.push(Line::from(Span::styled(project_line, style)));

            // Show apps if project is expanded
            // Visual polish: Indented apps show clear parent-child relationship
            if project.expanded {
                for (app_idx, app) in project.apps.iter().enumerate() {
                    let app_line = self.format_app_line(
                        app,
                        proj_idx,
                        app_idx,
                        state.selected_project_idx,
                        state.selected_app_idx,
                        theme,
                    );
                    lines.push(app_line);
                }
            }
        }

        // Create the widget with appropriate border style based on focus
        // Visual feedback: Highlighted border shows which panel is active
        let border_style = if self.focus == PanelFocus::AppList {
            Style::default().fg(theme.primary) // Highlight border when focused
        } else {
            Style::default().fg(theme.border)
        };

        let app_list = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Projects & Apps")
                    .border_style(border_style)
            );

        frame.render_widget(app_list, area);
    }

    /// Formats a single app line with status indicator and selection highlight
    /// Returns a Line with appropriate styling
    /// Visual polish: Clear status indicators with consistent spacing
    fn format_app_line<'a>(
        &self,
        app: &'a AppStateData,
        proj_idx: usize,
        app_idx: usize,
        selected_proj_idx: usize,
        selected_app_idx: usize,
        theme: &'a Theme,
    ) -> Line<'a> {
        // Status indicator: ● for running, ○ for stopped
        // Visual consistency: Unicode circles provide clear at-a-glance status
        let status_icon = if app.status.is_running() { "●" } else { "○" };
        let status_color = if app.status.is_running() {
            theme.running
        } else {
            theme.stopped
        };
        
        // Check if this app is currently selected
        let is_selected = proj_idx == selected_proj_idx 
            && app_idx == selected_app_idx
            && self.focus == PanelFocus::AppList;
        
        // Apply selection styling
        // Visual feedback: Background highlight shows current selection
        let text_style = if is_selected {
            Style::default()
                .bg(theme.selected_bg)
                .fg(theme.text)
        } else {
            Style::default().fg(theme.text)
        };
        
        // Build the line with indentation, status icon, and app name
        // Visual polish: Consistent 2-space indentation for hierarchy
        Line::from(vec![
            Span::raw("  "), // Indentation for apps under projects
            Span::styled(status_icon, Style::default().fg(status_color)),
            Span::styled(format!(" {}", app.name), text_style),
        ])
    }

    /// Renders the Status tab panel showing app details
    fn render_status_panel(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let content = if let Some(app) = state.selected_app() {
            self.build_status_content(app, theme)
        } else {
            vec![Line::from(Span::styled(
                "No app selected",
                Style::default().fg(theme.text_dim),
            ))]
        };

        let border_style = if self.focus == PanelFocus::DetailPanel {
            Style::default().fg(theme.primary)
        } else {
            Style::default().fg(theme.border)
        };

        let status_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Details")
                    .border_style(border_style)
            );

        frame.render_widget(status_panel, area);
    }

    /// Builds the content for the status panel
    /// Shows app name, status, PID, uptime, type, path, and dependencies
    fn build_status_content<'a>(&self, app: &'a AppStateData, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // App name header
        lines.push(Line::from(vec![
            Span::styled("─ ", Style::default().fg(theme.border)),
            Span::styled(
                app.name.clone(),
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ─", Style::default().fg(theme.border)),
        ]));
        lines.push(Line::from("")); // Empty line for spacing

        // Status information
        let status_text = app.status.as_str();
        let status_color = if app.status.is_running() {
            theme.running
        } else {
            theme.stopped
        };
        
        lines.push(Line::from(vec![
            Span::styled("Status:      ", Style::default().fg(theme.text_dim)),
            Span::styled("● ", Style::default().fg(status_color)),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]));

        // Additional details for running apps
        if let crate::tui::state::AppStatus::Running { pid, uptime, .. } = &app.status {
            lines.push(Line::from(vec![
                Span::styled("PID:         ", Style::default().fg(theme.text_dim)),
                Span::styled(pid.to_string(), Style::default().fg(theme.text)),
            ]));

            // Format uptime in a human-readable way
            let uptime_str = Self::format_duration(uptime);
            lines.push(Line::from(vec![
                Span::styled("Uptime:      ", Style::default().fg(theme.text_dim)),
                Span::styled(uptime_str, Style::default().fg(theme.text)),
            ]));
        }

        // App type
        lines.push(Line::from(vec![
            Span::styled("Type:        ", Style::default().fg(theme.text_dim)),
            Span::styled(app.app_type.clone(), Style::default().fg(theme.text)),
        ]));

        // Project
        lines.push(Line::from(vec![
            Span::styled("Project:     ", Style::default().fg(theme.text_dim)),
            Span::styled(app.project.clone(), Style::default().fg(theme.text)),
        ]));

        // Path (if available from app data)
        if let Some(path) = &app.path {
            lines.push(Line::from(vec![
                Span::styled("Path:        ", Style::default().fg(theme.text_dim)),
                Span::styled(path.clone(), Style::default().fg(theme.text)),
            ]));
        }

        // Dependencies section
        if !app.dependencies.is_empty() {
            lines.push(Line::from("")); // Empty line for spacing
            lines.push(Line::from(Span::styled(
                "Dependencies:",
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )));

            for dep in &app.dependencies {
                // Show dependency with a checkmark or x based on status
                // For now, we'll just show the dependency name
                // TODO: Check actual dependency status
                lines.push(Line::from(vec![
                    Span::styled("  • ", Style::default().fg(theme.text_dim)),
                    Span::styled(dep.clone(), Style::default().fg(theme.text)),
                ]));
            }
        }

        lines
    }

    /// Formats a duration into a human-readable string (e.g., "2h 15m 32s")
    pub(crate) fn format_duration(duration: &chrono::Duration) -> String {
        let total_seconds = duration.num_seconds();
        
        if total_seconds < 60 {
            format!("{}s", total_seconds)
        } else if total_seconds < 3600 {
            let minutes = total_seconds / 60;
            let seconds = total_seconds % 60;
            format!("{}m {}s", minutes, seconds)
        } else {
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;
            format!("{}h {}m {}s", hours, minutes, seconds)
        }
    }

    /// Renders the Commands tab panel showing available commands
    fn render_commands_panel(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let content = if let Some(app) = state.selected_app() {
            self.build_commands_content(app, theme)
        } else {
            vec![Line::from(Span::styled(
                "No app selected",
                Style::default().fg(theme.text_dim),
            ))]
        };

        let border_style = if self.focus == PanelFocus::DetailPanel {
            Style::default().fg(theme.primary)
        } else {
            Style::default().fg(theme.border)
        };

        let commands_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Available Commands")
                    .border_style(border_style)
            );

        frame.render_widget(commands_panel, area);
    }

    /// Builds the content for the commands panel
    /// Groups commands by environment (local, docker, k8s)
    /// Highlights the currently selected command when detail panel has focus
    fn build_commands_content<'a>(&self, app: &'a AppStateData, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // App name header
        lines.push(Line::from(vec![
            Span::styled("─ ", Style::default().fg(theme.border)),
            Span::styled(
                format!("{} commands", app.name),
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ─", Style::default().fg(theme.border)),
        ]));
        lines.push(Line::from("")); // Empty line for spacing

        if app.commands.is_empty() {
            lines.push(Line::from(Span::styled(
                "No commands configured",
                Style::default().fg(theme.text_dim),
            )));
            return lines;
        }

        // Track the global command index across all environments
        let mut global_cmd_idx = 0;

        // Display commands grouped by environment
        // Order: local, docker, k8s
        for env in &["local", "docker", "k8s"] {
            if let Some(commands) = app.commands.get(*env) {
                // Environment header
                lines.push(Line::from(Span::styled(
                    format!("{}:", env.to_uppercase()),
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )));

                // List commands in this environment
                for cmd in commands.iter() {
                    // Check if this command is selected
                    let is_selected = global_cmd_idx == self.selected_command_idx
                        && self.focus == PanelFocus::DetailPanel;
                    
                    // Choose prefix and styling based on selection
                    let (prefix, name_style, cmd_style) = if is_selected {
                        (
                            " >",
                            Style::default()
                                .fg(theme.text)
                                .bg(theme.selected_bg)
                                .add_modifier(Modifier::BOLD),
                            Style::default()
                                .fg(theme.text_dim)
                                .bg(theme.selected_bg),
                        )
                    } else {
                        (
                            "  ",
                            Style::default().fg(theme.text),
                            Style::default().fg(theme.text_dim),
                        )
                    };
                    
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.primary)),
                        Span::styled(format!(" {:<12}", cmd.name), name_style),
                        Span::styled(cmd.command.clone(), cmd_style),
                    ]));
                    
                    global_cmd_idx += 1;
                }

                lines.push(Line::from("")); // Empty line between environments
            }
        }

        lines
    }

    /// Renders the Logs tab panel showing available log files
    fn render_logs_panel(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let content = if let Some(app) = state.selected_app() {
            self.build_logs_content(app, theme)
        } else {
            vec![Line::from(Span::styled(
                "No app selected",
                Style::default().fg(theme.text_dim),
            ))]
        };

        let border_style = if self.focus == PanelFocus::DetailPanel {
            Style::default().fg(theme.primary)
        } else {
            Style::default().fg(theme.border)
        };

        let logs_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Log Files")
                    .border_style(border_style)
            );

        frame.render_widget(logs_panel, area);
    }

    /// Builds the content for the logs panel
    /// Shows available log files with name, size, and date
    fn build_logs_content<'a>(&self, app: &'a AppStateData, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // App name header
        lines.push(Line::from(vec![
            Span::styled("─ ", Style::default().fg(theme.border)),
            Span::styled(
                format!("{} logs", app.name),
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ─", Style::default().fg(theme.border)),
        ]));
        lines.push(Line::from("")); // Empty line for spacing

        // Discover log files for this app
        match self.log_manager.list_logs_for_app(&app.name) {
            Ok(log_files) => {
                if log_files.is_empty() {
                    // No log files found - show helpful message
                    lines.push(Line::from(Span::styled(
                        "No log files found for this app.",
                        Style::default().fg(theme.text_dim),
                    )));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "Log files will appear here after you start the app.",
                        Style::default().fg(theme.text_dim),
                    )));
                } else {
                    // Display log files
                    for (idx, log_file) in log_files.iter().enumerate() {
                        // Check if this log file is selected
                        let is_selected = idx == self.selected_log_idx
                            && self.focus == PanelFocus::DetailPanel;
                        
                        // Format file size and date
                        let size_str = LogManager::format_file_size(log_file.size);
                        let date_str = LogManager::format_relative_date(&log_file.modified);
                        
                        // Choose prefix and styling based on selection
                        let (prefix, name_style, meta_style) = if is_selected {
                            (
                                " >",
                                Style::default()
                                    .fg(theme.text)
                                    .bg(theme.selected_bg)
                                    .add_modifier(Modifier::BOLD),
                                Style::default()
                                    .fg(theme.text_dim)
                                    .bg(theme.selected_bg),
                            )
                        } else {
                            (
                                "  ",
                                Style::default().fg(theme.text),
                                Style::default().fg(theme.text_dim),
                            )
                        };
                        
                        // First line: prefix + filename
                        lines.push(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(theme.primary)),
                            Span::styled(format!(" {}", log_file.name), name_style),
                        ]));
                        
                        // Second line: size and date (indented)
                        lines.push(Line::from(vec![
                            Span::styled("   ", Style::default()),
                            Span::styled(format!("{}    {}", size_str, date_str), meta_style),
                        ]));
                        
                        // Empty line between log files for readability
                        if idx < log_files.len() - 1 {
                            lines.push(Line::from(""));
                        }
                    }
                }
            }
            Err(e) => {
                // Error reading log files - show error message
                lines.push(Line::from(Span::styled(
                    format!("Error reading log files: {}", e),
                    Style::default().fg(theme.error),
                )));
            }
        }

        lines
    }

    /// Renders the footer with contextual keyboard shortcuts
    /// Shows different shortcuts based on active tab and focus
    /// Visual polish: Context-aware help keeps users informed of available actions
    fn render_footer(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        // Context-aware shortcuts based on active tab
        // UX: Shows only relevant shortcuts to avoid overwhelming users
        let shortcuts = match self.active_tab {
            MainTab::Status => {
                "↑↓/jk: Navigate  ←→: Switch Panel  s: Start/Stop  r: Restart  ?: Help  q: Quit"
            }
            MainTab::Commands => {
                "↑↓/jk: Navigate  ←→: Switch Panel  Tab/1-4: Switch Tab  Enter: Execute  q: Quit"
            }
            MainTab::Logs => {
                "↑↓/jk: Navigate  ←→: Switch Panel  Tab/1-4: Switch Tab  Enter: View Log  q: Quit"
            }
            MainTab::Config => match self.config_mode {
                ConfigMode::View => {
                    "↑↓/jk: Navigate  a: Add  e: Edit Cmd  E: Edit App  s: Set Default  D: Deps  d: Delete  q: Quit"
                }
                ConfigMode::Add | ConfigMode::Edit => {
                    "Tab/↑↓: Navigate Fields  Type: Edit  Enter: Save  Esc: Cancel"
                }
                ConfigMode::EditCommand => {
                    "Tab: Switch Field  ←→: Move Cursor  Type: Edit  Enter: Save  Esc: Cancel"
                }
                ConfigMode::EditDependencies => {
                    "↑↓/jk: Navigate  a: Add  d: Delete  Esc: Close"
                }
                ConfigMode::AddDependency => {
                    "↑↓/jk: Navigate  Enter: Add  Esc: Cancel"
                }
            },
        };

        let footer = Paragraph::new(shortcuts)
            .style(Style::default().fg(theme.text_dim))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
            );

        frame.render_widget(footer, area);
    }

    /// Renders the Config tab panel
    fn render_config_panel(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        // Render base view
        match self.config_mode {
            ConfigMode::View => self.render_config_view(frame, area, state, theme),
            ConfigMode::Add | ConfigMode::Edit => self.render_config_form(frame, area, theme),
            ConfigMode::EditCommand => self.render_command_edit_form(frame, area, theme),
            ConfigMode::EditDependencies => {
                self.render_config_view(frame, area, state, theme);
                self.render_dependencies_popup(frame, state, theme);
            }
            ConfigMode::AddDependency => {
                self.render_config_view(frame, area, state, theme);
                self.render_add_dependency_popup(frame, theme);
            }
        }
    }

    /// Renders the config view mode (shows selected app's config)
    fn render_config_view(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let content = self.build_config_view_content(state, theme);

        let border_style = if self.focus == PanelFocus::DetailPanel {
            Style::default().fg(theme.primary)
        } else {
            Style::default().fg(theme.border)
        };

        let config_panel = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Configuration")
                .border_style(border_style),
        );

        frame.render_widget(config_panel, area);
    }

    /// Builds content for config view mode - shows selected app's config
    fn build_config_view_content(&self, state: &AppState, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if let Some(app) = state.selected_app() {
            // App header with selection indicator
            lines.push(Line::from(vec![
                Span::styled("─ ", Style::default().fg(theme.border)),
                Span::styled("● ", Style::default().fg(theme.primary)),
                Span::styled(
                    format!("{} Configuration", app.name),
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ─", Style::default().fg(theme.border)),
            ]));
            lines.push(Line::from(""));

            // Load full config to get all details
            match load_config() {
                Ok(config) => {
                    if let Some(project) = config.projects.get(&app.project) {
                        if let Some(full_app) = project.apps.get(&app.name) {
                            // Project
                            lines.push(Line::from(vec![
                                Span::styled("Project:     ".to_string(), Style::default().fg(theme.text_dim)),
                                Span::styled(app.project.clone(), Style::default().fg(theme.text)),
                            ]));

                            // App Type
                            lines.push(Line::from(vec![
                                Span::styled("Type:        ".to_string(), Style::default().fg(theme.text_dim)),
                                Span::styled(full_app.app_type.clone(), Style::default().fg(theme.text)),
                            ]));

                            // Path
                            lines.push(Line::from(vec![
                                Span::styled("Path:        ".to_string(), Style::default().fg(theme.text_dim)),
                                Span::styled(full_app.path.clone(), Style::default().fg(theme.text)),
                            ]));

                            lines.push(Line::from(""));

                            // Commands section
                            lines.push(Line::from(Span::styled(
                                "Commands:".to_string(),
                                Style::default()
                                    .fg(theme.secondary)
                                    .add_modifier(Modifier::BOLD),
                            )));

                            // Track global command index for selection
                            let mut global_cmd_idx = 0;

                            // Local commands - sorted alphabetically
                            if let Some(local) = &full_app.commands.local {
                                lines.push(Line::from(Span::styled(
                                    "  LOCAL:".to_string(),
                                    Style::default().fg(theme.text_dim),
                                )));
                                let mut sorted_cmds: Vec<_> = local.iter().collect();
                                sorted_cmds.sort_by_key(|(name, _)| *name);
                                let is_default = full_app.defaults.local.as_ref();
                                for (name, cmd) in sorted_cmds {
                                    let is_selected = global_cmd_idx == self.selected_config_command_idx
                                        && self.focus == PanelFocus::DetailPanel;
                                    let is_this_default = is_default == Some(name);
                                    
                                    let (prefix, name_style, cmd_style) = if is_selected {
                                        (
                                            " >",
                                            Style::default().fg(theme.text).bg(theme.selected_bg).add_modifier(Modifier::BOLD),
                                            Style::default().fg(theme.text_dim).bg(theme.selected_bg),
                                        )
                                    } else {
                                        (
                                            "  ",
                                            Style::default().fg(theme.text),
                                            Style::default().fg(theme.text_dim),
                                        )
                                    };
                                    
                                    let mut spans = vec![
                                        Span::styled(prefix.to_string(), Style::default().fg(theme.primary)),
                                        Span::styled(" ".to_string(), Style::default()),
                                        Span::styled(format!("{:<15}", name), name_style),
                                    ];
                                    
                                    if is_this_default {
                                        spans.push(Span::styled(" [default]".to_string(), Style::default().fg(theme.success)));
                                    }
                                    
                                    spans.push(Span::styled("  ".to_string(), Style::default()));
                                    spans.push(Span::styled(cmd.clone(), cmd_style));
                                    
                                    lines.push(Line::from(spans));
                                    global_cmd_idx += 1;
                                }
                            }

                            // Docker commands - sorted alphabetically
                            if let Some(docker) = &full_app.commands.docker {
                                lines.push(Line::from(Span::styled(
                                    "  DOCKER:".to_string(),
                                    Style::default().fg(theme.text_dim),
                                )));
                                let mut sorted_cmds: Vec<_> = docker.iter().collect();
                                sorted_cmds.sort_by_key(|(name, _)| *name);
                                let is_default = full_app.defaults.docker.as_ref();
                                for (name, cmd) in sorted_cmds {
                                    let is_selected = global_cmd_idx == self.selected_config_command_idx
                                        && self.focus == PanelFocus::DetailPanel;
                                    let is_this_default = is_default == Some(name);
                                    
                                    let (prefix, name_style, cmd_style) = if is_selected {
                                        (
                                            " >",
                                            Style::default().fg(theme.text).bg(theme.selected_bg).add_modifier(Modifier::BOLD),
                                            Style::default().fg(theme.text_dim).bg(theme.selected_bg),
                                        )
                                    } else {
                                        (
                                            "  ",
                                            Style::default().fg(theme.text),
                                            Style::default().fg(theme.text_dim),
                                        )
                                    };
                                    
                                    let mut spans = vec![
                                        Span::styled(prefix.to_string(), Style::default().fg(theme.primary)),
                                        Span::styled(" ".to_string(), Style::default()),
                                        Span::styled(format!("{:<15}", name), name_style),
                                    ];
                                    
                                    if is_this_default {
                                        spans.push(Span::styled(" [default]".to_string(), Style::default().fg(theme.success)));
                                    }
                                    
                                    spans.push(Span::styled("  ".to_string(), Style::default()));
                                    spans.push(Span::styled(cmd.clone(), cmd_style));
                                    
                                    lines.push(Line::from(spans));
                                    global_cmd_idx += 1;
                                }
                            }

                            // K8s commands - sorted alphabetically
                            if let Some(k8s) = &full_app.commands.k8s {
                                lines.push(Line::from(Span::styled(
                                    "  K8S:".to_string(),
                                    Style::default().fg(theme.text_dim),
                                )));
                                let mut sorted_cmds: Vec<_> = k8s.iter().collect();
                                sorted_cmds.sort_by_key(|(name, _)| *name);
                                let is_default = full_app.defaults.k8s.as_ref();
                                for (name, cmd) in sorted_cmds {
                                    let is_selected = global_cmd_idx == self.selected_config_command_idx
                                        && self.focus == PanelFocus::DetailPanel;
                                    let is_this_default = is_default == Some(name);
                                    
                                    let (prefix, name_style, cmd_style) = if is_selected {
                                        (
                                            " >",
                                            Style::default().fg(theme.text).bg(theme.selected_bg).add_modifier(Modifier::BOLD),
                                            Style::default().fg(theme.text_dim).bg(theme.selected_bg),
                                        )
                                    } else {
                                        (
                                            "  ",
                                            Style::default().fg(theme.text),
                                            Style::default().fg(theme.text_dim),
                                        )
                                    };
                                    
                                    let mut spans = vec![
                                        Span::styled(prefix.to_string(), Style::default().fg(theme.primary)),
                                        Span::styled(" ".to_string(), Style::default()),
                                        Span::styled(format!("{:<15}", name), name_style),
                                    ];
                                    
                                    if is_this_default {
                                        spans.push(Span::styled(" [default]".to_string(), Style::default().fg(theme.success)));
                                    }
                                    
                                    spans.push(Span::styled("  ".to_string(), Style::default()));
                                    spans.push(Span::styled(cmd.clone(), cmd_style));
                                    
                                    lines.push(Line::from(spans));
                                    global_cmd_idx += 1;
                                }
                            }

                            lines.push(Line::from(""));

                            // Dependencies
                            if !full_app.dependencies.is_empty() {
                                lines.push(Line::from(Span::styled(
                                    "Dependencies:".to_string(),
                                    Style::default()
                                        .fg(theme.secondary)
                                        .add_modifier(Modifier::BOLD),
                                )));
                                for dep in &full_app.dependencies {
                                    lines.push(Line::from(vec![
                                        Span::styled("  • ".to_string(), Style::default().fg(theme.text_dim)),
                                        Span::styled(format!("{}/{}", dep.project, dep.app), Style::default().fg(theme.text)),
                                    ]));
                                }
                                lines.push(Line::from(""));
                            }

                            // Actions
                            lines.push(Line::from(""));
                            lines.push(Line::from(Span::styled(
                                "Actions:",
                                Style::default()
                                    .fg(theme.secondary)
                                    .add_modifier(Modifier::BOLD),
                            )));
                            lines.push(Line::from(vec![
                                Span::styled("  a", Style::default().fg(theme.primary)),
                                Span::styled(" - Add new app", Style::default().fg(theme.text)),
                            ]));
                            lines.push(Line::from(vec![
                                Span::styled("  e", Style::default().fg(theme.primary)),
                                Span::styled(" - Edit this app", Style::default().fg(theme.text)),
                            ]));
                            lines.push(Line::from(vec![
                                Span::styled("  d", Style::default().fg(theme.primary)),
                                Span::styled(" - Delete this app", Style::default().fg(theme.text)),
                            ]));
                        }
                    }
                }
                Err(e) => {
                    lines.push(Line::from(Span::styled(
                        format!("Error loading config: {}", e),
                        Style::default().fg(theme.error),
                    )));
                }
            }
        } else {
            // No app selected
            lines.push(Line::from(vec![
                Span::styled("─ ", Style::default().fg(theme.border)),
                Span::styled(
                    "Configuration",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ─", Style::default().fg(theme.border)),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Select an app from the left panel to view its configuration.",
                Style::default().fg(theme.text_dim),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Or press 'a' to add a new app.",
                Style::default().fg(theme.text_dim),
            )));
        }

        lines
    }

    /// Renders the config form (add/edit mode)
    fn render_config_form(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let content = self.build_config_form_content(theme);

        let title = match self.config_mode {
            ConfigMode::Add => "Add New App",
            ConfigMode::Edit => "Edit App",
            _ => "Config",
        };

        let config_panel = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(theme.primary)),
        );

        frame.render_widget(config_panel, area);
    }

    /// Renders the command edit form
    fn render_command_edit_form(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("─ ", Style::default().fg(theme.border)),
                Span::styled(
                    "Edit Command",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ─", Style::default().fg(theme.border)),
            ]),
            Line::from(""),
        ];

        // Environment (read-only)
        lines.push(Line::from(vec![
            Span::styled("  Environment: ", Style::default().fg(theme.text_dim)),
            Span::styled(
                self.config_form.edit_command_env.to_uppercase(),
                Style::default().fg(theme.text),
            ),
        ]));

        lines.push(Line::from(""));

        // Command name (editable)
        let is_name_focused = self.config_focused_field == ConfigField::EditCommandName;
        let name_label_style = if is_name_focused {
            Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_dim)
        };
        lines.push(Line::from(Span::styled("  Command Name:", name_label_style)));
        
        let mut name_spans = vec![Span::styled("  ", Style::default())];
        let name = &self.config_form.edit_command_name;
        let name_cursor = self.config_form.cursor_edit_command_name;
        
        if is_name_focused {
            if !name.is_empty() {
                let before = name.chars().take(name_cursor).collect::<String>();
                let after = name.chars().skip(name_cursor).collect::<String>();
                
                if !before.is_empty() {
                    name_spans.push(Span::styled(before, Style::default().fg(theme.text).bg(theme.selected_bg)));
                }
                name_spans.push(Span::styled("█", Style::default().fg(theme.primary)));
                if !after.is_empty() {
                    name_spans.push(Span::styled(after, Style::default().fg(theme.text).bg(theme.selected_bg)));
                }
            } else {
                name_spans.push(Span::styled("█", Style::default().fg(theme.primary)));
            }
        } else {
            name_spans.push(Span::styled(name.clone(), Style::default().fg(theme.text)));
        }
        lines.push(Line::from(name_spans));
        lines.push(Line::from(""));

        // Command value (editable)
        let is_value_focused = self.config_focused_field == ConfigField::EditCommandValue;
        let value_label_style = if is_value_focused {
            Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_dim)
        };
        lines.push(Line::from(Span::styled("  Command Value:", value_label_style)));
        
        let mut value_spans = vec![Span::styled("  ", Style::default())];
        let value = &self.config_form.edit_command_value;
        let value_cursor = self.config_form.cursor_edit_command_value;
        
        if is_value_focused {
            if !value.is_empty() {
                let before = value.chars().take(value_cursor).collect::<String>();
                let after = value.chars().skip(value_cursor).collect::<String>();
                
                if !before.is_empty() {
                    value_spans.push(Span::styled(before, Style::default().fg(theme.text).bg(theme.selected_bg)));
                }
                value_spans.push(Span::styled("█", Style::default().fg(theme.primary)));
                if !after.is_empty() {
                    value_spans.push(Span::styled(after, Style::default().fg(theme.text).bg(theme.selected_bg)));
                }
            } else {
                value_spans.push(Span::styled("█", Style::default().fg(theme.primary)));
            }
        } else {
            value_spans.push(Span::styled(value.clone(), Style::default().fg(theme.text)));
        }
        lines.push(Line::from(value_spans));

        let config_panel = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Edit Command")
                .border_style(Style::default().fg(theme.primary)),
        );

        frame.render_widget(config_panel, area);
    }

    /// Builds content for config form
    fn build_config_form_content<'a>(&self, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        lines.push(Line::from(""));

        // Helper to render a field with cursor
        let render_field = |label: &str,
                            value: String,
                            cursor_pos: usize,
                            field: ConfigField,
                            focused: ConfigField,
                            theme: &'a Theme| -> Vec<Line<'a>> {
            let is_focused = field == focused;
            let label_style = if is_focused {
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_dim)
            };
            let value_style = if is_focused {
                Style::default().fg(theme.text).bg(theme.selected_bg)
            } else {
                Style::default().fg(theme.text)
            };

            let mut spans = vec![Span::styled("  ", Style::default())];
            
            if is_focused && !value.is_empty() {
                // Split at cursor position
                let before = value.chars().take(cursor_pos).collect::<String>();
                let after = value.chars().skip(cursor_pos).collect::<String>();
                
                if !before.is_empty() {
                    spans.push(Span::styled(before, value_style));
                }
                spans.push(Span::styled("█", Style::default().fg(theme.primary)));
                if !after.is_empty() {
                    spans.push(Span::styled(after, value_style));
                }
            } else if is_focused && value.is_empty() {
                spans.push(Span::styled("█", Style::default().fg(theme.primary)));
            } else {
                spans.push(Span::styled(
                    if value.is_empty() { " ".to_string() } else { value },
                    value_style,
                ));
            }

            vec![
                Line::from(Span::styled(format!("  {}", label), label_style)),
                Line::from(spans),
                Line::from(""),
            ]
        };

        lines.extend(render_field(
            "Project Name:",
            self.config_form.project_name.clone(),
            self.config_form.cursor_project_name,
            ConfigField::ProjectName,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "App Name:",
            self.config_form.app_name.clone(),
            self.config_form.cursor_app_name,
            ConfigField::AppName,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "App Type (e.g., nodejs, python):",
            self.config_form.app_type.clone(),
            self.config_form.cursor_app_type,
            ConfigField::AppType,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "Path:",
            self.config_form.path.clone(),
            self.config_form.cursor_path,
            ConfigField::Path,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "Local Start Command (optional):",
            self.config_form.local_start_cmd.clone(),
            self.config_form.cursor_local_start_cmd,
            ConfigField::LocalStartCmd,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "Docker Start Command (optional):",
            self.config_form.docker_start_cmd.clone(),
            self.config_form.cursor_docker_start_cmd,
            ConfigField::DockerStartCmd,
            self.config_focused_field,
            theme,
        ));

        lines
    }

    /// Renders the dependencies popup
    fn render_dependencies_popup(&self, frame: &mut Frame, state: &AppState, theme: &Theme) {
        let area = Self::centered_rect(60, 50, frame.area());

        // Clear the area first for better visibility
        frame.render_widget(ratatui::widgets::Clear, area);

        // Background
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.primary))
            .title("Edit Dependencies")
            .style(Style::default().bg(theme.bg));
        frame.render_widget(block, area);

        // Content
        let inner = Rect {
            x: area.x + 2,
            y: area.y + 2,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(4),
        };

        let mut lines = Vec::new();

        if let Some(app) = state.selected_app() {
            if app.dependencies.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No dependencies configured.",
                    Style::default().fg(theme.text_dim),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Press 'a' to add a dependency.",
                    Style::default().fg(theme.text_dim),
                )));
            } else {
                for (idx, dep) in app.dependencies.iter().enumerate() {
                    let is_selected = idx == self.selected_dependency_idx;
                    let style = if is_selected {
                        Style::default().bg(theme.selected_bg).fg(theme.text).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    };

                    let prefix = if is_selected { " > " } else { "   " };
                    lines.push(Line::from(vec![
                        Span::styled(prefix.to_string(), Style::default().fg(theme.primary)),
                        Span::styled(dep.clone(), style),
                    ]));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "a: Add  d: Delete  Esc: Close",
                Style::default().fg(theme.text_dim),
            )));
        }

        let content = Paragraph::new(lines);
        frame.render_widget(content, inner);
    }

    /// Renders the add dependency popup
    fn render_add_dependency_popup(&self, frame: &mut Frame, theme: &Theme) {
        let area = Self::centered_rect(70, 60, frame.area());

        // Clear the area first for better visibility
        frame.render_widget(ratatui::widgets::Clear, area);

        // Background
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.primary))
            .title("Add Dependency - Select Project and App")
            .style(Style::default().bg(theme.bg));
        frame.render_widget(block, area);

        // Content
        let inner = Rect {
            x: area.x + 2,
            y: area.y + 2,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(4),
        };

        let mut lines = Vec::new();

        match load_config() {
            Ok(config) => {
                let projects: Vec<_> = config.projects.iter().collect();
                
                for (proj_idx, (proj_name, project)) in projects.iter().enumerate() {
                    let is_selected_project = proj_idx == self.selected_add_dep_project_idx;
                    let proj_style = if is_selected_project {
                        Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    };

                    lines.push(Line::from(Span::styled(
                        format!("▼ {}", proj_name),
                        proj_style,
                    )));

                    if is_selected_project {
                        let apps: Vec<_> = project.apps.keys().collect();
                        for (app_idx, app_name) in apps.iter().enumerate() {
                            let is_selected_app = app_idx == self.selected_add_dep_app_idx;
                            let style = if is_selected_app {
                                Style::default().bg(theme.selected_bg).fg(theme.text).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(theme.text)
                            };

                            let prefix = if is_selected_app { " > " } else { "   " };
                            lines.push(Line::from(vec![
                                Span::styled(prefix, Style::default().fg(theme.primary)),
                                Span::styled(format!("● {}", app_name), style),
                            ]));
                        }
                    }

                    lines.push(Line::from(""));
                }

                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "↑↓: Navigate  Enter: Add  Esc: Cancel",
                    Style::default().fg(theme.text_dim),
                )));
            }
            Err(e) => {
                lines.push(Line::from(Span::styled(
                    format!("Error loading config: {}", e),
                    Style::default().fg(theme.error),
                )));
            }
        }

        let content = Paragraph::new(lines);
        frame.render_widget(content, inner);
    }

    /// Helper to create a centered rect
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

}

impl Default for MainView {
    fn default() -> Self {
        Self::new()
    }
}
