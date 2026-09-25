use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::process_manager_support::{find_process, state_store};
use crate::tui::app::{CommandRequest, CommandResult, CommandType};
use crate::tui::state::AppState;
use crate::tui::theme::Theme;
use crate::tui::views::MainView;
use crate::tui::widgets::command_popup::PopupState;
use crate::tui::widgets::CommandPopup;

pub struct PopupManager {
    pub active_popup: Option<CommandPopup>,
    command_tx: UnboundedSender<CommandRequest>,
}

impl PopupManager {
    pub(crate) fn new(command_tx: UnboundedSender<CommandRequest>) -> Self {
        Self {
            active_popup: None,
            command_tx,
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active_popup.is_some()
    }

    pub(crate) fn close_popup(&mut self) {
        self.active_popup = None;
    }

    pub(crate) fn update_status(&mut self, status: String) {
        if let Some(popup) = &mut self.active_popup {
            popup.update_status(status);
        }
    }

    /// Check all popup-related updates
    /// Returns true if redraw is needed
    pub(crate) fn tick(
        &mut self,
        state: &Arc<Mutex<AppState>>,
        command_rx: &mut UnboundedReceiver<CommandResult>,
        main_view: &MainView,
    ) -> Result<bool> {
        let mut redraw = false;

        redraw |= self.check_command_execution_request(state, main_view)?;
        redraw |= self.check_stop_request(state)?;
        redraw |= self.check_restart_request(state)?;
        redraw |= self.check_env_selection_request(state)?;
        redraw |= self.check_command_results(command_rx)?;

        // Check for status updates in active popup
        // This corresponds to app.rs check_status_update part that updates popup
        {
            let (status_updated, app_status) = {
                let state = state
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
                let updated = state.status_updated;

                let app_status = if self.active_popup.is_some() {
                    state.selected_app().map(|app| app.status.display_label())
                } else {
                    None
                };
                (updated, app_status)
            };

            if status_updated {
                if let (Some(popup), Some(status)) = (&mut self.active_popup, app_status) {
                    popup.update_status(status);
                    redraw = true;
                }
            }
        }

        Ok(redraw)
    }

    /// Render the active popup
    pub(crate) fn render(&mut self, frame: &mut Frame, theme: &Theme) {
        if let Some(popup) = &mut self.active_popup {
            popup.render(frame, theme);
        }
    }

    /// Handle keyboard input when popup is active
    pub(crate) fn handle_input(&mut self, key: KeyEvent) -> Result<()> {
        if let Some(popup) = &mut self.active_popup {
            match key.code {
                KeyCode::Esc => {
                    self.active_popup = None;
                }
                KeyCode::Enter => {
                    // If confirmed, execute
                    if matches!(popup.state(), PopupState::Confirm) {
                        self.execute_command_from_popup()?;
                    } else if matches!(popup.state(), PopupState::Success(_) | PopupState::Error(_))
                    {
                        self.active_popup = None;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => popup.scroll_up(),
                KeyCode::Down | KeyCode::Char('j') => popup.scroll_down(),
                KeyCode::Home => popup.scroll_to_top(),
                KeyCode::End => popup.enable_auto_scroll(),
                KeyCode::Left => popup.prev_environment(),
                KeyCode::Right => popup.next_environment(),
                _ => {}
            }
        }
        Ok(())
    }

    fn check_command_execution_request(
        &mut self,
        state: &Arc<Mutex<AppState>>,
        main_view: &MainView,
    ) -> Result<bool> {
        let request = {
            let state = state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            state.command_execution_requested
        };

        if let Some(command_idx) = request {
            let (app_name, project_name, command_name, environment) = {
                let mut state = state
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;

                if let Some(app) = state.selected_app() {
                    if let Some((env, cmd_info)) = main_view.get_command_by_index(app, command_idx)
                    {
                        (
                            app.name.clone(),
                            app.project.clone(),
                            cmd_info.name.clone(),
                            env.to_string(),
                        )
                    } else {
                        // Invalid command index
                        state.clear_command_execution_request();
                        return Ok(false);
                    }
                } else {
                    // No app selected
                    state.clear_command_execution_request();
                    return Ok(false);
                }
            };

            let mut popup = CommandPopup::new(
                "run".to_string(),
                format!("Running: {} ({})", command_name, environment),
                app_name.clone(),
                project_name,
                format!("{}:{}", environment, command_name),
            );

            // Set initial status
            {
                let state = state
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
                if let Some(app) = state.selected_app() {
                    if app.name == app_name {
                        popup.update_status(app.status.display_label());
                    }
                }
            }

            self.active_popup = Some(popup);

            let mut state = state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            state.clear_command_execution_request();
            return Ok(true);
        }
        Ok(false)
    }

    fn check_stop_request(&mut self, state: &Arc<Mutex<AppState>>) -> Result<bool> {
        let stop_requested = {
            let state = state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            state.stop_requested
        };

        if stop_requested {
            let (app_name, project_name) = {
                let mut state = state
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
                if let Some(app) = state.selected_app() {
                    (app.name.clone(), app.project.clone())
                } else {
                    state.clear_stop_requested();
                    return Ok(false);
                }
            };

            let mut popup = CommandPopup::new(
                "stop".to_string(),
                "Stopping process...".to_string(),
                app_name.clone(),
                project_name.clone(),
                "".to_string(),
            );

            {
                let state = state
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
                if let Some(app) = state.selected_app() {
                    if app.name == app_name {
                        popup.update_status(app.status.display_label());
                    }
                }
            }

            self.active_popup = Some(popup);

            // Immediately execute stop
            let request = CommandRequest {
                app_name,
                project: project_name,
                environment: String::new(),
                command_type: CommandType::Stop,
            };

            if let Some(popup) = &mut self.active_popup {
                popup.set_executing();
            }

            self.command_tx.send(request)?;

            let mut state = state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            state.clear_stop_requested();
            return Ok(true);
        }
        Ok(false)
    }

    fn check_restart_request(&mut self, state: &Arc<Mutex<AppState>>) -> Result<bool> {
        let restart_requested = {
            let state = state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            state.restart_requested
        };

        if restart_requested {
            let (app_name, project_name, environment) = {
                let mut state = state
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
                if let Some(app) = state.selected_app() {
                    let env = if let Ok(store) = state_store() {
                        if let Ok(Some(process)) =
                            find_process(&store, &app.project, &app.name, None)
                        {
                            process
                                .metadata
                                .get("environment")
                                .cloned()
                                .unwrap_or_else(|| "local".to_string())
                        } else {
                            "local".to_string()
                        }
                    } else {
                        "local".to_string()
                    };
                    (app.name.clone(), app.project.clone(), env)
                } else {
                    state.clear_restart_requested();
                    return Ok(false);
                }
            };

            let mut popup = CommandPopup::new(
                "restart".to_string(),
                "Restarting process...".to_string(),
                app_name.clone(),
                project_name.clone(),
                environment.clone(),
            );

            {
                let state = state
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
                if let Some(app) = state.selected_app() {
                    if app.name == app_name {
                        popup.update_status(app.status.display_label());
                    }
                }
            }

            self.active_popup = Some(popup);

            // Immediately execute restart
            let request = CommandRequest {
                app_name,
                project: project_name,
                environment,
                command_type: CommandType::Restart,
            };

            if let Some(popup) = &mut self.active_popup {
                popup.set_executing();
            }

            self.command_tx.send(request)?;

            let mut state = state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            state.clear_restart_requested();
            return Ok(true);
        }
        Ok(false)
    }

    fn check_env_selection_request(&mut self, state: &Arc<Mutex<AppState>>) -> Result<bool> {
        let env_selection_requested = {
            let state = state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            state.env_selection_requested
        };

        if env_selection_requested {
            let (app_name, project_name, default_env) = {
                let mut state = state
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
                if let Some(app) = state.selected_app() {
                    let preferences = crate::config::load_preferences().unwrap_or_default();
                    (
                        app.name.clone(),
                        app.project.clone(),
                        preferences.default_env,
                    )
                } else {
                    state.clear_env_selection_requested();
                    return Ok(false);
                }
            };

            let mut popup = CommandPopup::new_with_env_selection(
                "start".to_string(),
                "Start application".to_string(),
                app_name.clone(),
                project_name,
                default_env,
            );

            {
                let state = state
                    .lock()
                    .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
                if let Some(app) = state.selected_app() {
                    if app.name == app_name {
                        popup.update_status(app.status.display_label());
                    }
                }
            }

            self.active_popup = Some(popup);

            let mut state = state
                .lock()
                .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
            state.clear_env_selection_requested();
            return Ok(true);
        }
        Ok(false)
    }

    fn check_command_results(
        &mut self,
        command_rx: &mut UnboundedReceiver<CommandResult>,
    ) -> Result<bool> {
        let mut updated = false;
        while let Ok(result) = command_rx.try_recv() {
            updated = true;
            match result {
                CommandResult::Success(msg) => {
                    if let Some(popup) = &mut self.active_popup {
                        popup.set_success(msg);
                    }
                    crate::debug!("Command execution successful");
                }
                CommandResult::Error(msg) => {
                    if let Some(popup) = &mut self.active_popup {
                        popup.set_error(msg);
                    }
                    crate::debug!("Command execution failed");
                }
                CommandResult::LogLine(line) => {
                    if let Some(popup) = &mut self.active_popup {
                        popup.add_output_line(line);
                    }
                }
            }
        }
        Ok(updated)
    }

    fn execute_command_from_popup(&mut self) -> Result<()> {
        let request = if let Some(popup) = &self.active_popup {
            let command_type = match popup.command_name.as_str() {
                "start" => CommandType::Start,
                "run" => CommandType::Run,
                "stop" => CommandType::Stop,
                "restart" => CommandType::Restart,
                _ => CommandType::Start,
            };

            CommandRequest {
                app_name: popup.app_name.clone(),
                project: popup.project.clone(),
                environment: popup.environment.clone(),
                command_type,
            }
        } else {
            return Ok(());
        };

        if let Some(popup) = &mut self.active_popup {
            popup.set_executing();
        }

        self.command_tx.send(request)?;
        Ok(())
    }
}
