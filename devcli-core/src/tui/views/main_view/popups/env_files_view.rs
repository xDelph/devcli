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
    /// Renders the environment files view (full panel like config view)
    pub(crate) fn render_env_files_view(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let mut lines = Vec::new();

        if let Some(app) = state.selected_app() {
            lines.push(Line::from(vec![
                Span::styled("─ ", Style::default().fg(theme.border)),
                Span::styled("● ", Style::default().fg(theme.primary)),
                Span::styled(
                    format!("{} - Environment Files", app.name),
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ─", Style::default().fg(theme.border)),
            ]));
            lines.push(Line::from(""));

            if let Some(env_files) = &app.env_files {
                // Build a flat list of all entries (stage, context, file_path) for navigation
                let mut entries: Vec<(String, String, String)> = Vec::new();
                for (stage, contexts) in env_files {
                    let mut sorted_contexts: Vec<_> = contexts.iter().collect();
                    sorted_contexts.sort_by_key(|(context, _)| context.as_str());

                    for (context, file_path) in sorted_contexts {
                        entries.push((stage.clone(), context.clone(), file_path.clone()));
                    }
                }
                entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

                if entries.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "No environment files configured.",
                        Style::default().fg(theme.text_dim),
                    )));
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "Press 'a' to add an environment file.",
                        Style::default().fg(theme.text_dim),
                    )));
                } else {
                    // Display grouped by stage, but track flat index for selection
                    let mut sorted_stages: Vec<_> = env_files.iter().collect();
                    sorted_stages.sort_by_key(|(stage, _)| stage.as_str());

                    let mut flat_idx = 0;
                    for (stage, contexts) in sorted_stages {
                        // Stage header
                        lines.push(Line::from(vec![
                            Span::styled("  ", Style::default()),
                            Span::styled(
                                format!("{}:", stage),
                                Style::default()
                                    .fg(theme.primary)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]));

                        // Sort contexts for consistent display
                        let mut sorted_contexts: Vec<_> = contexts.iter().collect();
                        sorted_contexts.sort_by_key(|(context, _)| context.as_str());

                        for (context, file_path) in sorted_contexts {
                            let is_selected =
                                flat_idx == self.popup_scroll_manager.selected_env_file_idx;
                            let entry_style = if is_selected {
                                Style::default().bg(theme.selected_bg).fg(theme.text)
                            } else {
                                Style::default().fg(theme.text)
                            };

                            let prefix = if is_selected { "   > " } else { "     " };
                            lines.push(Line::from(vec![
                                Span::styled(
                                    prefix.to_string(),
                                    Style::default().fg(theme.primary),
                                ),
                                Span::styled(
                                    format!("{}: ", context),
                                    Style::default().fg(theme.secondary),
                                ),
                                Span::styled(file_path.clone(), entry_style),
                            ]));

                            flat_idx += 1;
                        }

                        lines.push(Line::from("")); // Blank line between stages
                    }
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "No environment files configured.",
                    Style::default().fg(theme.text_dim),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Press 'a' to add an environment file.",
                    Style::default().fg(theme.text_dim),
                )));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Actions:",
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(vec![
                Span::styled("  a", Style::default().fg(theme.primary)),
                Span::styled(" - Add environment file", Style::default().fg(theme.text)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  e", Style::default().fg(theme.primary)),
                Span::styled(" - Edit selected entry", Style::default().fg(theme.text)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  d", Style::default().fg(theme.primary)),
                Span::styled(" - Delete selected entry", Style::default().fg(theme.text)),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  Esc", Style::default().fg(theme.primary)),
                Span::styled(" - Back to config view", Style::default().fg(theme.text)),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                "No app selected",
                Style::default().fg(theme.text_dim),
            )));
        }

        let border_style = Style::default().fg(theme.primary);

        let panel = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Environment Files")
                .border_style(border_style)
                .style(Style::default().bg(theme.bg)),
        );

        frame.render_widget(panel, area);
    }
}
