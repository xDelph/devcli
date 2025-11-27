use super::super::{MainView, PanelFocus};
use crate::tui::log_manager::LogManager;
use crate::tui::state::{AppState, AppStateData};
use crate::tui::theme::Theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

impl MainView {
    /// Renders the Logs tab panel showing available log files
    pub(crate) fn render_logs_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let content = if let Some(app) = state.selected_app() {
            self.build_logs_content(app, theme)
        } else {
            vec![Line::from(Span::styled(
                "No app selected",
                Style::default().fg(theme.text_dim),
            ))]
        };

        let border_style = if self.focus == PanelFocus::DetailPanel {
            Style::default().fg(theme.primary)
        } else {
            Style::default().fg(theme.border)
        };

        let visible_height = area.height.saturating_sub(2) as usize;
        let scroll_offset =
            self.calculate_scroll_offset(self.selected_log_idx, visible_height, content.len());

        let logs_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Log Files")
                    .border_style(border_style),
            )
            .scroll((scroll_offset as u16, 0));

        frame.render_widget(logs_panel, area);
    }

    /// Builds the content for the logs panel
    fn build_logs_content<'a>(&self, app: &'a AppStateData, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("─ ", Style::default().fg(theme.border)),
            Span::styled(
                format!("{} logs", app.name),
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ─", Style::default().fg(theme.border)),
        ]));
        lines.push(Line::from(""));

        match self.log_manager.list_logs_for_app(&app.project, &app.name) {
            Ok(log_files) => {
                if log_files.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "No log files found for this app.",
                        Style::default().fg(theme.text_dim),
                    )));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "Log files will appear here after you start the app.",
                        Style::default().fg(theme.text_dim),
                    )));
                } else {
                    for (idx, log_file) in log_files.iter().enumerate() {
                        let is_selected =
                            idx == self.selected_log_idx && self.focus == PanelFocus::DetailPanel;

                        let size_str = LogManager::format_file_size(log_file.size);
                        let date_str = LogManager::format_relative_date(&log_file.modified);

                        let (prefix, text_style) = if is_selected {
                            (
                                " >",
                                Style::default()
                                    .fg(theme.text)
                                    .bg(theme.selected_bg)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            ("  ", Style::default().fg(theme.text))
                        };

                        lines.push(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(theme.primary)),
                            Span::styled(format!(" {} ({})", date_str, size_str), text_style),
                        ]));
                    }
                }
            }
            Err(e) => {
                lines.push(Line::from(Span::styled(
                    format!("Error reading log files: {}", e),
                    Style::default().fg(theme.error),
                )));
            }
        }

        lines
    }
}
