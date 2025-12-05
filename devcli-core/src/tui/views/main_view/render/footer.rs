use super::super::{ConfigMode, MainTab, MainView};
use crate::tui::state::AppState;
use crate::tui::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

impl MainView {
    /// Renders the footer with contextual keyboard shortcuts
    pub(crate) fn render_footer(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let shortcuts = match self.active_tab {
            MainTab::Status => {
                let mut parts = vec!["↑↓/jk: Navigate", "←→: Switch Panel"];
                if let Some(app) = state.selected_app() {
                    if app.status.is_running() {
                        parts.push("s: Stop");
                        parts.push("r: Restart");
                        parts.push("l: Quick Log");
                    } else {
                        parts.push("s: Start");
                    }
                } else {
                    parts.push("s: Start/Stop");
                }
                parts.push("?: Help");
                parts.push("q: Quit");
                parts.join("  ")
            }
            MainTab::Commands => {
                "↑↓/jk: Navigate  ←→: Switch Panel  Tab/1-4: Switch Tab  Enter: Execute  q: Quit"
                    .to_string()
            }
            MainTab::Logs => {
                "↑↓/jk: Navigate  ←→: Switch Panel  Tab/1-4: Switch Tab  Enter: View Log  q: Quit"
                    .to_string()
            }
            MainTab::Config => match self.config_mode {
                ConfigMode::View => {
                    "↑↓/jk: Navigate  a: Add Cmd  e: Edit Cmd  f: Env Files  E: Edit App  s: Set Default  D: Deps  d: Delete  q: Quit".to_string()
                }
                ConfigMode::Add | ConfigMode::Edit => {
                    "Tab/↑↓: Navigate Fields  Type: Edit  Enter: Save  Esc: Cancel".to_string()
                }
                ConfigMode::AddCommand | ConfigMode::EditCommand => {
                    "Tab: Switch Field  ←→: Move Cursor  Type: Edit  Enter: Save  Esc: Cancel"
                        .to_string()
                }
                ConfigMode::EditEnvFiles => {
                    "↑↓/jk: Navigate  a: Add  e: Edit  d: Delete  Esc: Close".to_string()
                }
                ConfigMode::AddEnvFile | ConfigMode::EditEnvFile => {
                    "Tab: Switch Field  ←→: Move Cursor  Type: Edit  Enter: Save  Esc: Cancel"
                        .to_string()
                }
                ConfigMode::EditDependencies => {
                    "↑↓/jk: Navigate  a: Add  d: Delete  Esc: Close".to_string()
                }
                ConfigMode::AddDependency => {
                    "↑↓/jk: Navigate  Enter: Add  Esc: Cancel".to_string()
                }
                ConfigMode::ConfirmDelete => "Y: Confirm  N/Esc: Cancel".to_string(),
            },
        };

        let footer = Paragraph::new(shortcuts)
            .style(theme.style_text_dim())
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style_border_default()),
            );

        frame.render_widget(footer, area);
    }
}
