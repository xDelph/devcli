use super::super::{ConfigMode, MainTab, MainView};
use crate::tui::theme::Theme;
use ratatui::{
    layout::{Alignment, Rect},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

impl MainView {
    /// Renders the footer with contextual keyboard shortcuts
    pub(crate) fn render_footer(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
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
                    "↑↓/jk: Navigate  a: Add Cmd  e: Edit Cmd  f: Env Files  E: Edit App  s: Set Default  D: Deps  d: Delete  q: Quit"
                }
                ConfigMode::Add | ConfigMode::Edit => {
                    "Tab/↑↓: Navigate Fields  Type: Edit  Enter: Save  Esc: Cancel"
                }
                ConfigMode::AddCommand | ConfigMode::EditCommand => {
                    "Tab: Switch Field  ←→: Move Cursor  Type: Edit  Enter: Save  Esc: Cancel"
                }
                ConfigMode::EditEnvFiles => {
                    "↑↓/jk: Navigate  a: Add  e: Edit  d: Delete  Esc: Close"
                }
                ConfigMode::AddEnvFile | ConfigMode::EditEnvFile => {
                    "Tab: Switch Field  ←→: Move Cursor  Type: Edit  Enter: Save  Esc: Cancel"
                }
                ConfigMode::EditDependencies => {
                    "↑↓/jk: Navigate  a: Add  d: Delete  Esc: Close"
                }
                ConfigMode::AddDependency => {
                    "↑↓/jk: Navigate  Enter: Add  Esc: Cancel"
                }
                ConfigMode::ConfirmDelete => {
                    "Y: Confirm  N/Esc: Cancel"
                }
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
