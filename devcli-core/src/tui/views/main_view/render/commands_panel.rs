use super::super::{MainView, PanelFocus};
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
    /// Renders the Commands tab panel showing available commands
    pub(crate) fn render_commands_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let content = if let Some(app) = state.selected_app() {
            self.build_commands_content(app, theme)
        } else {
            vec![Line::from(Span::styled(
                "No app selected",
                theme.style_text_dim(),
            ))]
        };

        let border_style = if self.focus == PanelFocus::DetailPanel {
            theme.style_text_primary()
        } else {
            Style::default().fg(theme.border)
        };

        let visible_height = area.height.saturating_sub(2) as usize;
        let scroll_offset =
            self.calculate_scroll_offset(self.selected_command_idx, visible_height, content.len());

        let commands_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Available Commands")
                    .border_style(border_style),
            )
            .scroll((scroll_offset as u16, 0));

        frame.render_widget(commands_panel, area);
    }

    /// Builds the content for the commands panel
    fn build_commands_content<'a>(&self, app: &'a AppStateData, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("─ ", Style::default().fg(theme.border)),
            Span::styled(
                format!("{} commands", app.name),
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ─", Style::default().fg(theme.border)),
        ]));
        lines.push(Line::from(""));

        if app.commands.is_empty() {
            lines.push(Line::from(Span::styled(
                "No commands configured",
                theme.style_text_dim(),
            )));
            return lines;
        }

        let max_name_len = app
            .commands
            .values()
            .flat_map(|cmds| cmds.iter())
            .map(|cmd| cmd.name.len())
            .max()
            .unwrap_or(12)
            .max(12);

        let mut global_cmd_idx = 0;

        // Iterate through environments in standard order
        use crate::config::models::Environment;
        for env in Environment::all() {
            let env_str = env.as_str();
            if let Some(commands) = app.commands.get(env_str) {
                lines.push(Line::from(Span::styled(
                    format!("{}:", env.display_name()),
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )));

                for cmd in commands.iter() {
                    let is_selected = global_cmd_idx == self.selected_command_idx
                        && self.focus == PanelFocus::DetailPanel;

                    let (prefix, name_style, cmd_style) = if is_selected {
                        (
                            " >",
                            Style::default()
                                .fg(theme.text)
                                .bg(theme.selected_bg)
                                .add_modifier(Modifier::BOLD),
                            theme.style_text_dim().bg(theme.selected_bg),
                        )
                    } else {
                        (
                            "  ",
                            theme.style_text(),
                            theme.style_text_dim(),
                        )
                    };

                    lines.push(Line::from(vec![
                        Span::styled(prefix, theme.style_text_primary()),
                        Span::styled(
                            format!(" {:<width$}", cmd.name, width = max_name_len),
                            name_style,
                        ),
                        Span::styled("  ".to_string(), Style::default()),
                        Span::styled(cmd.command.clone(), cmd_style),
                    ]));

                    global_cmd_idx += 1;
                }

                lines.push(Line::from(""));
            }
        }

        lines
    }
}
