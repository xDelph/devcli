// Rendering logic for MainView
// Handles all UI rendering for different tabs and panels

use super::{ConfigField, ConfigMode, MainTab, MainView, PanelFocus};
use crate::config::loader::load_config;
use crate::tui::log_manager::LogManager;
use crate::tui::state::{AppState, AppStateData};
use crate::tui::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

impl MainView {
    /// Renders the main view with all its components
    pub(super) fn render_main(&self, frame: &mut Frame, state: &AppState, theme: &Theme) {
        let size = frame.area();

        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Min(0),    // Content area
                Constraint::Length(3), // Footer with shortcuts
            ])
            .split(size);

        self.render_tab_bar(frame, main_chunks[0], theme);

        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Left panel - app list
                Constraint::Percentage(70), // Right panel - details
            ])
            .split(main_chunks[1]);

        self.render_app_list(frame, content_chunks[0], state, theme);

        match self.active_tab {
            MainTab::Status => self.render_status_panel(frame, content_chunks[1], state, theme),
            MainTab::Commands => self.render_commands_panel(frame, content_chunks[1], state, theme),
            MainTab::Logs => self.render_logs_panel(frame, content_chunks[1], state, theme),
            MainTab::Config => self.render_config_panel(frame, content_chunks[1], state, theme),
        }

        self.render_footer(frame, main_chunks[2], theme);
    }

    /// Renders the tab bar at the top of the screen
    fn render_tab_bar(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let tab_titles = vec!["[1] Status", "[2] Commands", "[3] Logs", "[4] Config"];
        
        let selected_idx = match self.active_tab {
            MainTab::Status => 0,
            MainTab::Commands => 1,
            MainTab::Logs => 2,
            MainTab::Config => 3,
        };

        let tabs = Tabs::new(tab_titles)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
            )
            .select(selected_idx)
            .style(Style::default().fg(theme.text))
            .highlight_style(
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            );

        frame.render_widget(tabs, area);
    }

    /// Renders the left panel with the project/app tree
    fn render_app_list(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let mut lines = Vec::new();

        for (proj_idx, project) in state.projects.iter().enumerate() {
            let expansion_icon = if project.expanded { "▼" } else { "▶" };
            let project_line = format!("{} {}", expansion_icon, project.name);
            
            let is_project_selected = proj_idx == state.selected_project_idx;
            
            let style = if is_project_selected {
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
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
            Style::default().fg(theme.primary)
        } else {
            Style::default().fg(theme.border)
        };

        let visible_height = area.height.saturating_sub(2) as usize;
        let selected_line = self.calculate_selected_app_line(state);
        let scroll_offset = self.calculate_scroll_offset(selected_line, visible_height, lines.len());

        let app_list = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Projects & Apps")
                    .border_style(border_style)
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
        let status_icon = if app.status.is_running() { "●" } else { "○" };
        let status_color = if app.status.is_running() {
            theme.running
        } else {
            theme.stopped
        };
        
        let is_selected = proj_idx == selected_proj_idx && app_idx == selected_app_idx;
        
        let text_style = if is_selected {
            if self.focus == PanelFocus::AppList {
                Style::default()
                    .bg(theme.selected_bg)
                    .fg(theme.text)
            } else {
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            }
        } else {
            Style::default().fg(theme.text)
        };
        
        Line::from(vec![
            Span::raw("  "),
            Span::styled(status_icon, Style::default().fg(status_color)),
            Span::styled(format!(" {}", app.name), text_style),
        ])
    }

    /// Renders the Status tab panel showing app details
    fn render_status_panel(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
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

        let status_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Details")
                    .border_style(border_style)
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

        if let Some(path) = &app.path {
            lines.push(Line::from(vec![
                Span::styled("Path:        ", Style::default().fg(theme.text_dim)),
                Span::styled(path.clone(), Style::default().fg(theme.text)),
            ]));
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

    /// Renders the Commands tab panel showing available commands
    fn render_commands_panel(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let content = if let Some(app) = state.selected_app() {
            self.build_commands_content(app, theme)
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
        let scroll_offset = self.calculate_scroll_offset(self.selected_command_idx, visible_height, content.len());

        let commands_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Available Commands")
                    .border_style(border_style)
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
                Style::default().fg(theme.text_dim),
            )));
            return lines;
        }

        let max_name_len = app.commands
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
                            Style::default()
                                .fg(theme.text_dim)
                                .bg(theme.selected_bg),
                        )
                    } else {
                        (
                            "  ",
                            Style::default().fg(theme.text),
                            Style::default().fg(theme.text_dim),
                        )
                    };
                    
                    lines.push(Line::from(vec![
                        Span::styled(prefix, Style::default().fg(theme.primary)),
                        Span::styled(format!(" {:<width$}", cmd.name, width = max_name_len), name_style),
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

    /// Renders the Logs tab panel showing available log files
    fn render_logs_panel(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
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
        let scroll_offset = self.calculate_scroll_offset(self.selected_log_idx, visible_height, content.len());

        let logs_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Log Files")
                    .border_style(border_style)
            )
            .scroll((scroll_offset as u16, 0));

        frame.render_widget(logs_panel, area);
    }
}

impl MainView {
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

        match self.log_manager.list_logs_for_app(&app.name) {
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
                        let is_selected = idx == self.selected_log_idx
                            && self.focus == PanelFocus::DetailPanel;
                        
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
                            (
                                "  ",
                                Style::default().fg(theme.text),
                            )
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

    /// Renders the footer with contextual keyboard shortcuts
    fn render_footer(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let shortcuts = match self.active_tab {
            MainTab::Status => {
                "↑↓/jk: Navigate  ←→: Switch Panel  s: Start/Stop  r: Restart  ?: Help  q: Quit"
            }
            MainTab::Commands => {
                "↑↓/jk: Navigate  ←→: Switch Panel  Tab/1-4: Switch Tab  Enter: Execute  q: Quit"
            }
            MainTab::Logs => {
                "↑↓/jk: Navigate  ←→: Switch Panel  Tab/1-4: Switch Tab  Enter: View Log  q: Quit"
            }
            MainTab::Config => match self.config_mode {
                ConfigMode::View => {
                    "↑↓/jk: Navigate  a: Add Cmd  e: Edit Cmd  E: Edit App  s: Set Default  D: Deps  d: Delete  q: Quit"
                }
                ConfigMode::Add | ConfigMode::Edit => {
                    "Tab/↑↓: Navigate Fields  Type: Edit  Enter: Save  Esc: Cancel"
                }
                ConfigMode::AddCommand | ConfigMode::EditCommand => {
                    "Tab: Switch Field  ←→: Move Cursor  Type: Edit  Enter: Save  Esc: Cancel"
                }
                ConfigMode::EditDependencies => {
                    "↑↓/jk: Navigate  a: Add  d: Delete  Esc: Close"
                }
                ConfigMode::AddDependency => {
                    "↑↓/jk: Navigate  Enter: Add  Esc: Cancel"
                }
            },
        };

        let footer = Paragraph::new(shortcuts)
            .style(Style::default().fg(theme.text_dim))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
            );

        frame.render_widget(footer, area);
    }

    /// Renders the Config tab panel
    fn render_config_panel(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        match self.config_mode {
            ConfigMode::View => self.render_config_view(frame, area, state, theme),
            ConfigMode::Add | ConfigMode::Edit => self.render_config_form(frame, area, theme),
            ConfigMode::AddCommand | ConfigMode::EditCommand => self.render_command_edit_form(frame, area, theme),
            ConfigMode::EditDependencies => {
                self.render_config_view(frame, area, state, theme);
                self.render_dependencies_popup(frame, state, theme);
            }
            ConfigMode::AddDependency => {
                self.render_config_view(frame, area, state, theme);
                self.render_add_dependency_popup(frame, theme);
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
        let scroll_offset = self.calculate_scroll_offset(selected_line, visible_height, content.len());

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
}

impl MainView {
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
                                Span::styled("Project:     ".to_string(), Style::default().fg(theme.text_dim)),
                                Span::styled(app.project.clone(), Style::default().fg(theme.text)),
                            ]));

                            lines.push(Line::from(vec![
                                Span::styled("Type:        ".to_string(), Style::default().fg(theme.text_dim)),
                                Span::styled(full_app.app_type.clone(), Style::default().fg(theme.text)),
                            ]));

                            lines.push(Line::from(vec![
                                Span::styled("Path:        ".to_string(), Style::default().fg(theme.text_dim)),
                                Span::styled(full_app.path.clone(), Style::default().fg(theme.text)),
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
                                ("DOCKER", &full_app.commands.docker, &full_app.defaults.docker),
                                ("ORBSTACK", &full_app.commands.orbstack, &full_app.defaults.orbstack),
                                ("K8S", &full_app.commands.k8s, &full_app.defaults.k8s),
                            ];
                            
                            for (env_name, commands, defaults) in env_configs {
                                self.render_env_commands(&mut lines, &mut global_cmd_idx, env_name, commands, defaults, max_name_len, theme);
                            }

                            lines.push(Line::from(""));

                            if !full_app.dependencies.is_empty() {
                                lines.push(Line::from(Span::styled(
                                    "Dependencies:".to_string(),
                                    Style::default()
                                        .fg(theme.secondary)
                                        .add_modifier(Modifier::BOLD),
                                )));
                                for dep in &full_app.dependencies {
                                    lines.push(Line::from(vec![
                                        Span::styled("  • ".to_string(), Style::default().fg(theme.text_dim)),
                                        Span::styled(format!("{}/{}", dep.project, dep.app), Style::default().fg(theme.text)),
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
                                Span::styled(" - Edit selected command", Style::default().fg(theme.text)),
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
                        Style::default().fg(theme.text).bg(theme.selected_bg).add_modifier(Modifier::BOLD),
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
                    Span::styled(format!("{:<width$}", name, width = max_name_len), name_style),
                ];
                
                if is_this_default {
                    spans.push(Span::styled(" [default]".to_string(), Style::default().fg(theme.success)));
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

    /// Renders the command edit form
    fn render_command_edit_form(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
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
            Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_dim)
        };
        
        let env_value_style = if is_env_focused && is_add_mode {
            Style::default().fg(theme.text).bg(theme.selected_bg)
        } else {
            Style::default().fg(theme.text)
        };
        
        let env_text = if is_add_mode {
            format!("{} (Space to cycle)", self.config_form.edit_command_env.to_uppercase())
        } else {
            self.config_form.edit_command_env.to_uppercase()
        };
        
        lines.push(Line::from(vec![
            Span::styled("  Environment: ", env_label_style),
            Span::styled(env_text, env_value_style),
        ]));

        lines.push(Line::from(""));

        // Command name
        self.render_form_field(&mut lines, "Command Name:", &self.config_form.edit_command_name, 
                              self.config_form.cursor_edit_command_name, ConfigField::EditCommandName, theme);
        
        lines.push(Line::from(""));

        // Command value
        self.render_form_field(&mut lines, "Command Value:", &self.config_form.edit_command_value,
                              self.config_form.cursor_edit_command_value, ConfigField::EditCommandValue, theme);

        let config_panel = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Edit Command")
                .border_style(Style::default().fg(theme.primary)),
        );

        frame.render_widget(config_panel, area);
    }

    fn render_form_field(&self, lines: &mut Vec<Line>, label: &str, value: &str, cursor_pos: usize, field: ConfigField, theme: &Theme) {
        let is_focused = self.config_focused_field == field;
        let label_style = if is_focused {
            Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_dim)
        };
        lines.push(Line::from(Span::styled(format!("  {}", label), label_style)));
        
        let mut spans = vec![Span::styled("  ", Style::default())];
        
        if is_focused {
            if !value.is_empty() {
                let before = value.chars().take(cursor_pos).collect::<String>();
                let after = value.chars().skip(cursor_pos).collect::<String>();
                
                if !before.is_empty() {
                    spans.push(Span::styled(before, Style::default().fg(theme.text).bg(theme.selected_bg)));
                }
                spans.push(Span::styled("█", Style::default().fg(theme.primary)));
                if !after.is_empty() {
                    spans.push(Span::styled(after, Style::default().fg(theme.text).bg(theme.selected_bg)));
                }
            } else {
                spans.push(Span::styled("█", Style::default().fg(theme.primary)));
            }
        } else {
            spans.push(Span::styled(value.to_string(), Style::default().fg(theme.text)));
        }
        lines.push(Line::from(spans));
    }

    /// Builds content for config form
    fn build_config_form_content<'a>(&self, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();
        lines.push(Line::from(""));

        let render_field = |label: &str, value: String, cursor_pos: usize, field: ConfigField, focused: ConfigField, theme: &'a Theme| -> Vec<Line<'a>> {
            let is_focused = field == focused;
            let label_style = if is_focused {
                Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_dim)
            };
            let value_style = if is_focused {
                Style::default().fg(theme.text).bg(theme.selected_bg)
            } else {
                Style::default().fg(theme.text)
            };

            let mut spans = vec![Span::styled("  ", Style::default())];
            
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
                    if value.is_empty() { " ".to_string() } else { value },
                    value_style,
                ));
            }

            vec![
                Line::from(Span::styled(format!("  {}", label), label_style)),
                Line::from(spans),
                Line::from(""),
            ]
        };

        lines.extend(render_field("Project Name:", self.config_form.project_name.clone(), self.config_form.cursor_project_name, ConfigField::ProjectName, self.config_focused_field, theme));
        lines.extend(render_field("App Name:", self.config_form.app_name.clone(), self.config_form.cursor_app_name, ConfigField::AppName, self.config_focused_field, theme));
        lines.extend(render_field("App Type (e.g., nodejs, python):", self.config_form.app_type.clone(), self.config_form.cursor_app_type, ConfigField::AppType, self.config_focused_field, theme));
        lines.extend(render_field("Path:", self.config_form.path.clone(), self.config_form.cursor_path, ConfigField::Path, self.config_focused_field, theme));
        lines.extend(render_field("Local Start Command (optional):", self.config_form.local_start_cmd.clone(), self.config_form.cursor_local_start_cmd, ConfigField::LocalStartCmd, self.config_focused_field, theme));
        lines.extend(render_field("Docker Start Command (optional):", self.config_form.docker_start_cmd.clone(), self.config_form.cursor_docker_start_cmd, ConfigField::DockerStartCmd, self.config_focused_field, theme));

        lines
    }
}

impl MainView {
    /// Renders the dependencies popup
    fn render_dependencies_popup(&self, frame: &mut Frame, state: &AppState, theme: &Theme) {
        let area = Self::centered_rect(60, 50, frame.area());

        frame.render_widget(ratatui::widgets::Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.primary))
            .title("Edit Dependencies")
            .style(Style::default().bg(theme.bg));
        frame.render_widget(block, area);

        let inner = Rect {
            x: area.x + 2,
            y: area.y + 2,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(4),
        };

        let mut lines = Vec::new();

        if let Some(app) = state.selected_app() {
            if app.dependencies.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No dependencies configured.",
                    Style::default().fg(theme.text_dim),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Press 'a' to add a dependency.",
                    Style::default().fg(theme.text_dim),
                )));
            } else {
                for (idx, dep) in app.dependencies.iter().enumerate() {
                    let is_selected = idx == self.selected_dependency_idx;
                    let style = if is_selected {
                        Style::default().bg(theme.selected_bg).fg(theme.text).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    };

                    let prefix = if is_selected { " > " } else { "   " };
                    lines.push(Line::from(vec![
                        Span::styled(prefix.to_string(), Style::default().fg(theme.primary)),
                        Span::styled(dep.clone(), style),
                    ]));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "a: Add  d: Delete  Esc: Close",
                Style::default().fg(theme.text_dim),
            )));
        }

        let content = Paragraph::new(lines);
        frame.render_widget(content, inner);
    }

    /// Renders the add dependency popup
    fn render_add_dependency_popup(&self, frame: &mut Frame, theme: &Theme) {
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
                let projects: Vec<_> = config.projects.iter().collect();
                
                for (proj_idx, (proj_name, project)) in projects.iter().enumerate() {
                    let is_selected_project = proj_idx == self.selected_add_dep_project_idx;
                    let proj_style = if is_selected_project {
                        Style::default().fg(theme.primary).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.text)
                    };

                    lines.push(Line::from(Span::styled(
                        format!("▼ {}", proj_name),
                        proj_style,
                    )));

                    if is_selected_project {
                        let apps: Vec<_> = project.apps.keys().collect();
                        for (app_idx, app_name) in apps.iter().enumerate() {
                            let is_selected_app = app_idx == self.selected_add_dep_app_idx;
                            let style = if is_selected_app {
                                Style::default().bg(theme.selected_bg).fg(theme.text).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(theme.text)
                            };

                            let prefix = if is_selected_app { " > " } else { "   " };
                            lines.push(Line::from(vec![
                                Span::styled(prefix, Style::default().fg(theme.primary)),
                                Span::styled(format!("● {}", app_name), style),
                            ]));
                        }
                    }

                    lines.push(Line::from(""));
                }

                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "↑↓: Navigate  Enter: Add  Esc: Cancel",
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

    /// Helper to create a centered rect
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}
