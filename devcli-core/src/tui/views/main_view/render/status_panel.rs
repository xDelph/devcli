use super::super::{MainView, PanelFocus};
use crate::tui::state::{AppState, AppStateData, HealthStatus};
use crate::tui::theme::Theme;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

impl MainView {
    /// Renders the Status tab panel showing app details
    pub(crate) fn render_status_panel(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        // Split the area into two sections: Status (top) and Logs (bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10), // Status info height (adjustable or dynamic)
                Constraint::Min(10),    // Log view takes remaining space
            ])
            .split(area);

        let status_area = chunks[0];
        let log_area = chunks[1];

        // --- Render Status Section ---
        let content = if let Some(app) = state.selected_app() {
            self.build_status_content(app, state, theme)
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

        let status_panel = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Details")
                .border_style(border_style),
        );

        frame.render_widget(status_panel, status_area);

        // --- Render Log File List ---
        let is_focused = self.focus == PanelFocus::DetailPanel;
        let border_style = if is_focused {
            theme.style_text_primary()
        } else {
            Style::default().fg(theme.border)
        };

        if let Some(app) = state.selected_app() {
            if let Ok(mut logs) = self.log_manager.list_logs_for_app(&app.project, &app.name) {
                // Sort by modified time descending (newest first)
                logs.sort_by(|a, b| b.modified.cmp(&a.modified));

                if logs.is_empty() {
                    let placeholder = Paragraph::new("No logs available")
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title("Logs")
                                .border_style(border_style),
                        )
                        .style(theme.style_text_dim());
                    frame.render_widget(placeholder, log_area);
                } else {
                    use ratatui::widgets::{List, ListItem};

                    // Build items with blank separators between date groups
                    let mut items: Vec<ListItem> = Vec::new();
                    let mut last_date_category: Option<String> = None;

                    for (i, log) in logs.iter().enumerate() {
                        // Get the date category (Today, Yesterday, X days ago, or YYYY-MM-DD)
                        let date_category =
                            crate::tui::log_manager::LogManager::format_relative_date(
                                log.modified.with_timezone(&chrono::Local).date_naive(),
                            );

                        // Check if we need a separator (date changed)
                        if let Some(ref last) = last_date_category {
                            // Extract just the date part (without time for "Today HH:MM")
                            let last_base = last.split_whitespace().next().unwrap_or(last);
                            let curr_base = date_category
                                .split_whitespace()
                                .next()
                                .unwrap_or(&date_category);
                            if last_base != curr_base {
                                items.push(ListItem::new(""));
                            }
                        }
                        last_date_category = Some(date_category);

                        let style = if i == self.selected_log_idx && is_focused {
                            theme.style_text_primary_bold()
                        } else if i == self.selected_log_idx {
                            theme.style_text()
                        } else {
                            theme.style_text_dim()
                        };

                        let file_name = log
                            .path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");

                        let label =
                            crate::tui::log_manager::LogManager::format_log_label(file_name);
                        items.push(ListItem::new(label).style(style));
                    }

                    let list = List::new(items).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Logs")
                            .border_style(border_style),
                    );
                    frame.render_widget(list, log_area);
                }
            } else {
                let placeholder = Paragraph::new("Error reading logs")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Logs")
                            .border_style(border_style),
                    )
                    .style(theme.style_text_dim());
                frame.render_widget(placeholder, log_area);
            }
        } else {
            let placeholder = Paragraph::new("No app selected")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Logs")
                        .border_style(border_style),
                )
                .style(theme.style_text_dim());
            frame.render_widget(placeholder, log_area);
        }
    }

    /// Builds the content for the status panel
    fn build_status_content<'a>(
        &self,
        app: &'a AppStateData,
        state: &'a AppState,
        theme: &'a Theme,
    ) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        let app_display_name = app.alternative_name.as_ref().unwrap_or(&app.name);
        lines.push(Line::from(vec![
            Span::styled("─ ", Style::default().fg(theme.border)),
            Span::styled(
                app_display_name.clone(),
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
            Span::styled("Status:      ", theme.style_text_dim()),
            Span::styled("● ", Style::default().fg(status_color)),
            Span::styled(status_text, Style::default().fg(status_color)),
        ]));

        if let crate::tui::state::AppStatus::Running { pid, uptime, .. } = &app.status {
            lines.push(Line::from(vec![
                Span::styled("PID:         ", theme.style_text_dim()),
                Span::styled(pid.to_string(), theme.style_text()),
            ]));

            let uptime_str = Self::format_duration(uptime);
            lines.push(Line::from(vec![
                Span::styled("Uptime:      ", theme.style_text_dim()),
                Span::styled(uptime_str, theme.style_text()),
            ]));
        }

        lines.push(Line::from(vec![
            Span::styled("Type:        ", theme.style_text_dim()),
            Span::styled(app.app_type.clone(), theme.style_text()),
        ]));

        // Find the project to get its alternative_name
        let project_display_name = state
            .projects
            .iter()
            .find(|p| p.name == app.project)
            .and_then(|p| p.alternative_name.as_ref())
            .unwrap_or(&app.project);

        lines.push(Line::from(vec![
            Span::styled("Project:     ", theme.style_text_dim()),
            Span::styled(project_display_name.clone(), theme.style_text()),
        ]));

        // Show stage information
        // For running apps, show active stage (may be overridden with --stage flag)
        // For stopped apps, show configured stage
        if app.status.is_running() {
            if let Some(ref active_stage) = app.active_stage {
                lines.push(Line::from(vec![
                    Span::styled("Stage:       ", theme.style_text_dim()),
                    Span::styled(
                        active_stage.to_uppercase(),
                        Style::default()
                            .fg(theme.secondary)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" (active)", theme.style_text_dim()),
                ]));
            }
        } else if let Some(ref stage) = app.stage {
            lines.push(Line::from(vec![
                Span::styled("Stage:       ", theme.style_text_dim()),
                Span::styled(stage.to_uppercase(), theme.style_text_secondary()),
            ]));
        }

        if let Some(path) = &app.path {
            lines.push(Line::from(vec![
                Span::styled("Path:        ", theme.style_text_dim()),
                Span::styled(path.clone(), theme.style_text()),
            ]));
        }

        // Show env files summary if configured
        if let Some(env_files) = &app.env_files {
            let total_stages = env_files.len();
            if total_stages > 0 {
                lines.push(Line::from(vec![
                    Span::styled("Env Files:   ", theme.style_text_dim()),
                    Span::styled(
                        format!("{} stage(s) configured", total_stages),
                        theme.style_text_secondary(),
                    ),
                ]));
            }
        }

        // Show health status
        match &app.health_status {
            HealthStatus::Healthy => {
                lines.push(Line::from(vec![
                    Span::styled("Health:      ", theme.style_text_dim()),
                    Span::styled("✓ ", Style::default().fg(theme.running)),
                    Span::styled("Healthy", Style::default().fg(theme.running)),
                ]));
            }
            HealthStatus::Unhealthy { failures, .. } => {
                lines.push(Line::from(vec![
                    Span::styled("Health:      ", theme.style_text_dim()),
                    Span::styled("✗ ", Style::default().fg(theme.error)),
                    Span::styled(
                        format!("Unhealthy ({} failures)", failures),
                        Style::default().fg(theme.error),
                    ),
                ]));
            }
            HealthStatus::Unknown => {
                lines.push(Line::from(vec![
                    Span::styled("Health:      ", theme.style_text_dim()),
                    Span::styled("? ", Style::default().fg(theme.text_dim)),
                    Span::styled("Unknown", Style::default().fg(theme.text_dim)),
                ]));
            }
        }

        // Show restart count if > 0
        if app.restart_count > 0 {
            lines.push(Line::from(vec![
                Span::styled("Restarts:    ", theme.style_text_dim()),
                Span::styled(
                    format!("🔄 {}", app.restart_count),
                    theme.style_text_secondary(),
                ),
            ]));
        }

        // Show last exit code if app is stopped and has one
        if !app.status.is_running() {
            if let Some(exit_code) = app.last_exit_code {
                let exit_color = if exit_code == 0 {
                    theme.text
                } else {
                    theme.error
                };
                lines.push(Line::from(vec![
                    Span::styled("Exit Code:   ", theme.style_text_dim()),
                    Span::styled(exit_code.to_string(), Style::default().fg(exit_color)),
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
                    Span::styled("  • ", theme.style_text_dim()),
                    Span::styled(dep.clone(), theme.style_text()),
                ]));
            }
        }

        lines
    }
}
