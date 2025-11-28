use crate::tui::theme::Theme;
use crate::tui::views::main_view::{ConfigField, ConfigMode, MainView};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

impl MainView {
    /// Renders the command edit form
    pub(crate) fn render_command_edit_form(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("─ ", Style::default().fg(theme.border)),
                Span::styled(
                    "Edit Command",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ─", Style::default().fg(theme.border)),
            ]),
            Line::from(""),
        ];

        let is_env_focused = self.config_focused_field == ConfigField::EditCommandEnv;
        let is_add_mode = self.config_mode == ConfigMode::AddCommand;

        let env_label_style = if is_env_focused && is_add_mode {
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            theme.style_text_dim()
        };

        let env_value_style = if is_env_focused && is_add_mode {
            theme.style_text().bg(theme.selected_bg)
        } else {
            theme.style_text()
        };

        let env_text = if is_add_mode {
            format!(
                "{} (Space to cycle)",
                self.config_form.edit_command_env.to_uppercase()
            )
        } else {
            self.config_form.edit_command_env.to_uppercase()
        };

        lines.push(Line::from(vec![
            Span::styled("  Environment: ", env_label_style),
            Span::styled(env_text, env_value_style),
        ]));

        lines.push(Line::from(""));

        // Command name
        self.render_form_field(
            &mut lines,
            "Command Name:",
            &self.config_form.edit_command_name,
            ConfigField::EditCommandName,
            theme,
        );

        lines.push(Line::from(""));

        // Command value
        self.render_form_field(
            &mut lines,
            "Command Value:",
            &self.config_form.edit_command_value,
            ConfigField::EditCommandValue,
            theme,
        );

        let config_panel = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Edit Command")
                .border_style(theme.style_text_primary())
                .style(theme.style_bg_default()),
        );

        frame.render_widget(config_panel, area);
    }
}
