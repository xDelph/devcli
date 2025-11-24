use super::super::{ConfigField, ConfigMode, DeleteType, MainView, PanelFocus};
use crate::config::loader::load_config;
use crate::tui::state::AppState;
use crate::tui::theme::Theme;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

impl MainView {
    /// Renders the Config tab panel
    pub(crate) fn render_config_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        theme: &Theme,
    ) {
        match self.config_mode {
            ConfigMode::View => self.render_config_view(frame, area, state, theme),
            ConfigMode::Add | ConfigMode::Edit => self.render_config_form(frame, area, theme),
            ConfigMode::AddCommand | ConfigMode::EditCommand => {
                self.render_command_edit_form(frame, area, theme)
            }
            ConfigMode::EditEnvFiles => self.render_env_files_view(frame, area, state, theme),
            ConfigMode::AddEnvFile | ConfigMode::EditEnvFile => {
                self.render_add_env_file_form(frame, area, theme)
            }
            ConfigMode::EditDependencies => {
                self.render_config_view(frame, area, state, theme);
                self.render_dependencies_popup(frame, state, theme);
            }
            ConfigMode::AddDependency => {
                self.render_config_view(frame, area, state, theme);
                let (proj, app) = if let Some(app) = state.selected_app() {
                    (Some(app.project.as_str()), Some(app.name.as_str()))
                } else {
                    (None, None)
                };
                self.render_add_dependency_popup(frame, theme, proj, app);
            }
            ConfigMode::ConfirmDelete => {
                // Render the underlying view based on delete type
                match self.delete_confirm_type {
                    DeleteType::App | DeleteType::Command => {
                        self.render_config_view(frame, area, state, theme)
                    }
                    DeleteType::Dependency => {
                        self.render_config_view(frame, area, state, theme);
                        self.render_dependencies_popup(frame, state, theme);
                    }
                    DeleteType::EnvFile => self.render_env_files_view(frame, area, state, theme),
                }
                // Render confirmation popup on top
                self.render_delete_confirmation_popup(frame, theme);
            }
        }
    }

    /// Renders the config view mode (shows selected app's config)
    fn render_config_view(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let content = self.build_config_view_content(state, theme);

        let border_style = if self.focus == PanelFocus::DetailPanel {
            Style::default().fg(theme.primary)
        } else {
            Style::default().fg(theme.border)
        };

        let selected_line = self.calculate_selected_config_command_line(state);
        let visible_height = area.height.saturating_sub(2) as usize;
        let scroll_offset =
            self.calculate_scroll_offset(selected_line, visible_height, content.len());

        let config_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Configuration")
                    .border_style(border_style),
            )
            .scroll((scroll_offset as u16, 0));

        frame.render_widget(config_panel, area);
    }

    /// Builds content for config view mode - shows selected app's config
    fn build_config_view_content(&self, state: &AppState, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if let Some(app) = state.selected_app() {
            lines.push(Line::from(vec![
                Span::styled("─ ", Style::default().fg(theme.border)),
                Span::styled("● ", Style::default().fg(theme.primary)),
                Span::styled(
                    format!("{} Configuration", app.name),
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ─", Style::default().fg(theme.border)),
            ]));
            lines.push(Line::from(""));

            match load_config() {
                Ok(config) => {
                    if let Some(project) = config.projects.get(&app.project) {
                        if let Some(full_app) = project.apps.get(&app.name) {
                            lines.push(Line::from(vec![
                                Span::styled(
                                    "Project:     ".to_string(),
                                    Style::default().fg(theme.text_dim),
                                ),
                                Span::styled(app.project.clone(), Style::default().fg(theme.text)),
                            ]));

                            lines.push(Line::from(vec![
                                Span::styled(
                                    "Type:        ".to_string(),
                                    Style::default().fg(theme.text_dim),
                                ),
                                Span::styled(
                                    full_app.app_type.clone(),
                                    Style::default().fg(theme.text),
                                ),
                            ]));

                            lines.push(Line::from(vec![
                                Span::styled(
                                    "Path:        ".to_string(),
                                    Style::default().fg(theme.text_dim),
                                ),
                                Span::styled(
                                    full_app.path.clone(),
                                    Style::default().fg(theme.text),
                                ),
                            ]));

                            lines.push(Line::from(""));

                            lines.push(Line::from(Span::styled(
                                "Commands:".to_string(),
                                Style::default()
                                    .fg(theme.secondary)
                                    .add_modifier(Modifier::BOLD),
                            )));

                            let max_name_len = self.calculate_max_command_name_len(full_app);
                            let mut global_cmd_idx = 0;

                            // Render commands for each environment dynamically
                            let env_configs = [
                                ("LOCAL", &full_app.commands.local, &full_app.defaults.local),
                                (
                                    "DOCKER",
                                    &full_app.commands.docker,
                                    &full_app.defaults.docker,
                                ),
                                (
                                    "ORBSTACK",
                                    &full_app.commands.orbstack,
                                    &full_app.defaults.orbstack,
                                ),
                                ("K8S", &full_app.commands.k8s, &full_app.defaults.k8s),
                            ];

                            for (env_name, commands, defaults) in env_configs {
                                self.render_env_commands(
                                    &mut lines,
                                    &mut global_cmd_idx,
                                    env_name,
                                    commands,
                                    defaults,
                                    max_name_len,
                                    theme,
                                );
                            }

                            lines.push(Line::from(""));

                            // Display env_files configuration
                            if let Some(env_files) = &full_app.env_files {
                                lines.push(Line::from(Span::styled(
                                    "Environment Files:".to_string(),
                                    Style::default()
                                        .fg(theme.secondary)
                                        .add_modifier(Modifier::BOLD),
                                )));

                                // Sort stages alphabetically for consistent display
                                let mut sorted_stages: Vec<_> = env_files.iter().collect();
                                sorted_stages.sort_by_key(|(stage, _)| stage.as_str());

                                for (stage, contexts) in sorted_stages {
                                    lines.push(Line::from(vec![
                                        Span::styled("  ".to_string(), Style::default()),
                                        Span::styled(
                                            format!("{}: ", stage),
                                            Style::default().fg(theme.primary),
                                        ),
                                    ]));

                                    // Sort contexts alphabetically for consistent display
                                    let mut sorted_contexts: Vec<_> = contexts.iter().collect();
                                    sorted_contexts.sort_by_key(|(context, _)| context.as_str());

                                    for (context, file_path) in sorted_contexts {
                                        lines.push(Line::from(vec![
                                            Span::styled(
                                                "    • ".to_string(),
                                                Style::default().fg(theme.text_dim),
                                            ),
                                            Span::styled(
                                                format!("{}: ", context),
                                                Style::default().fg(theme.secondary),
                                            ),
                                            Span::styled(
                                                file_path.clone(),
                                                Style::default().fg(theme.text),
                                            ),
                                        ]));
                                    }
                                }
                                lines.push(Line::from(""));
                            }

                            if !full_app.dependencies.is_empty() {
                                lines.push(Line::from(Span::styled(
                                    "Dependencies:".to_string(),
                                    Style::default()
                                        .fg(theme.secondary)
                                        .add_modifier(Modifier::BOLD),
                                )));
                                for dep in &full_app.dependencies {
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            "  • ".to_string(),
                                            Style::default().fg(theme.text_dim),
                                        ),
                                        Span::styled(
                                            format!("{}/{}", dep.project, dep.app),
                                            Style::default().fg(theme.text),
                                        ),
                                    ]));
                                }
                                lines.push(Line::from(""));
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
                                Span::styled(" - Add new command", Style::default().fg(theme.text)),
                            ]));
                            lines.push(Line::from(vec![
                                Span::styled("  e", Style::default().fg(theme.primary)),
                                Span::styled(
                                    " - Edit selected command",
                                    Style::default().fg(theme.text),
                                ),
                            ]));
                            lines.push(Line::from(vec![
                                Span::styled("  f", Style::default().fg(theme.primary)),
                                Span::styled(
                                    " - Edit environment files",
                                    Style::default().fg(theme.text),
                                ),
                            ]));
                            lines.push(Line::from(vec![
                                Span::styled("  E", Style::default().fg(theme.primary)),
                                Span::styled(" - Edit this app", Style::default().fg(theme.text)),
                            ]));
                            lines.push(Line::from(vec![
                                Span::styled("  d", Style::default().fg(theme.primary)),
                                Span::styled(" - Delete this app", Style::default().fg(theme.text)),
                            ]));
                        }
                    }
                }
                Err(e) => {
                    lines.push(Line::from(Span::styled(
                        format!("Error loading config: {}", e),
                        Style::default().fg(theme.error),
                    )));
                }
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled("─ ", Style::default().fg(theme.border)),
                Span::styled(
                    "Configuration",
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ─", Style::default().fg(theme.border)),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Select an app from the left panel to view its configuration.",
                Style::default().fg(theme.text_dim),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Or press 'a' to add a new app.",
                Style::default().fg(theme.text_dim),
            )));
        }

        lines
    }

    fn calculate_max_command_name_len(&self, app: &crate::config::models::App) -> usize {
        let mut max_len = 15;
        if let Some(local) = &app.commands.local {
            max_len = max_len.max(local.keys().map(|k| k.len()).max().unwrap_or(0));
        }
        if let Some(docker) = &app.commands.docker {
            max_len = max_len.max(docker.keys().map(|k| k.len()).max().unwrap_or(0));
        }
        if let Some(orbstack) = &app.commands.orbstack {
            max_len = max_len.max(orbstack.keys().map(|k| k.len()).max().unwrap_or(0));
        }
        if let Some(k8s) = &app.commands.k8s {
            max_len = max_len.max(k8s.keys().map(|k| k.len()).max().unwrap_or(0));
        }
        max_len
    }

    #[allow(clippy::too_many_arguments)]
    fn render_env_commands(
        &self,
        lines: &mut Vec<Line<'static>>,
        global_cmd_idx: &mut usize,
        env_name: &str,
        commands: &Option<std::collections::HashMap<String, String>>,
        default: &Option<String>,
        max_name_len: usize,
        theme: &Theme,
    ) {
        if let Some(cmds) = commands {
            lines.push(Line::from(Span::styled(
                format!("  {}:", env_name),
                Style::default().fg(theme.text_dim),
            )));

            let mut sorted_cmds: Vec<_> = cmds.iter().collect();
            sorted_cmds.sort_by_key(|(name, _)| *name);

            for (name, cmd) in sorted_cmds {
                let is_selected = *global_cmd_idx == self.selected_config_command_idx
                    && self.focus == PanelFocus::DetailPanel;
                let is_this_default = default.as_ref() == Some(name);

                let (prefix, name_style, cmd_style) = if is_selected {
                    (
                        " >",
                        Style::default()
                            .fg(theme.text)
                            .bg(theme.selected_bg)
                            .add_modifier(Modifier::BOLD),
                        Style::default().fg(theme.text_dim).bg(theme.selected_bg),
                    )
                } else {
                    (
                        "  ",
                        Style::default().fg(theme.text),
                        Style::default().fg(theme.text_dim),
                    )
                };

                let mut spans = vec![
                    Span::styled(prefix.to_string(), Style::default().fg(theme.primary)),
                    Span::styled(" ".to_string(), Style::default()),
                    Span::styled(
                        format!("{:<width$}", name, width = max_name_len),
                        name_style,
                    ),
                ];

                if is_this_default {
                    spans.push(Span::styled(
                        " [default]".to_string(),
                        Style::default().fg(theme.success),
                    ));
                }

                spans.push(Span::styled("  ".to_string(), Style::default()));
                spans.push(Span::styled(cmd.clone(), cmd_style));

                lines.push(Line::from(spans));
                *global_cmd_idx += 1;
            }
            lines.push(Line::from(""));
        }
    }

    /// Renders the config form (add/edit mode)
    fn render_config_form(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let content = self.build_config_form_content(theme);

        let title = match self.config_mode {
            ConfigMode::Add => "Add New App",
            ConfigMode::Edit => "Edit App",
            _ => "Config",
        };

        let config_panel = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(theme.primary)),
        );

        frame.render_widget(config_panel, area);
    }

    pub(crate) fn render_form_field(
        &self,
        lines: &mut Vec<Line>,
        label: &str,
        editor: &crate::tui::widgets::TextEditor,
        field: ConfigField,
        theme: &Theme,
    ) {
        let is_focused = self.config_focused_field == field;
        let label_style = if is_focused {
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_dim)
        };
        lines.push(Line::from(Span::styled(
            format!("  {}", label),
            label_style,
        )));

        let mut spans = vec![Span::styled("  ", Style::default())];
        let value = editor.content();
        let cursor_pos = editor.cursor_position();

        if is_focused {
            if !value.is_empty() {
                let before = value.chars().take(cursor_pos).collect::<String>();
                let after = value.chars().skip(cursor_pos).collect::<String>();

                if !before.is_empty() {
                    spans.push(Span::styled(
                        before,
                        Style::default().fg(theme.text).bg(theme.selected_bg),
                    ));
                }
                spans.push(Span::styled("█", Style::default().fg(theme.primary)));
                if !after.is_empty() {
                    spans.push(Span::styled(
                        after,
                        Style::default().fg(theme.text).bg(theme.selected_bg),
                    ));
                }
            } else {
                spans.push(Span::styled("█", Style::default().fg(theme.primary)));
            }
        } else {
            spans.push(Span::styled(
                value.to_string(),
                Style::default().fg(theme.text),
            ));
        }
        lines.push(Line::from(spans));
    }

    /// Builds content for config form
    fn build_config_form_content<'a>(&self, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();
        lines.push(Line::from(""));

        let render_field = |label: &str,
                            editor: &crate::tui::widgets::TextEditor,
                            field: ConfigField,
                            focused: ConfigField,
                            theme: &'a Theme|
         -> Vec<Line<'a>> {
            let is_focused = field == focused;
            let label_style = if is_focused {
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_dim)
            };
            let value_style = if is_focused {
                Style::default().fg(theme.text).bg(theme.selected_bg)
            } else {
                Style::default().fg(theme.text)
            };

            let mut spans = vec![Span::styled("  ", Style::default())];
            let value = editor.content();
            let cursor_pos = editor.cursor_position();

            if is_focused && !value.is_empty() {
                let before = value.chars().take(cursor_pos).collect::<String>();
                let after = value.chars().skip(cursor_pos).collect::<String>();

                if !before.is_empty() {
                    spans.push(Span::styled(before, value_style));
                }
                spans.push(Span::styled("█", Style::default().fg(theme.primary)));
                if !after.is_empty() {
                    spans.push(Span::styled(after, value_style));
                }
            } else if is_focused && value.is_empty() {
                spans.push(Span::styled("█", Style::default().fg(theme.primary)));
            } else {
                spans.push(Span::styled(
                    if value.is_empty() {
                        " ".to_string()
                    } else {
                        value.to_string()
                    },
                    value_style,
                ));
            }

            vec![
                Line::from(Span::styled(format!("  {}", label), label_style)),
                Line::from(spans),
                Line::from(""),
            ]
        };

        lines.extend(render_field(
            "Project Name:",
            &self.config_form.project_name,
            ConfigField::ProjectName,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "App Name:",
            &self.config_form.app_name,
            ConfigField::AppName,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "App Type (e.g., nodejs, python):",
            &self.config_form.app_type,
            ConfigField::AppType,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "Path:",
            &self.config_form.path,
            ConfigField::Path,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "Local Start Command (optional):",
            &self.config_form.local_start_cmd,
            ConfigField::LocalStartCmd,
            self.config_focused_field,
            theme,
        ));
        lines.extend(render_field(
            "Docker Start Command (optional):",
            &self.config_form.docker_start_cmd,
            ConfigField::DockerStartCmd,
            self.config_focused_field,
            theme,
        ));

        lines
    }
}
