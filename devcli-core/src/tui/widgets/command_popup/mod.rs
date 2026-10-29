// Command execution popup widget
// Shows confirmation dialog before executing commands and displays execution feedback
// Follows the design pattern of modal dialogs with clear user feedback

use crate::config::{list_all_apps, loader::load_config_cached};
use crate::tui::views::log_viewer::app_color_manager::AppColorManager;
use std::cell::Cell;

mod render;

/// Command execution popup that shows confirmation and execution status
/// Manages the lifecycle of command execution from confirmation to completion
#[derive(Debug, Clone)]
pub struct CommandPopup {
    /// Name of the command being executed (e.g., "start", "test")
    pub command_name: String,
    /// The actual command text that will be executed (e.g., "npm start")
    pub command_text: String,
    /// Name of the app this command is for
    pub app_name: String,
    /// Project this app belongs to
    pub project: String,
    /// Environment where the command will run (local, docker, k8s)
    pub environment: String,
    /// Current state of the popup (confirm, executing, success, error)
    state: PopupState,
    /// Output lines from command execution (for displaying feedback)
    output_lines: Vec<String>,
    /// Scroll offset for viewing logs
    scroll_offset: usize,
    /// Auto-scroll to bottom when new lines arrive
    auto_scroll: bool,
    /// Last known visible lines (updated during render using interior mutability)
    last_visible_lines: Cell<usize>,
    /// Whether this popup allows environment selection
    allow_env_selection: bool,
    /// Available environments for selection
    available_envs: Vec<String>,
    /// Currently selected environment index
    selected_env_index: usize,
    /// Current app status (for display in header)
    pub app_status: Option<String>,
    /// Color manager for consistent app name coloring
    color_manager: AppColorManager,
    /// All app names from config (for colorizing any app name that appears in logs)
    all_app_names: Vec<String>,
    /// The command request this popup launched (results of other requests are
    /// ignored).
    pub(crate) request_id: Option<u64>,
}

/// Represents the different states of the command popup
/// Each state determines what content is displayed to the user
#[derive(Debug, Clone, PartialEq)]
pub enum PopupState {
    /// Asking user to confirm command execution
    Confirm,
    /// Command is currently executing in the background
    Executing,
    /// Command completed successfully with optional message
    Success(String),
    /// Command failed with error message
    Error(String),
}

impl CommandPopup {
    /// Creates a new command popup in the confirmation state
    ///
    /// # Arguments
    /// * `command_name` - Display name of the command (e.g., "start")
    /// * `command_text` - Actual command to execute (e.g., "npm start")
    /// * `app_name` - Name of the app this command is for
    /// * `project` - Project this app belongs to
    /// * `environment` - Environment context (local, docker, k8s)
    pub fn new(
        command_name: String,
        command_text: String,
        app_name: String,
        project: String,
        environment: String,
    ) -> Self {
        // Load config to get all app names for colorization
        // If config loading fails, use empty list (graceful degradation)
        let all_app_names = load_config_cached()
            .map(|config| {
                list_all_apps(&config)
                    .into_iter()
                    .map(|(_, app_name, _)| app_name)
                    .collect()
            })
            .unwrap_or_else(|_| Vec::new());

        Self {
            command_name,
            command_text,
            app_name,
            project,
            environment: environment.clone(),
            state: PopupState::Confirm,
            output_lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            last_visible_lines: Cell::new(30), // Default estimate
            allow_env_selection: false,
            available_envs: vec![environment],
            selected_env_index: 0,
            app_status: None,
            color_manager: AppColorManager::new(),
            all_app_names,
            request_id: None,
        }
    }

    /// Creates a new command popup with environment selection enabled
    ///
    /// # Arguments
    /// * `command_name` - Display name of the command (e.g., "start")
    /// * `command_text` - Actual command to execute (e.g., "npm start")
    /// * `app_name` - Name of the app this command is for
    /// * `project` - Project this app belongs to
    /// * `default_env` - Default environment to select
    pub fn new_with_env_selection(
        command_name: String,
        command_text: String,
        app_name: String,
        project: String,
        default_env: String,
    ) -> Self {
        use crate::config::models::Environment;
        let available_envs: Vec<String> = Environment::all()
            .iter()
            .map(|e| e.as_str().to_string())
            .collect();
        let selected_env_index = available_envs
            .iter()
            .position(|e| e == &default_env)
            .unwrap_or(0);

        // Load config to get all app names for colorization
        // If config loading fails, use empty list (graceful degradation)
        let all_app_names = load_config_cached()
            .map(|config| {
                list_all_apps(&config)
                    .into_iter()
                    .map(|(_, app_name, _)| app_name)
                    .collect()
            })
            .unwrap_or_else(|_| Vec::new());

        Self {
            command_name,
            command_text,
            app_name,
            project,
            environment: available_envs[selected_env_index].clone(),
            state: PopupState::Confirm,
            output_lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            last_visible_lines: Cell::new(30),
            allow_env_selection: true,
            available_envs,
            selected_env_index,
            app_status: None,
            color_manager: AppColorManager::new(),
            all_app_names,
            request_id: None,
        }
    }

    /// Gets the current state of the popup
    pub fn state(&self) -> &PopupState {
        &self.state
    }

    /// Sets the popup state to executing
    /// Called when user confirms and command starts running
    pub fn set_executing(&mut self) {
        self.state = PopupState::Executing;
        self.output_lines.clear();
    }

    /// Sets the popup state to success with a message
    /// Called when command completes successfully
    pub fn set_success(&mut self, message: String) {
        self.state = PopupState::Success(message);
    }

    /// Sets the popup state to error with a message
    /// Called when command fails
    pub fn set_error(&mut self, message: String) {
        self.state = PopupState::Error(message);
    }

    /// Adds an output line from the executing command
    /// Used to show real-time feedback during execution
    pub fn add_output_line(&mut self, line: String) {
        // Trim trailing whitespace
        let trimmed = line.trim_end().to_string();
        self.output_lines.push(trimmed);
        // Keep only the last 1000 lines
        if self.output_lines.len() > 1000 {
            self.output_lines.remove(0);
            // Adjust scroll offset if needed
            if self.scroll_offset > 0 {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
        }
    }

    /// Scroll up in the log output
    pub fn scroll_up(&mut self) {
        // If auto-scroll is on, we need to set scroll_offset to current bottom position first
        if self.auto_scroll {
            let max_scroll = self
                .output_lines
                .len()
                .saturating_sub(self.last_visible_lines.get());
            self.scroll_offset = max_scroll;
            self.auto_scroll = false;
        }

        // Scroll up by 1 line
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll down in the log output
    pub fn scroll_down(&mut self) {
        // If we have no lines, nothing to do
        if self.output_lines.is_empty() {
            return;
        }

        // Disable auto-scroll when user manually scrolls
        self.auto_scroll = false;

        // Calculate max scroll based on last known visible lines
        let max_scroll = self
            .output_lines
            .len()
            .saturating_sub(self.last_visible_lines.get());

        // Scroll down by 1 line
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }

        // If we reached the bottom, re-enable auto-scroll
        if self.scroll_offset >= max_scroll {
            self.auto_scroll = true;
        }
    }

    /// Enable auto-scroll to bottom
    pub fn enable_auto_scroll(&mut self) {
        self.auto_scroll = true;
    }

    /// Reset scroll to top
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = false;
    }

    /// Cycle to next environment (only if env selection is enabled)
    pub fn next_environment(&mut self) {
        if self.allow_env_selection && self.state == PopupState::Confirm {
            self.selected_env_index = (self.selected_env_index + 1) % self.available_envs.len();
            self.environment = self.available_envs[self.selected_env_index].clone();
        }
    }

    /// Cycle to previous environment (only if env selection is enabled)
    pub fn prev_environment(&mut self) {
        if self.allow_env_selection && self.state == PopupState::Confirm {
            if self.selected_env_index == 0 {
                self.selected_env_index = self.available_envs.len() - 1;
            } else {
                self.selected_env_index -= 1;
            }
            self.environment = self.available_envs[self.selected_env_index].clone();
        }
    }

    /// Check if environment selection is allowed
    pub fn allows_env_selection(&self) -> bool {
        self.allow_env_selection
    }

    /// Update the app status for display in header
    pub fn update_status(&mut self, status: String) {
        self.app_status = Some(status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_creation() {
        let popup = CommandPopup::new(
            "start".to_string(),
            "npm start".to_string(),
            "api-server".to_string(),
            "my-project".to_string(),
            "local".to_string(),
        );

        assert_eq!(popup.state(), &PopupState::Confirm);
        assert_eq!(popup.command_name, "start");
        assert_eq!(popup.command_text, "npm start");
        assert_eq!(popup.app_name, "api-server");
        assert_eq!(popup.project, "my-project");
        assert_eq!(popup.environment, "local");
    }

    #[test]
    fn test_popup_state_transitions() {
        let mut popup = CommandPopup::new(
            "start".to_string(),
            "npm start".to_string(),
            "api-server".to_string(),
            "my-project".to_string(),
            "local".to_string(),
        );

        // Initial state is Confirm
        assert_eq!(popup.state(), &PopupState::Confirm);

        // Transition to Executing
        popup.set_executing();
        assert_eq!(popup.state(), &PopupState::Executing);

        // Transition to Success
        popup.set_success("App started successfully".to_string());
        assert!(matches!(popup.state(), PopupState::Success(_)));

        // Reset and transition to Error
        popup.state = PopupState::Confirm;
        popup.set_error("Failed to start app".to_string());
        assert!(matches!(popup.state(), PopupState::Error(_)));
    }

    #[test]
    fn test_output_lines_limit() {
        let mut popup = CommandPopup::new(
            "start".to_string(),
            "npm start".to_string(),
            "api-server".to_string(),
            "my-project".to_string(),
            "local".to_string(),
        );

        // Add more than 1000 lines
        for i in 0..1050 {
            popup.add_output_line(format!("Line {}", i));
        }

        // Should only keep the last 1000 lines
        assert_eq!(popup.output_lines.len(), 1000);
        assert_eq!(popup.output_lines[0], "Line 50");
        assert_eq!(popup.output_lines[999], "Line 1049");
    }
}
