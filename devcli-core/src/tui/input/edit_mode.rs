use crate::config::models::Environment;
use crate::tui::state::AppState;
use crate::tui::views::{ConfigField, ConfigMode, MainView};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::MutexGuard;

/// Trait for handling input in edit mode (config forms)
pub trait EditModeHandler {
    /// Handles input when in edit mode
    fn handle_edit_mode_input(
        &mut self,
        key: KeyEvent,
        state: &mut MutexGuard<AppState>,
    ) -> Result<bool>;

    /// Handles Enter key in edit mode
    fn handle_edit_mode_enter(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool>;

    /// Cycles through form fields with Tab key
    fn cycle_form_field(&mut self);

    /// Moves form field focus up
    fn move_form_field_up(&mut self);

    /// Moves form field focus down
    fn move_form_field_down(&mut self);

    /// Adds a character to the currently focused field
    fn add_char_to_field(&mut self, c: char);

    /// Removes a character from the currently focused field
    fn remove_char_from_field(&mut self);

    /// Moves cursor left in the current field
    fn move_cursor_left(&mut self);

    /// Moves cursor right in the current field
    fn move_cursor_right(&mut self);

    /// Cycles through available environments for AddCommand mode
    fn cycle_environment(&mut self);

    /// Handles input when in delete confirmation mode
    fn handle_delete_confirmation(
        &mut self,
        key: KeyEvent,
        state: &mut MutexGuard<AppState>,
    ) -> Result<bool>;
}

impl EditModeHandler for MainView {
    fn handle_edit_mode_input(
        &mut self,
        key: KeyEvent,
        state: &mut MutexGuard<AppState>,
    ) -> Result<bool> {
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
                if self.config_mode == ConfigMode::AddCommand
                    && self.config_focused_field == ConfigField::EditCommandEnv
                    && c == ' '
                {
                    self.cycle_environment();
                    return Ok(true);
                }
                self.add_char_to_field(c);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn handle_edit_mode_enter(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
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
            _ => Ok(false),
        }
    }

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
        } else if self.config_mode == ConfigMode::AddEnvFile
            || self.config_mode == ConfigMode::EditEnvFile
        {
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

    fn add_char_to_field(&mut self, c: char) {
        use crate::tui::views::main_view::ConfigField;

        match self.config_focused_field {
            ConfigField::ProjectName => self.config_form.project_name.insert_char(c),
            ConfigField::AppName => self.config_form.app_name.insert_char(c),
            ConfigField::AppType => self.config_form.app_type.insert_char(c),
            ConfigField::Path => self.config_form.path.insert_char(c),
            ConfigField::LocalStartCmd => self.config_form.local_start_cmd.insert_char(c),
            ConfigField::DockerStartCmd => self.config_form.docker_start_cmd.insert_char(c),
            ConfigField::EditCommandName => self.config_form.edit_command_name.insert_char(c),
            ConfigField::EditCommandValue => self.config_form.edit_command_value.insert_char(c),
            ConfigField::EnvFileStage => self.config_form.env_file_stage.insert_char(c),
            ConfigField::EnvFileContext => self.config_form.env_file_context.insert_char(c),
            ConfigField::EnvFilePath => self.config_form.env_file_path.insert_char(c),
            _ => {}
        }
    }

    fn remove_char_from_field(&mut self) {
        use crate::tui::views::main_view::ConfigField;

        match self.config_focused_field {
            ConfigField::ProjectName => self.config_form.project_name.delete_char(),
            ConfigField::AppName => self.config_form.app_name.delete_char(),
            ConfigField::AppType => self.config_form.app_type.delete_char(),
            ConfigField::Path => self.config_form.path.delete_char(),
            ConfigField::LocalStartCmd => self.config_form.local_start_cmd.delete_char(),
            ConfigField::DockerStartCmd => self.config_form.docker_start_cmd.delete_char(),
            ConfigField::EditCommandName => self.config_form.edit_command_name.delete_char(),
            ConfigField::EditCommandValue => self.config_form.edit_command_value.delete_char(),
            ConfigField::EnvFileStage => self.config_form.env_file_stage.delete_char(),
            ConfigField::EnvFileContext => self.config_form.env_file_context.delete_char(),
            ConfigField::EnvFilePath => self.config_form.env_file_path.delete_char(),
            _ => {}
        }
    }

    fn move_cursor_left(&mut self) {
        use crate::tui::views::main_view::ConfigField;

        match self.config_focused_field {
            ConfigField::ProjectName => self.config_form.project_name.move_cursor_left(),
            ConfigField::AppName => self.config_form.app_name.move_cursor_left(),
            ConfigField::AppType => self.config_form.app_type.move_cursor_left(),
            ConfigField::Path => self.config_form.path.move_cursor_left(),
            ConfigField::LocalStartCmd => self.config_form.local_start_cmd.move_cursor_left(),
            ConfigField::DockerStartCmd => self.config_form.docker_start_cmd.move_cursor_left(),
            ConfigField::EditCommandName => self.config_form.edit_command_name.move_cursor_left(),
            ConfigField::EditCommandValue => self.config_form.edit_command_value.move_cursor_left(),
            ConfigField::EnvFileStage => self.config_form.env_file_stage.move_cursor_left(),
            ConfigField::EnvFileContext => self.config_form.env_file_context.move_cursor_left(),
            ConfigField::EnvFilePath => self.config_form.env_file_path.move_cursor_left(),
            _ => {}
        }
    }

    fn move_cursor_right(&mut self) {
        use crate::tui::views::main_view::ConfigField;

        match self.config_focused_field {
            ConfigField::ProjectName => self.config_form.project_name.move_cursor_right(),
            ConfigField::AppName => self.config_form.app_name.move_cursor_right(),
            ConfigField::AppType => self.config_form.app_type.move_cursor_right(),
            ConfigField::Path => self.config_form.path.move_cursor_right(),
            ConfigField::LocalStartCmd => self.config_form.local_start_cmd.move_cursor_right(),
            ConfigField::DockerStartCmd => self.config_form.docker_start_cmd.move_cursor_right(),
            ConfigField::EditCommandName => self.config_form.edit_command_name.move_cursor_right(),
            ConfigField::EditCommandValue => {
                self.config_form.edit_command_value.move_cursor_right()
            }
            ConfigField::EnvFileStage => self.config_form.env_file_stage.move_cursor_right(),
            ConfigField::EnvFileContext => self.config_form.env_file_context.move_cursor_right(),
            ConfigField::EnvFilePath => self.config_form.env_file_path.move_cursor_right(),
            _ => {}
        }
    }

    fn cycle_environment(&mut self) {
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

    fn handle_delete_confirmation(
        &mut self,
        key: KeyEvent,
        state: &mut MutexGuard<AppState>,
    ) -> Result<bool> {
        use crate::commands::env::remove_env_file;
        use crate::tui::views::main_view::DeleteType;

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
                                if let Err(e) =
                                    self.delete_command(&project, &app_name, env, &cmd_name)
                                {
                                    eprintln!("Error deleting command: {}", e);
                                } else {
                                    // Adjust selection if needed
                                    let total_commands =
                                        crate::tui::views::MainView::count_total_commands(app);
                                    if self.selected_config_command_idx
                                        >= total_commands.saturating_sub(1)
                                    {
                                        self.selected_config_command_idx =
                                            total_commands.saturating_sub(2).max(0);
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
                            if let Err(e) = self.remove_dependency(
                                &project,
                                &app_name,
                                self.popup_scroll_manager.selected_dependency_idx,
                            ) {
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
                                        entries.push((
                                            stage.clone(),
                                            context.clone(),
                                            file_path.clone(),
                                        ));
                                    }
                                }
                                entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

                                if let Some((stage, context, _)) =
                                    entries.get(self.popup_scroll_manager.selected_env_file_idx)
                                {
                                    if let Err(e) = remove_env_file(&app.name, stage, Some(context))
                                    {
                                        eprintln!("Error removing env file: {}", e);
                                    } else {
                                        // Adjust selection if needed
                                        let total_entries: usize =
                                            env_files.values().map(|contexts| contexts.len()).sum();
                                        if self.popup_scroll_manager.selected_env_file_idx
                                            >= total_entries.saturating_sub(1)
                                        {
                                            self.popup_scroll_manager.selected_env_file_idx =
                                                total_entries.saturating_sub(2).max(0);
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
