use super::super::{MainView, PanelFocus};
use crate::tui::state::{AppState, AppStateData, HealthStatus};
use crate::tui::theme::Theme;
use ratatui::{
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

impl MainView {
    /// Renders the left panel with the project/app tree
    pub(crate) fn render_app_list(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        let mut lines = Vec::new();

        for (proj_idx, project) in state.projects.iter().enumerate() {
            let expansion_icon = if project.expanded { "▼" } else { "▶" };
            let project_line = format!("{} {}", expansion_icon, project.name);

            let is_project_selected = proj_idx == state.selected_project_idx;

            let style = if is_project_selected {
                theme.style_text_primary_bold()
            } else {
                theme.style_text()
            };

            lines.push(Line::from(Span::styled(project_line, style)));

            if project.expanded {
                for (app_idx, app) in project.apps.iter().enumerate() {
                    let app_line = self.format_app_line(
                        app,
                        proj_idx,
                        app_idx,
                        state.selected_project_idx,
                        state.selected_app_idx,
                        theme,
                    );
                    lines.push(app_line);
                }
            }

            if proj_idx < state.projects.len() - 1 {
                lines.push(Line::from(""));
            }
        }

        let border_style = if self.focus == PanelFocus::AppList {
            theme.style_border_focused()
        } else {
            theme.style_border_default()
        };

        let visible_height = area.height.saturating_sub(2) as usize;
        let selected_line = self.calculate_selected_app_line(state);
        let scroll_offset =
            self.calculate_scroll_offset(selected_line, visible_height, lines.len());

        let app_list = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Projects & Apps")
                    .border_style(border_style),
            )
            .scroll((scroll_offset as u16, 0));

        frame.render_widget(app_list, area);
    }

    /// Formats a single app line with status indicator and selection highlight
    fn format_app_line<'a>(
        &self,
        app: &'a AppStateData,
        proj_idx: usize,
        app_idx: usize,
        selected_proj_idx: usize,
        selected_app_idx: usize,
        theme: &'a Theme,
    ) -> Line<'a> {
        let status_icon = if app.status.is_running() {
            "●"
        } else {
            "○"
        };

        let is_selected = proj_idx == selected_proj_idx && app_idx == selected_app_idx;

        let text_style = if is_selected {
            if self.focus == PanelFocus::AppList {
                theme.style_bg_selected_text()
            } else {
                theme.style_text_primary_bold()
            }
        } else {
            theme.style_text()
        };

        // Build stage indicator
        // Show active_stage if running (may be overridden), otherwise show configured stage
        let stage_indicator = if app.status.is_running() {
            // For running apps, show the active stage (the one actually used)
            if let Some(ref stage) = app.active_stage {
                format!(" [{}]", stage.to_uppercase())
            } else {
                String::new()
            }
        } else {
            // For stopped apps, show the configured stage
            if let Some(ref stage) = app.stage {
                format!(" [{}]", stage.to_uppercase())
            } else {
                String::new()
            }
        };

        // Style for stage indicator - use a distinct color
        let stage_style = if is_selected {
            if self.focus == PanelFocus::AppList {
                theme.style_bg_selected_secondary()
            } else {
                theme.style_text_secondary_bold()
            }
        } else {
            theme.style_text_secondary()
        };

        // Build restart count indicator
        let restart_indicator = if app.restart_count > 0 {
            format!(" 🔄{}", app.restart_count)
        } else {
            String::new()
        };

        // Build health status indicator
        let health_indicator = match &app.health_status {
            HealthStatus::Healthy => " ✓",
            HealthStatus::Unhealthy { .. } => " ✗",
            HealthStatus::Unknown => "",
        };

        // Build exit code indicator (only show for stopped apps with exit code)
        let exit_code_indicator = if !app.status.is_running() {
            if let Some(exit_code) = app.last_exit_code {
                if exit_code != 0 {
                    format!(" [exit:{}]", exit_code)
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let mut spans = vec![
            Span::raw("  "),
            Span::styled(
                status_icon,
                if app.status.is_running() {
                    theme.style_text_running()
                } else {
                    theme.style_text_stopped()
                },
            ),
            Span::styled(format!(" {}", app.name), text_style),
            Span::styled(stage_indicator, stage_style),
        ];

        // Add restart indicator if present
        if !restart_indicator.is_empty() {
            spans.push(Span::styled(restart_indicator, stage_style));
        }

        // Add health indicator if present
        if !health_indicator.is_empty() {
            let health_style = if is_selected {
                if self.focus == PanelFocus::AppList {
                    theme.style_bg_selected_secondary()
                } else {
                    match &app.health_status {
                        HealthStatus::Healthy => theme.style_text_running(),
                        HealthStatus::Unhealthy { .. } => theme.style_text_error(),
                        HealthStatus::Unknown => theme.style_text_secondary(),
                    }
                }
            } else {
                match &app.health_status {
                    HealthStatus::Healthy => theme.style_text_running(),
                    HealthStatus::Unhealthy { .. } => theme.style_text_error(),
                    HealthStatus::Unknown => theme.style_text_secondary(),
                }
            };
            spans.push(Span::styled(health_indicator, health_style));
        }

        // Add exit code indicator if present
        if !exit_code_indicator.is_empty() {
            spans.push(Span::styled(exit_code_indicator, theme.style_text_error()));
        }

        Line::from(spans)
    }
}
