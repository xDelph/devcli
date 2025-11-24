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
    /// Renders the Status tab panel showing app details
    pub(crate) fn render_status_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let content = if let Some(app) = state.selected_app() {
            self.build_status_content(app, theme)
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

        let status_panel = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Details")
                .border_style(border_style),
        );

        frame.render_widget(status_panel, area);
    }

    /// Builds the content for the status panel
    fn build_status_content<'a>(&self, app: &'a AppStateData, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        lines.push(Line::from(vec![
            Span::styled("─ ", Style::default().fg(theme.border)),
            Span::styled(
                app.name.clone(),
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" ─", Style::default().fg(theme.border)),
        ]));
        lines.push(Line::from(""));

        let status_text = app.status.as_str();
        let status_color = if app.status.is_running() {
            theme.running
        } else {
            theme.stopped
        };

        lines.push(Line::from(vec![
            Span::styled("Status:      ", Style::default().fg(theme.text_dim)),
            Span::styled("● ", Style::default().fg(status_color)),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]));

        if let crate::tui::state::AppStatus::Running { pid, uptime, .. } = &app.status {
            lines.push(Line::from(vec![
                Span::styled("PID:         ", Style::default().fg(theme.text_dim)),
                Span::styled(pid.to_string(), Style::default().fg(theme.text)),
            ]));

            let uptime_str = Self::format_duration(uptime);
            lines.push(Line::from(vec![
                Span::styled("Uptime:      ", Style::default().fg(theme.text_dim)),
                Span::styled(uptime_str, Style::default().fg(theme.text)),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled("Type:        ", Style::default().fg(theme.text_dim)),
            Span::styled(app.app_type.clone(), Style::default().fg(theme.text)),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Project:     ", Style::default().fg(theme.text_dim)),
            Span::styled(app.project.clone(), Style::default().fg(theme.text)),
        ]));

        // Show stage information
        // For running apps, show active stage (may be overridden with --stage flag)
        // For stopped apps, show configured stage
        if app.status.is_running() {
            if let Some(ref active_stage) = app.active_stage {
                lines.push(Line::from(vec![
                    Span::styled("Stage:       ", Style::default().fg(theme.text_dim)),
                    Span::styled(
                        active_stage.to_uppercase(),
                        Style::default()
                            .fg(theme.secondary)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" (active)", Style::default().fg(theme.text_dim)),
                ]));
            }
        } else if let Some(ref stage) = app.stage {
            lines.push(Line::from(vec![
                Span::styled("Stage:       ", Style::default().fg(theme.text_dim)),
                Span::styled(stage.to_uppercase(), Style::default().fg(theme.secondary)),
            ]));
        }

        if let Some(path) = &app.path {
            lines.push(Line::from(vec![
                Span::styled("Path:        ", Style::default().fg(theme.text_dim)),
                Span::styled(path.clone(), Style::default().fg(theme.text)),
            ]));
        }

        // Show env files summary if configured
        if let Some(env_files) = &app.env_files {
            let total_stages = env_files.len();
            if total_stages > 0 {
                lines.push(Line::from(vec![
                    Span::styled("Env Files:   ", Style::default().fg(theme.text_dim)),
                    Span::styled(
                        format!("{} stage(s) configured", total_stages),
                        Style::default().fg(theme.secondary),
                    ),
                ]));
            }
        }

        if !app.dependencies.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Dependencies:",
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )));

            for dep in &app.dependencies {
                lines.push(Line::from(vec![
                    Span::styled("  • ", Style::default().fg(theme.text_dim)),
                    Span::styled(dep.clone(), Style::default().fg(theme.text)),
                ]));
            }
        }

        lines
    }
}
