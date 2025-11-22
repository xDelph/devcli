// Input handling logic for MainView
// Handles all keyboard input and user interactions

use super::{ConfigField, ConfigForm, ConfigMode, MainTab, MainView, PanelFocus};
use crate::config::models::Environment;
use crate::tui::state::AppState;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::{Arc, Mutex};

impl MainView {
    /// Handles keyboard input for the main view
    /// Returns true if the event was handled, false otherwise
    pub fn handle_input(&mut self, key: KeyEvent, state: &Arc<Mutex<AppState>>) -> Result<bool> {
        // Lock the state for modification
        // Use expect instead of context since PoisonError doesn't implement StdError
        let mut state = state.lock().expect("Failed to lock state");
        
        // Handle delete confirmation mode
        if self.config_mode == ConfigMode::ConfirmDelete {
            return self.handle_delete_confirmation(key, &mut state);
        }
        
        // In edit mode, only handle specific keys - everything else is for typing
        let in_edit_mode = self.active_tab == MainTab::Config && matches!(
            self.config_mode,
            ConfigMode::Add | ConfigMode::Edit | ConfigMode::AddCommand | ConfigMode::EditCommand | ConfigMode::AddEnvFile | ConfigMode::EditEnvFile
        );
        
        if in_edit_mode {
            return self.handle_edit_mode_input(key, &mut state);
        }
        
        // Normal mode key handling
        self.handle_normal_mode_input(key, &mut state)
    }

    /// Handles input when in edit mode (config forms)
    fn handle_edit_mode_input(&mut self, key: KeyEvent, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                // Handle ESC based on current mode to return to the right parent view
                match self.config_mode {
                    ConfigMode::AddEnvFile | ConfigMode::EditEnvFile => {
                        self.config_mode = ConfigMode::EditEnvFiles;
                    }
                    ConfigMode::AddDependency => {
                        self.config_mode = ConfigMode::EditDependencies;
                    }
                    _ => {
                        self.config_mode = ConfigMode::View;
                    }
                }
                Ok(true)
            }
            KeyCode::Enter => self.handle_edit_mode_enter(state),
            KeyCode::Tab => {
                self.cycle_form_field();
                Ok(true)
            }
            KeyCode::Up => {
                if matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
                    self.move_form_field_up();
                }
                Ok(true)
            }
            KeyCode::Down => {
                if matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
                    self.move_form_field_down();
                }
                Ok(true)
            }
            KeyCode::Left => {
                self.move_cursor_left();
                Ok(true)
            }
            KeyCode::Right => {
                self.move_cursor_right();
                Ok(true)
            }
            KeyCode::Backspace => {
                self.remove_char_from_field();
                Ok(true)
            }
            KeyCode::Char(c) => {
                // Special handling for environment field in AddCommand mode
                if self.config_mode == ConfigMode::AddCommand && self.config_focused_field == ConfigField::EditCommandEnv && c == ' ' {
                    self.cycle_environment();
                    return Ok(true);
                }
                self.add_char_to_field(c);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Handles Enter key in edit mode
    fn handle_edit_mode_enter(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        match self.config_mode {
            ConfigMode::Add | ConfigMode::Edit => {
                if let Err(e) = self.save_config_form() {
                    eprintln!("Error saving config: {}", e);
                } else {
                    // Reload state after saving
                    if let Err(e) = self.reload_state_from_config(state) {
                        eprintln!("Error reloading state: {}", e);
                    }
                }
                Ok(true)
            }
            ConfigMode::AddCommand => {
                if let Some(app) = state.selected_app() {
                    let project = app.project.clone();
                    let app_name = app.name.clone();
                    
                    if let Err(e) = self.save_new_command(&project, &app_name) {
                        eprintln!("Error saving command: {}", e);
                        return Ok(true);
                    }
                    
                    if let Err(e) = self.reload_state_from_config(state) {
                        eprintln!("Error reloading state: {}", e);
                    }
                }
                Ok(true)
            }
            ConfigMode::EditCommand => {
                if let Some(app) = state.selected_app() {
                    let project = app.project.clone();
                    let app_name = app.name.clone();
                    
                    if let Err(e) = self.save_command_edit(&project, &app_name) {
                        eprintln!("Error saving command: {}", e);
                        return Ok(true);
                    }
                    
                    if let Err(e) = self.reload_state_from_config(state) {
                        eprintln!("Error reloading state: {}", e);
                    }
                }
                Ok(true)
            }
            ConfigMode::AddEnvFile | ConfigMode::EditEnvFile => {
                if let Some(app) = state.selected_app() {
                    let app_name = app.name.clone();
                    
                    if let Err(e) = self.save_env_file(&app_name) {
                        eprintln!("Error saving env file: {}", e);
                        return Ok(true);
                    }
                    
                    if let Err(e) = self.reload_state_from_config(state) {
                        eprintln!("Error reloading state: {}", e);
                    }
                }
                Ok(true)
            }
            _ => Ok(false)
        }
    }

    /// Cycles through form fields with Tab key
    fn cycle_form_field(&mut self) {
        if self.config_mode == ConfigMode::AddCommand {
            self.config_focused_field = match self.config_focused_field {
                ConfigField::EditCommandEnv => ConfigField::EditCommandName,
                ConfigField::EditCommandName => ConfigField::EditCommandValue,
                ConfigField::EditCommandValue => ConfigField::EditCommandEnv,
                _ => ConfigField::EditCommandEnv,
            };
        } else if self.config_mode == ConfigMode::EditCommand {
            self.config_focused_field = match self.config_focused_field {
                ConfigField::EditCommandName => ConfigField::EditCommandValue,
                ConfigField::EditCommandValue => ConfigField::EditCommandName,
                _ => ConfigField::EditCommandName,
            };
        } else if self.config_mode == ConfigMode::AddEnvFile || self.config_mode == ConfigMode::EditEnvFile {
            self.config_focused_field = match self.config_focused_field {
                ConfigField::EnvFileStage => ConfigField::EnvFileContext,
                ConfigField::EnvFileContext => ConfigField::EnvFilePath,
                ConfigField::EnvFilePath => ConfigField::EnvFileStage,
                _ => ConfigField::EnvFileStage,
            };
        } else if matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
            self.config_focused_field = match self.config_focused_field {
                ConfigField::ProjectName => ConfigField::AppName,
                ConfigField::AppName => ConfigField::AppType,
                ConfigField::AppType => ConfigField::Path,
                ConfigField::Path => ConfigField::LocalStartCmd,
                ConfigField::LocalStartCmd => ConfigField::DockerStartCmd,
                ConfigField::DockerStartCmd => ConfigField::ProjectName,
                _ => ConfigField::ProjectName,
            };
        }
    }

    /// Moves form field focus up
    fn move_form_field_up(&mut self) {
        if matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
            self.config_focused_field = match self.config_focused_field {
                ConfigField::ProjectName => ConfigField::DockerStartCmd,
                ConfigField::AppName => ConfigField::ProjectName,
                ConfigField::AppType => ConfigField::AppName,
                ConfigField::Path => ConfigField::AppType,
                ConfigField::LocalStartCmd => ConfigField::Path,
                ConfigField::DockerStartCmd => ConfigField::LocalStartCmd,
                _ => ConfigField::ProjectName,
            };
        }
    }

    /// Moves form field focus down
    fn move_form_field_down(&mut self) {
        if matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
            self.config_focused_field = match self.config_focused_field {
                ConfigField::ProjectName => ConfigField::AppName,
                ConfigField::AppName => ConfigField::AppType,
                ConfigField::AppType => ConfigField::Path,
                ConfigField::Path => ConfigField::LocalStartCmd,
                ConfigField::LocalStartCmd => ConfigField::DockerStartCmd,
                ConfigField::DockerStartCmd => ConfigField::ProjectName,
                _ => ConfigField::ProjectName,
            };
        }
    }

    /// Adds a character to the currently focused field
    pub(super) fn add_char_to_field(&mut self, c: char) {
        let (field, cursor) = match self.config_focused_field {
            ConfigField::ProjectName => (&mut self.config_form.project_name, &mut self.config_form.cursor_project_name),
            ConfigField::AppName => (&mut self.config_form.app_name, &mut self.config_form.cursor_app_name),
            ConfigField::AppType => (&mut self.config_form.app_type, &mut self.config_form.cursor_app_type),
            ConfigField::Path => (&mut self.config_form.path, &mut self.config_form.cursor_path),
            ConfigField::LocalStartCmd => (&mut self.config_form.local_start_cmd, &mut self.config_form.cursor_local_start_cmd),
            ConfigField::DockerStartCmd => (&mut self.config_form.docker_start_cmd, &mut self.config_form.cursor_docker_start_cmd),
            ConfigField::EditCommandEnv => return, // No typing for dropdown
            ConfigField::EditCommandName => (&mut self.config_form.edit_command_name, &mut self.config_form.cursor_edit_command_name),
            ConfigField::EditCommandValue => (&mut self.config_form.edit_command_value, &mut self.config_form.cursor_edit_command_value),
            ConfigField::EnvFileStage => (&mut self.config_form.env_file_stage, &mut self.config_form.cursor_env_file_stage),
            ConfigField::EnvFileContext => (&mut self.config_form.env_file_context, &mut self.config_form.cursor_env_file_context),
            ConfigField::EnvFilePath => (&mut self.config_form.env_file_path, &mut self.config_form.cursor_env_file_path),
        };
        field.insert(*cursor, c);
        *cursor += 1;
    }

    /// Removes a character from the currently focused field
    pub(super) fn remove_char_from_field(&mut self) {
        let (field, cursor) = match self.config_focused_field {
            ConfigField::ProjectName => (&mut self.config_form.project_name, &mut self.config_form.cursor_project_name),
            ConfigField::AppName => (&mut self.config_form.app_name, &mut self.config_form.cursor_app_name),
            ConfigField::AppType => (&mut self.config_form.app_type, &mut self.config_form.cursor_app_type),
            ConfigField::Path => (&mut self.config_form.path, &mut self.config_form.cursor_path),
            ConfigField::LocalStartCmd => (&mut self.config_form.local_start_cmd, &mut self.config_form.cursor_local_start_cmd),
            ConfigField::DockerStartCmd => (&mut self.config_form.docker_start_cmd, &mut self.config_form.cursor_docker_start_cmd),
            ConfigField::EditCommandEnv => return, // No editing for dropdown
            ConfigField::EditCommandName => (&mut self.config_form.edit_command_name, &mut self.config_form.cursor_edit_command_name),
            ConfigField::EditCommandValue => (&mut self.config_form.edit_command_value, &mut self.config_form.cursor_edit_command_value),
            ConfigField::EnvFileStage => (&mut self.config_form.env_file_stage, &mut self.config_form.cursor_env_file_stage),
            ConfigField::EnvFileContext => (&mut self.config_form.env_file_context, &mut self.config_form.cursor_env_file_context),
            ConfigField::EnvFilePath => (&mut self.config_form.env_file_path, &mut self.config_form.cursor_env_file_path),
        };
        if *cursor > 0 {
            *cursor -= 1;
            field.remove(*cursor);
        }
    }

    /// Moves cursor left in the current field
    pub(super) fn move_cursor_left(&mut self) {
        let cursor = match self.config_focused_field {
            ConfigField::ProjectName => &mut self.config_form.cursor_project_name,
            ConfigField::AppName => &mut self.config_form.cursor_app_name,
            ConfigField::AppType => &mut self.config_form.cursor_app_type,
            ConfigField::Path => &mut self.config_form.cursor_path,
            ConfigField::LocalStartCmd => &mut self.config_form.cursor_local_start_cmd,
            ConfigField::DockerStartCmd => &mut self.config_form.cursor_docker_start_cmd,
            ConfigField::EditCommandName => &mut self.config_form.cursor_edit_command_name,
            ConfigField::EditCommandValue => &mut self.config_form.cursor_edit_command_value,
            ConfigField::EditCommandEnv => return, // No cursor for dropdown
            ConfigField::EnvFileStage => &mut self.config_form.cursor_env_file_stage,
            ConfigField::EnvFileContext => &mut self.config_form.cursor_env_file_context,
            ConfigField::EnvFilePath => &mut self.config_form.cursor_env_file_path,
        };
        *cursor = cursor.saturating_sub(1);
    }

    /// Moves cursor right in the current field
    pub(super) fn move_cursor_right(&mut self) {
        let (field, cursor) = match self.config_focused_field {
            ConfigField::ProjectName => (&self.config_form.project_name, &mut self.config_form.cursor_project_name),
            ConfigField::AppName => (&self.config_form.app_name, &mut self.config_form.cursor_app_name),
            ConfigField::AppType => (&self.config_form.app_type, &mut self.config_form.cursor_app_type),
            ConfigField::Path => (&self.config_form.path, &mut self.config_form.cursor_path),
            ConfigField::LocalStartCmd => (&self.config_form.local_start_cmd, &mut self.config_form.cursor_local_start_cmd),
            ConfigField::DockerStartCmd => (&self.config_form.docker_start_cmd, &mut self.config_form.cursor_docker_start_cmd),
            ConfigField::EditCommandName => (&self.config_form.edit_command_name, &mut self.config_form.cursor_edit_command_name),
            ConfigField::EditCommandValue => (&self.config_form.edit_command_value, &mut self.config_form.cursor_edit_command_value),
            ConfigField::EditCommandEnv => return, // No cursor for dropdown
            ConfigField::EnvFileStage => (&self.config_form.env_file_stage, &mut self.config_form.cursor_env_file_stage),
            ConfigField::EnvFileContext => (&self.config_form.env_file_context, &mut self.config_form.cursor_env_file_context),
            ConfigField::EnvFilePath => (&self.config_form.env_file_path, &mut self.config_form.cursor_env_file_path),
        };
        if *cursor < field.len() {
            *cursor += 1;
        }
    }

    /// Cycles through available environments for AddCommand mode
    pub(super) fn cycle_environment(&mut self) {
        let envs = Environment::all();
        let current = Environment::from_string(&self.config_form.edit_command_env);
        
        if let Some(current_env) = current {
            if let Some(pos) = envs.iter().position(|e| e == &current_env) {
                let next_pos = (pos + 1) % envs.len();
                self.config_form.edit_command_env = envs[next_pos].as_str().to_string();
                return;
            }
        }
        
        // Default to first environment if not found
        self.config_form.edit_command_env = envs[0].as_str().to_string();
    }
}

impl MainView {
    /// Handles input in normal (non-edit) mode
    fn handle_normal_mode_input(&mut self, key: KeyEvent, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        match key.code {
            // Tab switching with number keys (1-4)
            KeyCode::Char('1') | KeyCode::Char('&') => {
                self.switch_to_tab(MainTab::Status);
                Ok(true)
            }
            KeyCode::Char('2') | KeyCode::Char('é') => {
                self.switch_to_tab(MainTab::Commands);
                Ok(true)
            }
            KeyCode::Char('3') | KeyCode::Char('"') => {
                self.switch_to_tab(MainTab::Logs);
                Ok(true)
            }
            KeyCode::Char('4') | KeyCode::Char('\'') => {
                self.switch_to_tab(MainTab::Config);
                Ok(true)
            }
            // Tab key to cycle through tabs
            KeyCode::Tab => {
                self.cycle_tab();
                Ok(true)
            }
            // Arrow keys for panel switching
            KeyCode::Left | KeyCode::Right => {
                self.toggle_panel_focus();
                Ok(true)
            }
            // Navigation with Shift modifier
            KeyCode::Up if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                self.handle_shift_up(state);
                Ok(true)
            }
            KeyCode::Down if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                self.handle_shift_down(state);
                Ok(true)
            }
            // Regular navigation
            KeyCode::Up | KeyCode::Char('k') => {
                self.handle_up(state);
                Ok(true)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.handle_down(state);
                Ok(true)
            }
            // Space to expand/collapse projects
            KeyCode::Char(' ') if self.focus == PanelFocus::AppList => {
                state.toggle_project_expansion();
                Ok(true)
            }
            // Enter key actions
            KeyCode::Enter => self.handle_enter(state),
            // Env files view actions (must come BEFORE general config tab keys for proper matching)
            KeyCode::Char('a') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditEnvFiles => {
                self.config_mode = ConfigMode::AddEnvFile;
                self.config_form.env_file_stage.clear();
                self.config_form.env_file_context.clear();
                self.config_form.env_file_path.clear();
                self.config_form.cursor_env_file_stage = 0;
                self.config_form.cursor_env_file_context = 0;
                self.config_form.cursor_env_file_path = 0;
                self.config_focused_field = ConfigField::EnvFileStage;
                Ok(true)
            }
            KeyCode::Char('e') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditEnvFiles => {
                self.handle_edit_env_file(state)
            }
            KeyCode::Char('d') if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditEnvFiles => {
                self.handle_delete_env_file(state)
            }
            // Config tab specific keys (general patterns come after specific EditEnvFiles patterns)
            KeyCode::Char('a') if self.active_tab == MainTab::Config => self.handle_config_add(state),
            KeyCode::Char('e') if self.active_tab == MainTab::Config => self.handle_config_edit(state),
            KeyCode::Char('f') if self.active_tab == MainTab::Config => self.handle_config_edit_env_files(state),
            KeyCode::Char('E') if self.active_tab == MainTab::Config => self.handle_config_edit_app(state),
            KeyCode::Char('D') if self.active_tab == MainTab::Config => self.handle_config_edit_deps(),
            KeyCode::Char('s') if self.active_tab == MainTab::Config => self.handle_config_set_default(state),
            KeyCode::Char('d') if self.active_tab == MainTab::Config => self.handle_config_delete(state),
            // Dependencies popup actions
            KeyCode::Esc if self.active_tab == MainTab::Config => self.handle_config_escape(),
            // Status tab specific keys
            KeyCode::Char('s') if self.active_tab == MainTab::Status => self.handle_status_start_stop(state),
            KeyCode::Char('r') if self.active_tab == MainTab::Status => self.handle_status_restart(state),
            _ => Ok(false), // Event not handled
        }
    }

    fn switch_to_tab(&mut self, tab: MainTab) {
        self.active_tab = tab;
        self.detail_scroll = 0;
        self.selected_command_idx = 0;
        if self.active_tab == MainTab::Logs {
            self.selected_log_idx = 0;
        }
        if self.active_tab == MainTab::Config {
            self.config_mode = ConfigMode::View;
            self.selected_config_command_idx = 0;
        }
    }

    fn cycle_tab(&mut self) {
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

    fn toggle_panel_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::AppList => PanelFocus::DetailPanel,
            PanelFocus::DetailPanel => PanelFocus::AppList,
        };
    }

    fn handle_shift_up(&mut self, state: &mut std::sync::MutexGuard<AppState>) {
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
    }

    fn handle_shift_down(&mut self, state: &mut std::sync::MutexGuard<AppState>) {
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
    }

    fn handle_up(&mut self, state: &mut std::sync::MutexGuard<AppState>) {
        // Dependencies popup navigation
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditDependencies {
            self.selected_dependency_idx = self.selected_dependency_idx.saturating_sub(1);
            return;
        }
        // Add dependency navigation
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::AddDependency {
            self.selected_add_dep_project_idx = self.selected_add_dep_project_idx.saturating_sub(1);
            self.selected_add_dep_app_idx = 0;
            return;
        }
        // Env files popup navigation
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditEnvFiles {
            self.selected_env_file_idx = self.selected_env_file_idx.saturating_sub(1);
            return;
        }
        
        match self.focus {
            PanelFocus::AppList => {
                state.select_previous();
                self.selected_command_idx = 0;
                self.selected_log_idx = 0;
                self.selected_config_command_idx = 0;
            }
            PanelFocus::DetailPanel => {
                if self.active_tab == MainTab::Commands {
                    self.selected_command_idx = self.selected_command_idx.saturating_sub(1);
                } else if self.active_tab == MainTab::Logs {
                    self.selected_log_idx = self.selected_log_idx.saturating_sub(1);
                } else if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View {
                    self.selected_config_command_idx = self.selected_config_command_idx.saturating_sub(1);
                } else {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
            }
        }
    }

    fn handle_down(&mut self, state: &mut std::sync::MutexGuard<AppState>) {
        // Dependencies popup navigation
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditDependencies {
            if let Some(app) = state.selected_app() {
                if self.selected_dependency_idx < app.dependencies.len().saturating_sub(1) {
                    self.selected_dependency_idx += 1;
                }
            }
            return;
        }
        // Add dependency navigation
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::AddDependency {
            if let Ok(config) = crate::config::loader::load_config() {
                if self.selected_add_dep_project_idx < config.projects.len().saturating_sub(1) {
                    self.selected_add_dep_project_idx += 1;
                    self.selected_add_dep_app_idx = 0;
                }
            }
            return;
        }
        // Env files view navigation
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditEnvFiles {
            if let Some(app) = state.selected_app() {
                if let Some(env_files) = &app.env_files {
                    // Count total entries (not just stages)
                    let total_entries: usize = env_files.values().map(|contexts| contexts.len()).sum();
                    if self.selected_env_file_idx < total_entries.saturating_sub(1) {
                        self.selected_env_file_idx += 1;
                    }
                }
            }
            return;
        }
        
        match self.focus {
            PanelFocus::AppList => {
                state.select_next();
                self.selected_command_idx = 0;
                self.selected_log_idx = 0;
                self.selected_config_command_idx = 0;
            }
            PanelFocus::DetailPanel => {
                if self.active_tab == MainTab::Commands {
                    if let Some(app) = state.selected_app() {
                        let total_commands = Self::count_total_commands(app);
                        if self.selected_command_idx < total_commands.saturating_sub(1) {
                            self.selected_command_idx += 1;
                        }
                    }
                } else if self.active_tab == MainTab::Logs {
                    if let Some(app) = state.selected_app() {
                        if let Ok(log_files) = self.log_manager.list_logs_for_app(&app.name) {
                            if self.selected_log_idx < log_files.len().saturating_sub(1) {
                                self.selected_log_idx += 1;
                            }
                        }
                    }
                } else if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View {
                    if let Some(app) = state.selected_app() {
                        let total_commands = Self::count_total_commands(app);
                        if self.selected_config_command_idx < total_commands.saturating_sub(1) {
                            self.selected_config_command_idx += 1;
                        }
                    }
                } else {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
            }
        }
    }

    fn handle_enter(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        // Add dependency
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::AddDependency {
            if let Some(app) = state.selected_app() {
                let project = app.project.clone();
                let app_name = app.name.clone();
                // Lock will be released when function returns
                if let Err(e) = self.add_dependency(&project, &app_name) {
                    eprintln!("Error adding dependency: {}", e);
                }
            }
            return Ok(true);
        }
        
        if self.focus == PanelFocus::DetailPanel {
            match self.active_tab {
                MainTab::Commands => {
                    state.set_command_execution_requested(self.selected_command_idx);
                }
                MainTab::Logs => {
                    if let Some(app) = state.selected_app() {
                        if let Ok(log_files) = self.log_manager.list_logs_for_app(&app.name) {
                            if let Some(log_file) = log_files.get(self.selected_log_idx) {
                                state.current_view = crate::tui::state::ViewType::LogViewer {
                                    log_path: log_file.path.clone(),
                                };
                            }
                        }
                    }
                }
                MainTab::Config => {
                    if matches!(self.config_mode, ConfigMode::Add | ConfigMode::Edit) {
                        if let Err(e) = self.save_config_form() {
                            eprintln!("Error saving config: {}", e);
                        }
                        return Ok(true);
                    } else if self.config_mode == ConfigMode::AddCommand {
                        if let Some(app) = state.selected_app() {
                            let project = app.project.clone();
                            let app_name = app.name.clone();
                            if let Err(e) = self.save_new_command(&project, &app_name) {
                                eprintln!("Error saving command: {}", e);
                            }
                        }
                        return Ok(true);
                    } else if self.config_mode == ConfigMode::EditCommand {
                        if let Some(app) = state.selected_app() {
                            let project = app.project.clone();
                            let app_name = app.name.clone();
                            if let Err(e) = self.save_command_edit(&project, &app_name) {
                                eprintln!("Error saving command: {}", e);
                            }
                        }
                        return Ok(true);
                    } else if self.config_mode == ConfigMode::AddEnvFile || self.config_mode == ConfigMode::EditEnvFile {
                        if let Some(app) = state.selected_app() {
                            let app_name = app.name.clone();
                            if let Err(e) = self.save_env_file(&app_name) {
                                eprintln!("Error saving env file: {}", e);
                            }
                        }
                        return Ok(true);
                    }
                }
                MainTab::Status => {
                    if let Some(app) = state.selected_app() {
                        if !app.status.is_running() {
                            state.set_command_execution_requested(0);
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    // Config tab handlers
    fn handle_config_add(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        if self.config_mode == ConfigMode::View {
            if let Some(app) = state.selected_app() {
                self.config_mode = ConfigMode::AddCommand;
                self.config_form = ConfigForm {
                    project_name: app.project.clone(),
                    app_name: app.name.clone(),
                    edit_command_name: String::new(),
                    edit_command_value: String::new(),
                    edit_command_env: "local".to_string(),
                    ..Default::default()
                };
                self.config_focused_field = ConfigField::EditCommandEnv;
            }
        } else if self.config_mode == ConfigMode::EditDependencies {
            self.config_mode = ConfigMode::AddDependency;
            self.selected_add_dep_project_idx = 0;
            self.selected_add_dep_app_idx = 0;
        }
        Ok(true)
    }

    fn handle_config_edit(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        if self.config_mode == ConfigMode::View && self.focus == PanelFocus::DetailPanel {
            if let Some(app) = state.selected_app() {
                let project = app.project.clone();
                let app_name = app.name.clone();
                if let Err(e) = self.load_command_into_form(&project, &app_name) {
                    eprintln!("Error loading command: {}", e);
                }
            }
        }
        Ok(true)
    }

    fn handle_config_edit_app(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        if self.config_mode == ConfigMode::View {
            if let Some(app) = state.selected_app() {
                self.load_app_into_form(&app.project, &app.name, app);
                self.config_mode = ConfigMode::Edit;
                self.config_focused_field = ConfigField::ProjectName;
            }
        }
        Ok(true)
    }

    fn handle_config_edit_deps(&mut self) -> Result<bool> {
        if self.config_mode == ConfigMode::View {
            self.config_mode = ConfigMode::EditDependencies;
            self.selected_dependency_idx = 0;
        }
        Ok(true)
    }

    fn handle_config_edit_env_files(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        if self.config_mode == ConfigMode::View && state.selected_app().is_some() {
            self.config_mode = ConfigMode::EditEnvFiles;
            self.selected_env_file_idx = 0;
        }
        Ok(true)
    }

    fn handle_config_set_default(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        if self.config_mode == ConfigMode::View && self.focus == PanelFocus::DetailPanel {
            if let Some(app) = state.selected_app() {
                let project = app.project.clone();
                let app_name = app.name.clone();
                if let Err(e) = self.set_command_as_default(&project, &app_name) {
                    eprintln!("Error setting default command: {}", e);
                }
            }
        }
        Ok(true)
    }

    fn handle_config_delete(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        use super::DeleteType;
        
        if self.config_mode == ConfigMode::View {
            if let Some(app) = state.selected_app() {
                // Check if we're focused on the detail panel (commands) or app list
                if self.focus == PanelFocus::DetailPanel {
                    // Delete the selected command
                    if let Some((env, cmd_info)) = self.get_selected_config_command(app) {
                        self.delete_confirm_message = format!("Delete command '{}' ({})?", cmd_info.name, env.to_uppercase());
                        self.delete_confirm_type = DeleteType::Command;
                        self.config_mode = ConfigMode::ConfirmDelete;
                    }
                } else {
                    // Delete the app
                    self.delete_confirm_message = format!("Delete app '{}'?", app.name);
                    self.delete_confirm_type = DeleteType::App;
                    self.config_mode = ConfigMode::ConfirmDelete;
                }
            }
        } else if self.config_mode == ConfigMode::EditDependencies {
            if let Some(app) = state.selected_app() {
                if let Some(dep_name) = app.dependencies.get(self.selected_dependency_idx) {
                    self.delete_confirm_message = format!("Remove dependency '{}'?", dep_name);
                    self.delete_confirm_type = DeleteType::Dependency;
                    self.config_mode = ConfigMode::ConfirmDelete;
                }
            }
        }
        Ok(true)
    }

    fn handle_config_escape(&mut self) -> Result<bool> {
        match self.config_mode {
            ConfigMode::EditDependencies => {
                self.config_mode = ConfigMode::View;
                Ok(true)
            }
            ConfigMode::AddDependency => {
                self.config_mode = ConfigMode::EditDependencies;
                Ok(true)
            }
            ConfigMode::EditEnvFiles => {
                self.config_mode = ConfigMode::View;
                Ok(true)
            }
            ConfigMode::AddEnvFile | ConfigMode::EditEnvFile => {
                self.config_mode = ConfigMode::EditEnvFiles;
                Ok(true)
            }
            _ => Ok(false)
        }
    }

    fn handle_status_start_stop(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        if let Some(app) = state.selected_app() {
            if app.status.is_running() {
                state.set_stop_requested();
            } else {
                state.set_env_selection_requested();
            }
        }
        Ok(true)
    }

    fn handle_status_restart(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        if let Some(app) = state.selected_app() {
            if app.status.is_running() {
                state.set_restart_requested();
            }
        }
        Ok(true)
    }
}

impl MainView {
    /// Saves a new or edited environment file configuration
    fn save_env_file(&mut self, app_name: &str) -> Result<()> {
        use crate::commands::env::add_env_file;
        
        let stage = self.config_form.env_file_stage.trim();
        let context = self.config_form.env_file_context.trim();
        let file_path = self.config_form.env_file_path.trim();
        
        if stage.is_empty() || context.is_empty() || file_path.is_empty() {
            return Err(anyhow::anyhow!("All fields are required"));
        }
        
        add_env_file(app_name, stage, context, file_path)?;
        
        self.config_mode = ConfigMode::EditEnvFiles;
        Ok(())
    }
    
    /// Loads the selected env file entry into the form for editing
    fn handle_edit_env_file(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        if let Some(app) = state.selected_app() {
            if let Some(env_files) = &app.env_files {
                // Build flat list of entries
                let mut entries: Vec<(String, String, String)> = Vec::new();
                for (stage, contexts) in env_files {
                    let mut sorted_contexts: Vec<_> = contexts.iter().collect();
                    sorted_contexts.sort_by_key(|(context, _)| context.as_str());
                    
                    for (context, file_path) in sorted_contexts {
                        entries.push((stage.clone(), context.clone(), file_path.clone()));
                    }
                }
                entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
                
                if let Some((stage, context, file_path)) = entries.get(self.selected_env_file_idx) {
                    self.config_form.env_file_stage = stage.clone();
                    self.config_form.env_file_context = context.clone();
                    self.config_form.env_file_path = file_path.clone();
                    self.config_form.cursor_env_file_stage = stage.len();
                    self.config_form.cursor_env_file_context = context.len();
                    self.config_form.cursor_env_file_path = file_path.len();
                    self.config_focused_field = ConfigField::EnvFileStage;
                    self.config_mode = ConfigMode::EditEnvFile;
                }
            }
        }
        Ok(true)
    }
    
    /// Deletes the selected environment file entry
    fn handle_delete_env_file(&mut self, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        use super::DeleteType;
        
        if let Some(app) = state.selected_app() {
            if let Some(env_files) = &app.env_files {
                // Build flat list of entries
                let mut entries: Vec<(String, String, String)> = Vec::new();
                for (stage, contexts) in env_files {
                    let mut sorted_contexts: Vec<_> = contexts.iter().collect();
                    sorted_contexts.sort_by_key(|(context, _)| context.as_str());
                    
                    for (context, file_path) in sorted_contexts {
                        entries.push((stage.clone(), context.clone(), file_path.clone()));
                    }
                }
                entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
                
                if let Some((stage, context, file_path)) = entries.get(self.selected_env_file_idx) {
                    self.delete_confirm_message = format!(
                        "Delete env file?\n\nStage: {}\nContext: {}\nFile: {}",
                        stage, context, file_path
                    );
                    self.delete_confirm_type = DeleteType::EnvFile;
                    self.config_mode = ConfigMode::ConfirmDelete;
                }
            }
        }
        Ok(true)
    }
    
    /// Handles input when in delete confirmation mode
    fn handle_delete_confirmation(&mut self, key: KeyEvent, state: &mut std::sync::MutexGuard<AppState>) -> Result<bool> {
        use super::DeleteType;
        use crate::commands::env::remove_env_file;
        
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // User confirmed deletion - perform the delete action
                let previous_mode = match self.delete_confirm_type {
                    DeleteType::App => {
                        if let Some(app) = state.selected_app() {
                            let project = app.project.clone();
                            let app_name = app.name.clone();
                            if let Err(e) = self.delete_app(&project, &app_name) {
                                eprintln!("Error deleting app: {}", e);
                            } else {
                                // Reload state after deletion
                                if let Err(e) = self.reload_state_from_config(state) {
                                    eprintln!("Error reloading state: {}", e);
                                }
                            }
                        }
                        ConfigMode::View
                    }
                    DeleteType::Command => {
                        if let Some(app) = state.selected_app() {
                            let project = app.project.clone();
                            let app_name = app.name.clone();
                            if let Some((env, cmd_info)) = self.get_selected_config_command(app) {
                                let cmd_name = cmd_info.name.clone();
                                if let Err(e) = self.delete_command(&project, &app_name, env, &cmd_name) {
                                    eprintln!("Error deleting command: {}", e);
                                } else {
                                    // Adjust selection if needed
                                    let total_commands = Self::count_total_commands(app);
                                    if self.selected_config_command_idx >= total_commands.saturating_sub(1) {
                                        self.selected_config_command_idx = total_commands.saturating_sub(2).max(0);
                                    }
                                    // Reload state after deletion
                                    if let Err(e) = self.reload_state_from_config(state) {
                                        eprintln!("Error reloading state: {}", e);
                                    }
                                }
                            }
                        }
                        ConfigMode::View
                    }
                    DeleteType::Dependency => {
                        if let Some(app) = state.selected_app() {
                            let project = app.project.clone();
                            let app_name = app.name.clone();
                            if let Err(e) = self.remove_dependency(&project, &app_name, self.selected_dependency_idx) {
                                eprintln!("Error removing dependency: {}", e);
                            } else {
                                // Reload state after deletion
                                if let Err(e) = self.reload_state_from_config(state) {
                                    eprintln!("Error reloading state: {}", e);
                                }
                            }
                        }
                        ConfigMode::EditDependencies
                    }
                    DeleteType::EnvFile => {
                        if let Some(app) = state.selected_app() {
                            if let Some(env_files) = &app.env_files {
                                // Build flat list of entries
                                let mut entries: Vec<(String, String, String)> = Vec::new();
                                for (stage, contexts) in env_files {
                                    let mut sorted_contexts: Vec<_> = contexts.iter().collect();
                                    sorted_contexts.sort_by_key(|(context, _)| context.as_str());
                                    
                                    for (context, file_path) in sorted_contexts {
                                        entries.push((stage.clone(), context.clone(), file_path.clone()));
                                    }
                                }
                                entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
                                
                                if let Some((stage, context, _)) = entries.get(self.selected_env_file_idx) {
                                    if let Err(e) = remove_env_file(&app.name, stage, Some(context)) {
                                        eprintln!("Error removing env file: {}", e);
                                    } else {
                                        // Adjust selection if needed
                                        let total_entries: usize = env_files.values().map(|contexts| contexts.len()).sum();
                                        if self.selected_env_file_idx >= total_entries.saturating_sub(1) {
                                            self.selected_env_file_idx = total_entries.saturating_sub(2).max(0);
                                        }
                                        // Reload state after deletion
                                        if let Err(e) = self.reload_state_from_config(state) {
                                            eprintln!("Error reloading state: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        ConfigMode::EditEnvFiles
                    }
                };
                self.config_mode = previous_mode;
                Ok(true)
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                // User cancelled - return to previous mode
                let previous_mode = match self.delete_confirm_type {
                    DeleteType::App => ConfigMode::View,
                    DeleteType::Command => ConfigMode::View,
                    DeleteType::Dependency => ConfigMode::EditDependencies,
                    DeleteType::EnvFile => ConfigMode::EditEnvFiles,
                };
                self.config_mode = previous_mode;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
