// Main view implementation with split-panel layout
// Inspired by GitUI's clean tab-based interface
// Features: Status, Commands, and Logs tabs with project/app tree on left and details on right

use crate::tui::log_manager::LogManager;
use crate::tui::state::{AppState, AppStateData};
use crate::tui::theme::Theme;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};
use std::sync::{Arc, Mutex};

/// Main view with tab-based navigation and split-panel layout
/// Left panel (30%): Project/app tree with status indicators
/// Right panel (70%): Details based on selected tab
pub struct MainView {
    /// Currently active tab
    pub(crate) active_tab: MainTab,
    /// Which panel has focus (left app list or right details)
    pub(crate) focus: PanelFocus,
    /// Scroll offset for the left panel list
    /// Reserved for future use when implementing scrolling in the app list
    #[allow(dead_code)]
    pub(crate) list_scroll: usize,
    /// Scroll offset for the right panel details
    pub(crate) detail_scroll: usize,
    /// Selected command index in the Commands tab
    /// Tracks which command is highlighted in the detail panel
    pub(crate) selected_command_idx: usize,
    /// Selected log file index in the Logs tab
    /// Tracks which log file is highlighted in the detail panel
    pub(crate) selected_log_idx: usize,
    /// Log manager for discovering log files
    pub(crate) log_manager: LogManager,
}

/// The three main tabs in the interface
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainTab {
    /// Status tab - shows app details and running status
    Status,
    /// Commands tab - shows available commands for selected app
    Commands,
    /// Logs tab - shows log files for selected app
    Logs,
}

/// Indicates which panel currently has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    /// Left panel with project/app tree
    AppList,
    /// Right panel with details/commands/logs
    DetailPanel,
}

impl MainView {
    /// Creates a new MainView with default settings
    pub fn new() -> Self {
        Self {
            active_tab: MainTab::Status,
            focus: PanelFocus::AppList,
            list_scroll: 0,
            detail_scroll: 0,
            selected_command_idx: 0,
            selected_log_idx: 0,
            log_manager: LogManager::default(),
        }
    }

    /// Handles keyboard input for the main view
    /// Returns true if the event was handled, false otherwise
    pub fn handle_input(&mut self, key: KeyEvent, state: &Arc<Mutex<AppState>>) -> Result<bool> {
        // Lock the state for modification
        // Use expect instead of context since PoisonError doesn't implement StdError
        let mut state = state.lock().expect("Failed to lock state");
        match key.code {
            // Tab switching with number keys (1-3)
            // Also support keyboard layout variants (e.g., French AZERTY: &=1, é=2, "=3)
            KeyCode::Char('1') | KeyCode::Char('&') => {
                self.active_tab = MainTab::Status;
                self.detail_scroll = 0; // Reset scroll when switching tabs
                self.selected_command_idx = 0; // Reset command selection
                Ok(true)
            }
            KeyCode::Char('2') | KeyCode::Char('é') => {
                self.active_tab = MainTab::Commands;
                self.detail_scroll = 0;
                self.selected_command_idx = 0; // Reset command selection
                Ok(true)
            }
            KeyCode::Char('3') | KeyCode::Char('"') => {
                self.active_tab = MainTab::Logs;
                self.detail_scroll = 0;
                self.selected_command_idx = 0; // Reset command selection
                self.selected_log_idx = 0; // Reset log selection
                Ok(true)
            }
            // Tab key to cycle through tabs
            KeyCode::Tab => {
                // Cycle to next tab
                self.active_tab = match self.active_tab {
                    MainTab::Status => MainTab::Commands,
                    MainTab::Commands => MainTab::Logs,
                    MainTab::Logs => MainTab::Status,
                };
                self.detail_scroll = 0;
                self.selected_command_idx = 0;
                if self.active_tab == MainTab::Logs {
                    self.selected_log_idx = 0;
                }
                Ok(true)
            }
            // Left/Right arrows to switch panel focus
            KeyCode::Left | KeyCode::Right => {
                self.focus = match self.focus {
                    PanelFocus::AppList => PanelFocus::DetailPanel,
                    PanelFocus::DetailPanel => PanelFocus::AppList,
                };
                Ok(true)
            }
            // Navigation keys - behavior depends on which panel has focus
            KeyCode::Up | KeyCode::Char('k') => {
                match self.focus {
                    PanelFocus::AppList => {
                        state.select_previous();
                        // Reset command and log selection when changing apps
                        self.selected_command_idx = 0;
                        self.selected_log_idx = 0;
                    }
                    PanelFocus::DetailPanel => {
                        // In Commands tab, navigate through commands
                        if self.active_tab == MainTab::Commands {
                            self.selected_command_idx = self.selected_command_idx.saturating_sub(1);
                        } else if self.active_tab == MainTab::Logs {
                            // In Logs tab, navigate through log files
                            self.selected_log_idx = self.selected_log_idx.saturating_sub(1);
                        } else {
                            // Scroll up in detail panel for other tabs
                            self.detail_scroll = self.detail_scroll.saturating_sub(1);
                        }
                    }
                }
                Ok(true)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                match self.focus {
                    PanelFocus::AppList => {
                        state.select_next();
                        // Reset command and log selection when changing apps
                        self.selected_command_idx = 0;
                        self.selected_log_idx = 0;
                    }
                    PanelFocus::DetailPanel => {
                        // In Commands tab, navigate through commands
                        if self.active_tab == MainTab::Commands {
                            // Get total command count to limit navigation
                            if let Some(app) = state.selected_app() {
                                let total_commands = Self::count_total_commands(app);
                                if self.selected_command_idx < total_commands.saturating_sub(1) {
                                    self.selected_command_idx += 1;
                                }
                            }
                        } else if self.active_tab == MainTab::Logs {
                            // In Logs tab, navigate through log files
                            if let Some(app) = state.selected_app() {
                                // Get log files to determine max index
                                if let Ok(log_files) = self.log_manager.list_logs_for_app(&app.name) {
                                    if self.selected_log_idx < log_files.len().saturating_sub(1) {
                                        self.selected_log_idx += 1;
                                    }
                                }
                            }
                        } else {
                            // Scroll down in detail panel for other tabs
                            self.detail_scroll = self.detail_scroll.saturating_add(1);
                        }
                    }
                }
                Ok(true)
            }
            // Space to expand/collapse projects (only in app list)
            KeyCode::Char(' ') if self.focus == PanelFocus::AppList => {
                state.toggle_project_expansion();
                Ok(true)
            }
            // Enter key - trigger actions based on context
            KeyCode::Enter => {
                if self.focus == PanelFocus::DetailPanel {
                    match self.active_tab {
                        MainTab::Commands => {
                            // Signal that we want to execute a command
                            // The actual execution will be handled by the app
                            state.set_command_execution_requested(self.selected_command_idx);
                        }
                        MainTab::Logs => {
                            // Open the selected log file in the log viewer
                            if let Some(app) = state.selected_app() {
                                if let Ok(log_files) = self.log_manager.list_logs_for_app(&app.name) {
                                    if let Some(log_file) = log_files.get(self.selected_log_idx) {
                                        // Transition to log viewer view
                                        state.current_view = crate::tui::state::ViewType::LogViewer {
                                            log_path: log_file.path.clone(),
                                        };
                                    }
                                }
                            }
                        }
                        MainTab::Status => {
                            // TODO: Implement start/stop app action
                        }
                    }
                }
                Ok(true)
            }
            _ => Ok(false), // Event not handled
        }
    }

    /// Counts the total number of commands for an app across all environments
    /// Used to limit command navigation in the Commands tab
    fn count_total_commands(app: &AppStateData) -> usize {
        app.commands.values().map(|cmds| cmds.len()).sum()
    }

    /// Gets the currently selected command info based on the selected index
    /// Returns (environment, command_info) tuple if a valid command is selected
    /// 
    /// Commands are indexed sequentially across environments in order: local, docker, k8s
    /// For example, if local has 3 commands and docker has 2:
    /// - Index 0-2: local commands
    /// - Index 3-4: docker commands
    pub fn get_selected_command<'a>(
        &self,
        app: &'a AppStateData,
    ) -> Option<(&'a str, &'a crate::tui::state::CommandInfo)> {
        let mut current_idx = 0;
        
        // Iterate through environments in order: local, docker, k8s
        for env in &["local", "docker", "k8s"] {
            if let Some(commands) = app.commands.get(*env) {
                // Check if the selected index falls within this environment's commands
                if self.selected_command_idx < current_idx + commands.len() {
                    let cmd_idx = self.selected_command_idx - current_idx;
                    return Some((env, &commands[cmd_idx]));
                }
                current_idx += commands.len();
            }
        }
        
        None
    }

    /// Renders the main view with all its components
    pub fn render(&self, frame: &mut Frame, state: &AppState, theme: &Theme) {
        let size = frame.size();

        // Main layout: tab bar, content area, footer
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Min(0),    // Content area
                Constraint::Length(3), // Footer with shortcuts
            ])
            .split(size);

        // Render tab bar at the top
        self.render_tab_bar(frame, main_chunks[0], theme);

        // Split content area into left (app list) and right (details) panels
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30), // Left panel - app list
                Constraint::Percentage(70), // Right panel - details
            ])
            .split(main_chunks[1]);

        // Render left panel (project/app tree)
        self.render_app_list(frame, content_chunks[0], state, theme);

        // Render right panel based on active tab
        match self.active_tab {
            MainTab::Status => self.render_status_panel(frame, content_chunks[1], state, theme),
            MainTab::Commands => self.render_commands_panel(frame, content_chunks[1], state, theme),
            MainTab::Logs => self.render_logs_panel(frame, content_chunks[1], state, theme),
        }

        // Render footer with contextual keyboard shortcuts
        self.render_footer(frame, main_chunks[2], theme);
    }

    /// Renders the tab bar at the top of the screen
    /// Shows Status, Commands, and Logs tabs with the active one highlighted
    /// Visual polish: Clear tab indicators with consistent spacing
    fn render_tab_bar(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        // Tab titles with keyboard shortcuts for quick access
        // Visual consistency: Uniform formatting across all tabs
        let tab_titles = vec!["[1] Status", "[2] Commands", "[3] Logs"];
        
        // Determine which tab index is active (0, 1, or 2)
        let selected_idx = match self.active_tab {
            MainTab::Status => 0,
            MainTab::Commands => 1,
            MainTab::Logs => 2,
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
    /// Shows projects with expand/collapse and apps with status indicators
    /// Visual polish: Clear hierarchy with consistent indentation and spacing
    fn render_app_list(&self, frame: &mut Frame, area: Rect, state: &AppState, theme: &Theme) {
        let mut lines = Vec::new();

        // Build the tree structure with visual hierarchy
        // Optimization: Pre-allocate capacity for better performance
        for (proj_idx, project) in state.projects.iter().enumerate() {
            // Project header with expansion indicator
            // Visual consistency: Unicode arrows for expand/collapse state
            let expansion_icon = if project.expanded { "▼" } else { "▶" };
            let project_line = format!("{} {}", expansion_icon, project.name);
            
            // Highlight if this project is selected and we're in the app list panel
            let is_project_selected = proj_idx == state.selected_project_idx 
                && self.focus == PanelFocus::AppList;
            
            let style = if is_project_selected {
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            };
            
            lines.push(Line::from(Span::styled(project_line, style)));

            // Show apps if project is expanded
            // Visual polish: Indented apps show clear parent-child relationship
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
        }

        // Create the widget with appropriate border style based on focus
        // Visual feedback: Highlighted border shows which panel is active
        let border_style = if self.focus == PanelFocus::AppList {
            Style::default().fg(theme.primary) // Highlight border when focused
        } else {
            Style::default().fg(theme.border)
        };

        let app_list = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Projects & Apps")
                    .border_style(border_style)
            );

        frame.render_widget(app_list, area);
    }

    /// Formats a single app line with status indicator and selection highlight
    /// Returns a Line with appropriate styling
    /// Visual polish: Clear status indicators with consistent spacing
    fn format_app_line<'a>(
        &self,
        app: &'a AppStateData,
        proj_idx: usize,
        app_idx: usize,
        selected_proj_idx: usize,
        selected_app_idx: usize,
        theme: &'a Theme,
    ) -> Line<'a> {
        // Status indicator: ● for running, ○ for stopped
        // Visual consistency: Unicode circles provide clear at-a-glance status
        let status_icon = if app.status.is_running() { "●" } else { "○" };
        let status_color = if app.status.is_running() {
            theme.running
        } else {
            theme.stopped
        };
        
        // Check if this app is currently selected
        let is_selected = proj_idx == selected_proj_idx 
            && app_idx == selected_app_idx
            && self.focus == PanelFocus::AppList;
        
        // Apply selection styling
        // Visual feedback: Background highlight shows current selection
        let text_style = if is_selected {
            Style::default()
                .bg(theme.selected_bg)
                .fg(theme.text)
        } else {
            Style::default().fg(theme.text)
        };
        
        // Build the line with indentation, status icon, and app name
        // Visual polish: Consistent 2-space indentation for hierarchy
        Line::from(vec![
            Span::raw("  "), // Indentation for apps under projects
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
    /// Shows app name, status, PID, uptime, type, path, and dependencies
    fn build_status_content<'a>(&self, app: &'a AppStateData, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // App name header
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
        lines.push(Line::from("")); // Empty line for spacing

        // Status information
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

        // Additional details for running apps
        if let crate::tui::state::AppStatus::Running { pid, uptime, .. } = &app.status {
            lines.push(Line::from(vec![
                Span::styled("PID:         ", Style::default().fg(theme.text_dim)),
                Span::styled(pid.to_string(), Style::default().fg(theme.text)),
            ]));

            // Format uptime in a human-readable way
            let uptime_str = Self::format_duration(uptime);
            lines.push(Line::from(vec![
                Span::styled("Uptime:      ", Style::default().fg(theme.text_dim)),
                Span::styled(uptime_str, Style::default().fg(theme.text)),
            ]));
        }

        // App type
        lines.push(Line::from(vec![
            Span::styled("Type:        ", Style::default().fg(theme.text_dim)),
            Span::styled(app.app_type.clone(), Style::default().fg(theme.text)),
        ]));

        // Project
        lines.push(Line::from(vec![
            Span::styled("Project:     ", Style::default().fg(theme.text_dim)),
            Span::styled(app.project.clone(), Style::default().fg(theme.text)),
        ]));

        // Path (if available from app data)
        if let Some(path) = &app.path {
            lines.push(Line::from(vec![
                Span::styled("Path:        ", Style::default().fg(theme.text_dim)),
                Span::styled(path.clone(), Style::default().fg(theme.text)),
            ]));
        }

        // Dependencies section
        if !app.dependencies.is_empty() {
            lines.push(Line::from("")); // Empty line for spacing
            lines.push(Line::from(Span::styled(
                "Dependencies:",
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )));

            for dep in &app.dependencies {
                // Show dependency with a checkmark or x based on status
                // For now, we'll just show the dependency name
                // TODO: Check actual dependency status
                lines.push(Line::from(vec![
                    Span::styled("  • ", Style::default().fg(theme.text_dim)),
                    Span::styled(dep.clone(), Style::default().fg(theme.text)),
                ]));
            }
        }

        lines
    }

    /// Formats a duration into a human-readable string (e.g., "2h 15m 32s")
    pub(crate) fn format_duration(duration: &chrono::Duration) -> String {
        let total_seconds = duration.num_seconds();
        
        if total_seconds < 60 {
            format!("{}s", total_seconds)
        } else if total_seconds < 3600 {
            let minutes = total_seconds / 60;
            let seconds = total_seconds % 60;
            format!("{}m {}s", minutes, seconds)
        } else {
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;
            format!("{}h {}m {}s", hours, minutes, seconds)
        }
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

        let commands_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Available Commands")
                    .border_style(border_style)
            );

        frame.render_widget(commands_panel, area);
    }

    /// Builds the content for the commands panel
    /// Groups commands by environment (local, docker, k8s)
    /// Highlights the currently selected command when detail panel has focus
    fn build_commands_content<'a>(&self, app: &'a AppStateData, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // App name header
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
        lines.push(Line::from("")); // Empty line for spacing

        if app.commands.is_empty() {
            lines.push(Line::from(Span::styled(
                "No commands configured",
                Style::default().fg(theme.text_dim),
            )));
            return lines;
        }

        // Track the global command index across all environments
        let mut global_cmd_idx = 0;

        // Display commands grouped by environment
        // Order: local, docker, k8s
        for env in &["local", "docker", "k8s"] {
            if let Some(commands) = app.commands.get(*env) {
                // Environment header
                lines.push(Line::from(Span::styled(
                    format!("{}:", env.to_uppercase()),
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )));

                // List commands in this environment
                for cmd in commands.iter() {
                    // Check if this command is selected
                    let is_selected = global_cmd_idx == self.selected_command_idx
                        && self.focus == PanelFocus::DetailPanel;
                    
                    // Choose prefix and styling based on selection
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
                        Span::styled(format!(" {:<12}", cmd.name), name_style),
                        Span::styled(cmd.command.clone(), cmd_style),
                    ]));
                    
                    global_cmd_idx += 1;
                }

                lines.push(Line::from("")); // Empty line between environments
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

        let logs_panel = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Log Files")
                    .border_style(border_style)
            );

        frame.render_widget(logs_panel, area);
    }

    /// Builds the content for the logs panel
    /// Shows available log files with name, size, and date
    fn build_logs_content<'a>(&self, app: &'a AppStateData, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        // App name header
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
        lines.push(Line::from("")); // Empty line for spacing

        // Discover log files for this app
        match self.log_manager.list_logs_for_app(&app.name) {
            Ok(log_files) => {
                if log_files.is_empty() {
                    // No log files found - show helpful message
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
                    // Display log files
                    for (idx, log_file) in log_files.iter().enumerate() {
                        // Check if this log file is selected
                        let is_selected = idx == self.selected_log_idx
                            && self.focus == PanelFocus::DetailPanel;
                        
                        // Format file size and date
                        let size_str = LogManager::format_file_size(log_file.size);
                        let date_str = LogManager::format_relative_date(&log_file.modified);
                        
                        // Choose prefix and styling based on selection
                        let (prefix, name_style, meta_style) = if is_selected {
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
                        
                        // First line: prefix + filename
                        lines.push(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(theme.primary)),
                            Span::styled(format!(" {}", log_file.name), name_style),
                        ]));
                        
                        // Second line: size and date (indented)
                        lines.push(Line::from(vec![
                            Span::styled("   ", Style::default()),
                            Span::styled(format!("{}    {}", size_str, date_str), meta_style),
                        ]));
                        
                        // Empty line between log files for readability
                        if idx < log_files.len() - 1 {
                            lines.push(Line::from(""));
                        }
                    }
                }
            }
            Err(e) => {
                // Error reading log files - show error message
                lines.push(Line::from(Span::styled(
                    format!("Error reading log files: {}", e),
                    Style::default().fg(theme.error),
                )));
            }
        }

        lines
    }

    /// Renders the footer with contextual keyboard shortcuts
    /// Shows different shortcuts based on active tab and focus
    /// Visual polish: Context-aware help keeps users informed of available actions
    fn render_footer(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        // Context-aware shortcuts based on active tab
        // UX: Shows only relevant shortcuts to avoid overwhelming users
        let shortcuts = match self.active_tab {
            MainTab::Status => {
                "↑↓/jk: Navigate  ←→: Switch Panel  Tab/1-3: Switch Tab  Space: Expand  ?: Help  q: Quit"
            }
            MainTab::Commands => {
                "↑↓/jk: Navigate  ←→: Switch Panel  Tab/1-3: Switch Tab  Enter: Execute  ?: Help  q: Quit"
            }
            MainTab::Logs => {
                "↑↓/jk: Navigate  ←→: Switch Panel  Tab/1-3: Switch Tab  Enter: View Log  ?: Help  q: Quit"
            }
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
}

impl Default for MainView {
    fn default() -> Self {
        Self::new()
    }
}
