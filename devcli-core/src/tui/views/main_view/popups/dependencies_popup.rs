use crate::tui::state::AppState;
use crate::tui::theme::Theme;
use crate::tui::views::main_view::MainView;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

impl MainView {
    /// Renders the dependencies popup
    pub(crate) fn render_dependencies_popup(
        &self,
        frame: &mut Frame,
        state: &AppState,
        theme: &Theme,
    ) {
        let area = Self::centered_rect(60, 50, frame.area());

        frame.render_widget(ratatui::widgets::Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.primary))
            .title("Edit Dependencies")
            .style(Style::default().bg(theme.bg));
        frame.render_widget(block, area);

        let inner = Rect {
            x: area.x + 2,
            y: area.y + 2,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(4),
        };

        let mut lines = Vec::new();

        if let Some(app) = state.selected_app() {
            if app.dependencies.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No dependencies configured.",
                    Style::default().fg(theme.text_dim),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Press 'a' to add a dependency.",
                    Style::default().fg(theme.text_dim),
                )));
            } else {
                for (idx, dep) in app.dependencies.iter().enumerate() {
                    let is_selected = idx == self.popup_scroll_manager.selected_dependency_idx;
                    let style = if is_selected {
                        Style::default()
                            .bg(theme.selected_bg)
                            .fg(theme.text)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    };

                    let prefix = if is_selected { " > " } else { "   " };
                    lines.push(Line::from(vec![
                        Span::styled(prefix.to_string(), Style::default().fg(theme.primary)),
                        Span::styled(dep.clone(), style),
                    ]));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "a: Add  d: Delete  Esc: Close",
                Style::default().fg(theme.text_dim),
            )));
        }

        let content = Paragraph::new(lines);
        frame.render_widget(content, inner);
    }
}
