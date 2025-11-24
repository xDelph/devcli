use super::super::{MainTab, MainView};
use crate::tui::theme::Theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Tabs},
    Frame,
};

impl MainView {
    /// Renders the tab bar at the top of the screen
    pub(crate) fn render_tab_bar(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let tab_titles = vec!["[1] Status", "[2] Commands", "[3] Logs", "[4] Config"];

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
                    .border_style(Style::default().fg(theme.border)),
            )
            .select(selected_idx)
            .style(Style::default().fg(theme.text))
            .highlight_style(
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(tabs, area);
    }
}
