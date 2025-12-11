use super::super::{MainTab, MainView};
use crate::tui::theme::Theme;
use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Tabs},
    Frame,
};

impl MainView {
    /// Renders the tab bar at the top of the screen
    pub(crate) fn render_tab_bar(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let tab_titles = vec!["[1] Status", "[2] Config"];

        let selected_idx = match self.active_tab {
            MainTab::Status => 0,
            MainTab::Config => 1,
        };

        let tabs = Tabs::new(tab_titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style_border_default()),
            )
            .select(selected_idx)
            .style(theme.style_text())
            .highlight_style(theme.style_text_primary_bold());

        frame.render_widget(tabs, area);
    }
}
