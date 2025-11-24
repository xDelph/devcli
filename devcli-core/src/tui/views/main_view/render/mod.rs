pub mod app_list;
pub mod commands_panel;
pub mod config_panel;
pub mod footer;
pub mod logs_panel;
pub mod status_panel;
pub mod tab_bar;
pub mod utils;

use super::{MainTab, MainView};
use crate::tui::state::AppState;
use crate::tui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

impl MainView {
    /// Renders the main view with all its components
    pub(super) fn render_main(&self, frame: &mut Frame, state: &AppState, theme: &Theme) {
        let size = frame.area();

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Min(0),    // Content area
                Constraint::Length(3), // Footer with shortcuts
            ])
            .split(size);

        self.render_tab_bar(frame, main_chunks[0], theme);

        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Left panel - app list
                Constraint::Percentage(70), // Right panel - details
            ])
            .split(main_chunks[1]);

        self.render_app_list(frame, content_chunks[0], state, theme);

        match self.active_tab {
            MainTab::Status => self.render_status_panel(frame, content_chunks[1], state, theme),
            MainTab::Commands => self.render_commands_panel(frame, content_chunks[1], state, theme),
            MainTab::Logs => self.render_logs_panel(frame, content_chunks[1], state, theme),
            MainTab::Config => self.render_config_panel(frame, content_chunks[1], state, theme),
        }

        self.render_footer(frame, main_chunks[2], theme);
    }
}
