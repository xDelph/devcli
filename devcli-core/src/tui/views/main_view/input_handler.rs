// Input handling logic for MainView
// Handles all keyboard input and user interactions

use super::{ConfigField, ConfigForm, ConfigMode, MainTab, MainView, PanelFocus};
use crate::tui::input::keybindings::KeyBindings;
use crate::tui::input::{EditModeHandler, NavigationHandler};
use crate::tui::state::AppState;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use std::sync::{Arc, Mutex, MutexGuard};

impl MainView {
    /// Handles keyboard input for the main view
    /// Returns true if the event was handled, false otherwise
    pub fn handle_input(&mut self, key: KeyEvent, state: &Arc<Mutex<AppState>>) -> Result<bool> {
        // Lock the state for modification
        let mut state = state
            .lock()
            .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;

        // Handle delete confirmation mode
        if self.config_mode == ConfigMode::ConfirmDelete {
            return self.handle_delete_confirmation(key, &mut state);
        }

        // In edit mode, only handle specific keys - everything else is for typing
        let in_edit_mode = self.active_tab == MainTab::Config
            && matches!(
                self.config_mode,
                ConfigMode::Add
                    | ConfigMode::Edit
                    | ConfigMode::AddCommand
                    | ConfigMode::EditCommand
                    | ConfigMode::AddEnvFile
                    | ConfigMode::EditEnvFile
            );

        if in_edit_mode {
            return self.handle_edit_mode_input(key, &mut state);
        }

        // Normal mode key handling
        self.handle_normal_mode_input(key, &mut state)
    }
}

impl NavigationHandler for MainView {
    fn handle_normal_mode_input(
        &mut self,
        key: KeyEvent,
        state: &mut MutexGuard<AppState>,
    ) -> Result<bool> {
        // POPUP INPUT HANDLING - Must be FIRST to prevent background interaction
        // All popup modes should trap input here before any other processing

        // EditDependencies popup
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditDependencies {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.popup_scroll_manager.scroll_dependencies_up();
                    return Ok(true);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(app) = state.selected_app() {
                        self.popup_scroll_manager
                            .scroll_dependencies_down(app.dependencies.len());
                    }
                    return Ok(true);
                }
                KeyCode::Char('a') => {
                    self.config_mode = ConfigMode::AddDependency;
                    self.popup_scroll_manager.selected_add_dep_idx = 0;
                    return Ok(true);
                }
                KeyCode::Char('d') => {
                    return self.handle_config_delete(state);
                }
                KeyCode::Esc => {
                    return self.handle_config_escape();
                }
                _ => return Ok(true), // Consume all other keys
            }
        }

        // AddDependency popup
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::AddDependency {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.popup_scroll_manager.scroll_add_dependency_up();
                    return Ok(true);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Ok(config) = crate::config::loader::load_config() {
                        let (proj, app) = if let Some(app) = state.selected_app() {
                            (Some(app.project.as_str()), Some(app.name.as_str()))
                        } else {
                            (None, None)
                        };

                        let candidates = self.get_sorted_dependency_candidates(&config, proj, app);
                        // Count total apps across all projects
                        let total_apps: usize = candidates.iter().map(|(_, apps)| apps.len()).sum();

                        self.popup_scroll_manager
                            .scroll_add_dependency_down(total_apps);
                    }
                    return Ok(true);
                }
                KeyCode::Esc => {
                    return self.handle_config_escape();
                }
                KeyCode::Right | KeyCode::Enter => {
                    return self.handle_enter(state);
                }
                _ => return Ok(true), // Consume all other keys
            }
        }

        // EditEnvFiles popup
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::EditEnvFiles {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.popup_scroll_manager.scroll_env_files_up();
                    return Ok(true);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(app) = state.selected_app() {
                        if let Some(env_files) = &app.env_files {
                            let total_entries: usize =
                                env_files.values().map(|contexts| contexts.len()).sum();
                            self.popup_scroll_manager
                                .scroll_env_files_down(total_entries);
                        }
                    }
                    return Ok(true);
                }
                KeyCode::Char('a') => {
                    self.config_mode = ConfigMode::AddEnvFile;
                    self.config_form.env_file_stage.clear();
                    self.config_form.env_file_context.clear();
                    self.config_form.env_file_path.clear();
                    self.config_focused_field = ConfigField::EnvFileStage;
                    return Ok(true);
                }
                KeyCode::Char('e') => {
                    self.load_selected_env_file(state)?;
                    return Ok(true);
                }
                KeyCode::Char('d') => {
                    self.confirm_delete_env_file(state)?;
                    return Ok(true);
                }
                KeyCode::Esc => {
                    return self.handle_config_escape();
                }
                _ => return Ok(true), // Consume all other keys
            }
        }

        // Normal mode key handling - only reached if no popup is active
        if KeyBindings::is_tab_status(key) {
            self.switch_to_tab(MainTab::Status);
            self.selected_log_idx = 0;
            return Ok(true);
        }

        if KeyBindings::is_tab_config(key) {
            self.switch_to_tab(MainTab::Config);
            return Ok(true);
        }
        if KeyBindings::is_tab_cycle(key) {
            self.cycle_tab();
            if self.active_tab == MainTab::Status {
                self.selected_log_idx = 0;
            }
            return Ok(true);
        }
        if KeyBindings::is_panel_toggle(key) {
            self.toggle_panel_focus();
            return Ok(true);
        }
        if KeyBindings::is_fast_up(key) {
            self.handle_shift_up(state);
            return Ok(true);
        }
        if KeyBindings::is_fast_down(key) {
            self.handle_shift_down(state);
            return Ok(true);
        }
        if KeyBindings::is_up(key) {
            self.handle_up(state);
            return Ok(true);
        }
        if KeyBindings::is_down(key) {
            self.handle_down(state);
            return Ok(true);
        }
        if KeyBindings::is_expand(key) && self.focus == PanelFocus::AppList {
            state.toggle_project_expansion();
            return Ok(true);
        }
        if KeyBindings::is_enter(key) {
            return self.handle_enter(state);
        }

        // Specific handlers based on tab and key
        match key.code {
            // Env files view actions
            KeyCode::Char('a')
                if self.active_tab == MainTab::Config
                    && self.config_mode == ConfigMode::EditEnvFiles =>
            {
                self.config_mode = ConfigMode::AddEnvFile;
                self.config_form.env_file_stage.clear();
                self.config_form.env_file_context.clear();
                self.config_form.env_file_path.clear();
                self.config_focused_field = ConfigField::EnvFileStage;
                Ok(true)
            }
            KeyCode::Char('e')
                if self.active_tab == MainTab::Config
                    && self.config_mode == ConfigMode::EditEnvFiles =>
            {
                self.load_selected_env_file(state)?;
                Ok(true)
            }
            KeyCode::Char('d')
                if self.active_tab == MainTab::Config
                    && self.config_mode == ConfigMode::EditEnvFiles =>
            {
                self.confirm_delete_env_file(state)?;
                Ok(true)
            }
            // Config tab specific keys
            KeyCode::Char('a') if self.active_tab == MainTab::Config => {
                self.handle_config_add(state)
            }
            KeyCode::Char('e') if self.active_tab == MainTab::Config => {
                self.handle_config_edit(state)
            }
            KeyCode::Char('f') if self.active_tab == MainTab::Config => {
                self.handle_config_edit_env_files(state)
            }
            KeyCode::Char('E') if self.active_tab == MainTab::Config => {
                self.handle_config_edit_app(state)
            }
            KeyCode::Char('D') if self.active_tab == MainTab::Config => {
                self.handle_config_edit_deps()
            }
            KeyCode::Char('s') if self.active_tab == MainTab::Config => {
                self.handle_config_set_default(state)
            }
            KeyCode::Char('d') if self.active_tab == MainTab::Config => {
                self.handle_config_delete(state)
            }
            // Dependencies popup actions
            KeyCode::Esc if self.active_tab == MainTab::Config => self.handle_config_escape(),
            // Status tab specific keys or global AppList shortcuts
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if self.active_tab == MainTab::Status || self.focus == PanelFocus::AppList {
                    self.handle_status_start_stop(state)
                } else if self.active_tab == MainTab::Config {
                    self.handle_config_set_default(state)
                } else {
                    Ok(false)
                }
            }
            KeyCode::Char('r') => {
                if self.active_tab == MainTab::Status || self.focus == PanelFocus::AppList {
                    self.handle_status_restart(state)
                } else {
                    Ok(false)
                }
            }
            // Quick log view shortcut
            KeyCode::Char('l') => {
                if let Some(app) = state.selected_app() {
                    // Only open logs if app is running
                    if app.status.is_running() {
                        if let Ok(log_files) =
                            self.log_manager.list_logs_for_app(&app.project, &app.name)
                        {
                            if let Some(log_file) = log_files.first() {
                                let prev = state.current_view.clone();
                                state.view_history.push(prev);
                                state.current_view = crate::tui::state::ViewType::LogViewer {
                                    log_paths: vec![log_file.path.clone()],
                                    active_index: 0,
                                };
                                return Ok(true);
                            }
                        }
                    }
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn switch_to_tab(&mut self, tab: MainTab) {
        self.active_tab = tab;
        self.detail_scroll = 0;
        // self.selected_command_idx = 0; // Removing this line
        if self.active_tab == MainTab::Config {
            self.config_mode = ConfigMode::View;
            self.selected_config_command_idx = 0;
        }
    }

    fn cycle_tab(&mut self) {
        self.active_tab = match self.active_tab {
            MainTab::Status => MainTab::Config,
            MainTab::Config => MainTab::Status,
        };
        self.detail_scroll = 0;

        if self.active_tab == MainTab::Config {
            self.config_mode = ConfigMode::View;
        }
    }

    /// Handles scrolling for the detail panel
    fn toggle_panel_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::AppList => PanelFocus::DetailPanel,
            PanelFocus::DetailPanel => PanelFocus::AppList,
        };
    }

    fn handle_shift_up(&mut self, state: &mut MutexGuard<AppState>) {
        match self.focus {
            PanelFocus::AppList => {
                for _ in 0..10 {
                    state.select_previous();
                }

                self.selected_config_command_idx = 0;

                if self.active_tab == MainTab::Status {
                    self.selected_log_idx = 0;
                }
            }
            PanelFocus::DetailPanel => {
                if (self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View)
                    || self.active_tab == MainTab::Status
                {
                    self.handle_detail_scroll(-10, state);
                } else {
                    self.detail_scroll = self.detail_scroll.saturating_sub(10);
                }
            }
        }
    }

    fn handle_shift_down(&mut self, state: &mut MutexGuard<AppState>) {
        match self.focus {
            PanelFocus::AppList => {
                for _ in 0..10 {
                    state.select_next();
                }

                self.selected_config_command_idx = 0;

                if self.active_tab == MainTab::Status {
                    self.selected_log_idx = 0;
                }
            }
            PanelFocus::DetailPanel => {
                if (self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View)
                    || self.active_tab == MainTab::Status
                {
                    self.handle_detail_scroll(10, state);
                } else if let Some(app) = state.selected_app() {
                    let total_commands = Self::count_total_commands(app);
                    self.selected_config_command_idx = (self.selected_config_command_idx + 10)
                        .min(total_commands.saturating_sub(1));
                } else {
                    self.detail_scroll = self.detail_scroll.saturating_add(10);
                }
            }
        }
    }

    fn handle_up(&mut self, state: &mut MutexGuard<AppState>) {
        // Popup navigation is now handled in handle_normal_mode_input
        // This function should never be reached when a popup is active
        match self.focus {
            PanelFocus::AppList => {
                state.select_previous();

                self.selected_config_command_idx = 0;

                if self.active_tab == MainTab::Status {
                    self.selected_log_idx = 0;
                }
            }
            PanelFocus::DetailPanel => {
                if (self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View)
                    || self.active_tab == MainTab::Status
                {
                    self.handle_detail_scroll(-1, state);
                } else {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                }
            }
        }
    }

    fn handle_down(&mut self, state: &mut MutexGuard<AppState>) {
        // Popup navigation is now handled in handle_normal_mode_input
        // This function should never be reached when a popup is active
        match self.focus {
            PanelFocus::AppList => {
                state.select_next();

                self.selected_config_command_idx = 0;
            }
            PanelFocus::DetailPanel => {
                if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::View {
                    if let Some(app) = state.selected_app() {
                        let total_commands = Self::count_total_commands(app);
                        if self.selected_config_command_idx < total_commands.saturating_sub(1) {
                            self.selected_config_command_idx += 1;
                        }
                    }
                } else if self.active_tab == MainTab::Status {
                    self.handle_detail_scroll(1, state);
                } else {
                    self.detail_scroll = self.detail_scroll.saturating_add(1);
                }
            }
        }
    }

    fn handle_enter(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
        if self.active_tab == MainTab::Config && self.config_mode == ConfigMode::AddDependency {
            if let Ok(config) = crate::config::loader::load_config() {
                let (curr_proj, curr_app) = if let Some(app) = state.selected_app() {
                    (Some(app.project.as_str()), Some(app.name.as_str()))
                } else {
                    (None, None)
                };

                let candidates =
                    self.get_sorted_dependency_candidates(&config, curr_proj, curr_app);

                if let Some((proj_name, app_name)) = self.get_app_at_flat_index(
                    &candidates,
                    self.popup_scroll_manager.selected_add_dep_idx,
                ) {
                    // Ensure we have a current project and app selected
                    if let (Some(curr_proj), Some(curr_app)) = (curr_proj, curr_app) {
                        // Lock will be released when function returns
                        if let Err(e) =
                            self.add_dependency(curr_proj, curr_app, &proj_name, &app_name)
                        {
                            eprintln!("Error adding dependency: {}", e);
                        } else {
                            // Reset state and return to dependencies list
                            self.popup_scroll_manager.reset();
                            self.config_mode = ConfigMode::EditDependencies;

                            // Reload state to reflect changes in UI
                            if let Err(e) = self.reload_state_from_config(state) {
                                eprintln!("Error reloading state: {}", e);
                            }
                        }
                    }
                }
            }
            return Ok(true);
        }

        if self.focus == PanelFocus::DetailPanel {
            match self.active_tab {
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
                    } else if self.config_mode == ConfigMode::AddEnvFile
                        || self.config_mode == ConfigMode::EditEnvFile
                    {
                        if let Some(app) = state.selected_app() {
                            let app_name = app.name.clone();
                            if let Err(e) = self.save_env_file(&app_name) {
                                eprintln!("Error saving env file: {}", e);
                            }
                        }
                        return Ok(true);
                    } else if self.config_mode == ConfigMode::View
                        && self.focus == PanelFocus::DetailPanel
                    {
                        state.set_command_execution_requested(self.selected_config_command_idx);
                        return Ok(true);
                    }
                }
                MainTab::Status => {
                    // Open log viewer for the selected log file
                    if let Some(app) = state.selected_app() {
                        if let Ok(mut logs) =
                            self.log_manager.list_logs_for_app(&app.project, &app.name)
                        {
                            logs.sort_by_key(|a| std::cmp::Reverse(a.modified));
                            if let Some(log) = logs.get(self.selected_log_idx) {
                                let prev = state.current_view.clone();
                                state.view_history.push(prev);
                                state.current_view = crate::tui::state::ViewType::LogViewer {
                                    log_paths: vec![log.path.clone()],
                                    active_index: 0,
                                };
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    fn handle_config_add(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
        use crate::tui::widgets::TextEditor;
        if self.config_mode == ConfigMode::View {
            if let Some(app) = state.selected_app() {
                self.config_mode = ConfigMode::AddCommand;
                self.config_form = ConfigForm {
                    project_name: TextEditor::with_content(app.project.clone()),
                    app_name: TextEditor::with_content(app.name.clone()),
                    edit_command_name: TextEditor::new(),
                    edit_command_value: TextEditor::new(),
                    edit_command_env: "local".to_string(),
                    ..Default::default()
                };
                self.config_focused_field = ConfigField::EditCommandEnv;
            }
        } else if self.config_mode == ConfigMode::EditDependencies {
            self.config_mode = ConfigMode::AddDependency;
            self.popup_scroll_manager.selected_add_dep_idx = 0;
        }
        Ok(true)
    }

    fn handle_config_edit(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
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

    fn handle_config_edit_app(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
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
            self.popup_scroll_manager.selected_dependency_idx = 0;
        }
        Ok(true)
    }

    fn handle_config_edit_env_files(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
        if self.config_mode == ConfigMode::View && state.selected_app().is_some() {
            self.config_mode = ConfigMode::EditEnvFiles;
            self.popup_scroll_manager.selected_env_file_idx = 0;
        }
        Ok(true)
    }

    fn handle_config_set_default(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
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

    fn handle_config_delete(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
        use crate::tui::views::main_view::DeleteType;

        if self.config_mode == ConfigMode::View {
            if let Some(app) = state.selected_app() {
                // Check if we're focused on the detail panel (commands) or app list
                if self.focus == PanelFocus::DetailPanel {
                    // Delete the selected command
                    if let Some((env, cmd_info)) = self.get_selected_config_command(app) {
                        self.delete_confirm_message = format!(
                            "Delete command '{}' ({})?",
                            cmd_info.name,
                            env.to_uppercase()
                        );
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
                if let Some(dep_name) = app
                    .dependencies
                    .get(self.popup_scroll_manager.selected_dependency_idx)
                {
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
                // Always exit back to dependencies list
                self.config_mode = ConfigMode::EditDependencies;
                self.popup_scroll_manager.reset();
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
            _ => Ok(false),
        }
    }

    fn handle_status_start_stop(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
        if let Some(app) = state.selected_app() {
            crate::debug!(
                "[InputHandler] Selected app: {}, Status: {:?}",
                app.name,
                app.status
            );
            if app.status.is_running() {
                crate::debug!("[InputHandler] Requesting stop");
                state.set_stop_requested();
            } else {
                crate::debug!("[InputHandler] Requesting env selection");
                state.set_env_selection_requested();
            }
        } else {
            crate::debug!("[InputHandler] No app selected");
        }
        Ok(true)
    }

    fn handle_status_restart(&mut self, state: &mut MutexGuard<AppState>) -> Result<bool> {
        if let Some(app) = state.selected_app() {
            if app.status.is_running() {
                state.set_restart_requested();
            }
        }
        Ok(true)
    }
}

impl MainView {
    /// Handles scrolling for the detail panel
    fn handle_detail_scroll(&mut self, direction: i16, state: &AppState) {
        match self.active_tab {
            MainTab::Status => {
                // Navigate log file list
                if let Some(app) = state.selected_app() {
                    if let Ok(logs) = self.log_manager.list_logs_for_app(&app.project, &app.name) {
                        let total = logs.len();
                        if total > 0 {
                            let new_idx = (self.selected_log_idx as isize + direction as isize)
                                .max(0)
                                .min((total - 1) as isize);
                            self.selected_log_idx = new_idx as usize;
                        }
                    }
                }
            }
            MainTab::Config => {
                if self.config_mode == ConfigMode::View {
                    let total_commands = if let Some(app) = state.selected_app() {
                        Self::count_total_commands(app)
                    } else {
                        0
                    };
                    let max_idx = total_commands.saturating_sub(1);
                    let new_idx = (self.selected_config_command_idx as isize + direction as isize)
                        .max(0)
                        .min(max_idx as isize);
                    self.selected_config_command_idx = new_idx as usize;
                }
            }
        }
    }
}
