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
    /// Renders the add/edit environment file form
    pub(crate) fn render_add_env_file_form(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let title = if self.config_mode == ConfigMode::EditEnvFile {
            "Edit Environment File"
        } else {
            "Add Environment File"
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled("─ ", Style::default().fg(theme.border)),
                Span::styled(
                    title,
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ─", Style::default().fg(theme.border)),
            ]),
            Line::from(""),
        ];

        // Stage field
        self.render_form_field(
            &mut lines,
            "Stage (e.g., qa, prod, dev):",
            &self.config_form.env_file_stage,
            ConfigField::EnvFileStage,
            theme,
        );

        lines.push(Line::from(""));

        // Context field
        self.render_form_field(
            &mut lines,
            "Context (local, docker, orbstack, k8s):",
            &self.config_form.env_file_context,
            ConfigField::EnvFileContext,
            theme,
        );

        lines.push(Line::from(""));

        // File path field
        self.render_form_field(
            &mut lines,
            "File Path (relative to app root):",
            &self.config_form.env_file_path,
            ConfigField::EnvFilePath,
            theme,
        );

        let config_panel = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(theme.style_text_primary())
                .style(theme.style_bg_default()),
        );

        frame.render_widget(config_panel, area);
    }
}
