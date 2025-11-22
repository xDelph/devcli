// Navigation and scroll management for MainView

use super::MainView;
use crate::tui::state::{AppState, AppStateData};

impl MainView {
    /// Counts the total number of commands for an app across all environments
    pub(super) fn count_total_commands(app: &AppStateData) -> usize {
        app.commands.values().map(|cmds| cmds.len()).sum()
    }

    /// Gets the currently selected command info based on the selected index
    pub fn get_selected_command<'a>(
        &self,
        app: &'a AppStateData,
    ) -> Option<(&'a str, &'a crate::tui::state::CommandInfo)> {
        self.get_command_by_index(app, self.selected_command_idx)
    }

    /// Returns (environment, command_info) tuple for a specific command index
    pub fn get_command_by_index<'a>(
        &self,
        app: &'a AppStateData,
        command_idx: usize,
    ) -> Option<(&'a str, &'a crate::tui::state::CommandInfo)> {
        use crate::config::models::Environment;
        let mut current_idx = 0;
        
        // Iterate through environments in standard order
        for env in Environment::all() {
            let env_str = env.as_str();
            if let Some(commands) = app.commands.get(env_str) {
                if command_idx < current_idx + commands.len() {
                    let cmd_idx = command_idx - current_idx;
                    return Some((env_str, &commands[cmd_idx]));
                }
                current_idx += commands.len();
            }
        }
        
        None
    }

    /// Calculates the scroll offset to keep the selected item visible
    pub(super) fn calculate_scroll_offset(&self, selected_idx: usize, visible_height: usize, total_lines: usize) -> usize {
        if total_lines <= visible_height {
            return 0;
        }

        if selected_idx < visible_height / 2 {
            0
        } else if selected_idx >= total_lines.saturating_sub(visible_height / 2) {
            total_lines.saturating_sub(visible_height)
        } else {
            selected_idx.saturating_sub(visible_height / 2)
        }
    }

    /// Calculates which line the selected app is on in the rendered list
    pub(super) fn calculate_selected_app_line(&self, state: &AppState) -> usize {
        let mut line = 0;
        
        for (proj_idx, project) in state.projects.iter().enumerate() {
            if proj_idx == state.selected_project_idx && !project.expanded {
                return line;
            }
            line += 1;
            
            if project.expanded {
                for (app_idx, _app) in project.apps.iter().enumerate() {
                    if proj_idx == state.selected_project_idx && app_idx == state.selected_app_idx {
                        return line;
                    }
                    line += 1;
                }
            }
            
            if proj_idx < state.projects.len() - 1 {
                line += 1;
            }
        }
        
        0
    }

    /// Calculates which line the selected command is on in the config panel
    pub(super) fn calculate_selected_config_command_line(&self, state: &AppState) -> usize {
        let mut line = 0;
        
        // Header and app info section
        line += 1; // Header line
        line += 1; // Empty line
        line += 3; // Project, Type, Path
        line += 1; // Empty line
        line += 1; // "Commands:" header
        
        if let Some(app) = state.selected_app() {
            let mut global_cmd_idx = 0;
            
            // Count LOCAL commands
            if let Some(commands) = app.commands.get("local") {
                line += 1; // "LOCAL:" header
                if global_cmd_idx + commands.len() > self.selected_config_command_idx {
                    return line + (self.selected_config_command_idx - global_cmd_idx);
                }
                line += commands.len();
                global_cmd_idx += commands.len();
                line += 1; // Empty line after section
            }
            
            // Count DOCKER commands
            if let Some(commands) = app.commands.get("docker") {
                line += 1; // "DOCKER:" header
                if global_cmd_idx + commands.len() > self.selected_config_command_idx {
                    return line + (self.selected_config_command_idx - global_cmd_idx);
                }
                line += commands.len();
                global_cmd_idx += commands.len();
                line += 1; // Empty line after section
            }
            
            // Count ORBSTACK commands
            if let Some(commands) = app.commands.get("orbstack") {
                line += 1; // "ORBSTACK:" header
                if global_cmd_idx + commands.len() > self.selected_config_command_idx {
                    return line + (self.selected_config_command_idx - global_cmd_idx);
                }
                line += commands.len();
                global_cmd_idx += commands.len();
                line += 1; // Empty line after section
            }
            
            // Count K8S commands
            if let Some(commands) = app.commands.get("k8s") {
                line += 1; // "K8S:" header
                if global_cmd_idx + commands.len() > self.selected_config_command_idx {
                    return line + (self.selected_config_command_idx - global_cmd_idx);
                }
                line += commands.len();
            }
        }
        
        line
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
}
