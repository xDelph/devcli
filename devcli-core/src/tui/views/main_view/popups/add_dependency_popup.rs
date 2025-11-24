use crate::config::loader::load_config;
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
    /// Renders the add dependency popup
    pub(crate) fn render_add_dependency_popup(
        &self,
        frame: &mut Frame,
        theme: &Theme,
        current_project: Option<&str>,
        current_app: Option<&str>,
    ) {
        let area = Self::centered_rect(70, 60, frame.area());

        frame.render_widget(ratatui::widgets::Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.primary))
            .title("Add Dependency - Select Project and App")
            .style(Style::default().bg(theme.bg));
        frame.render_widget(block, area);

        let inner = Rect {
            x: area.x + 2,
            y: area.y + 2,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(4),
        };

        let mut lines = Vec::new();

        match load_config() {
            Ok(config) => {
                let candidates =
                    self.get_sorted_dependency_candidates(&config, current_project, current_app);

                let mut current_flat_index = 0;

                for (proj_name, apps) in candidates {
                    // Always show project header
                    lines.push(Line::from(Span::styled(
                        format!("▼ {}", proj_name),
                        Style::default()
                            .fg(theme.primary)
                            .add_modifier(Modifier::BOLD),
                    )));

                    for app_name in apps {
                        let is_selected =
                            current_flat_index == self.popup_scroll_manager.selected_add_dep_idx;

                        let (style, prefix_style) = if is_selected {
                            (
                                Style::default().bg(theme.selected_bg).fg(theme.text),
                                Style::default().bg(theme.selected_bg).fg(theme.primary),
                            )
                        } else {
                            (
                                Style::default().fg(theme.text),
                                Style::default().fg(theme.primary),
                            )
                        };

                        let prefix = if is_selected { " > " } else { "   " };

                        lines.push(Line::from(vec![
                            Span::styled(prefix, prefix_style),
                            Span::styled(format!("● {}", app_name), style),
                        ]));

                        current_flat_index += 1;
                    }

                    lines.push(Line::from(""));
                }

                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "↑↓: Navigate  Enter: Select/Add  Esc: Back/Cancel",
                    Style::default().fg(theme.text_dim),
                )));
            }
            Err(e) => {
                lines.push(Line::from(Span::styled(
                    format!("Error loading config: {}", e),
                    Style::default().fg(theme.error),
                )));
            }
        }

        let content = Paragraph::new(lines);
        frame.render_widget(content, inner);
    }
}
