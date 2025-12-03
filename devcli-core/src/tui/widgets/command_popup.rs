// Command execution popup widget
// Shows confirmation dialog before executing commands and displays execution feedback
// Follows the design pattern of modal dialogs with clear user feedback

use crate::tui::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use std::cell::Cell;

/// Command execution popup that shows confirmation and execution status
/// Manages the lifecycle of command execution from confirmation to completion
#[derive(Debug, Clone)]
pub struct CommandPopup {
    /// Name of the command being executed (e.g., "start", "test")
    pub command_name: String,
    /// The actual command text that will be executed (e.g., "npm start")
    pub command_text: String,
    /// Name of the app this command is for
    pub app_name: String,
    /// Project this app belongs to
    pub project: String,
    /// Environment where the command will run (local, docker, k8s)
    pub environment: String,
    /// Current state of the popup (confirm, executing, success, error)
    state: PopupState,
    /// Output lines from command execution (for displaying feedback)
    output_lines: Vec<String>,
    /// Scroll offset for viewing logs
    scroll_offset: usize,
    /// Auto-scroll to bottom when new lines arrive
    auto_scroll: bool,
    /// Last known visible lines (updated during render using interior mutability)
    last_visible_lines: Cell<usize>,
    /// Whether this popup allows environment selection
    allow_env_selection: bool,
    /// Available environments for selection
    available_envs: Vec<String>,
    /// Currently selected environment index
    selected_env_index: usize,
    /// Current app status (for display in header)
    pub app_status: Option<String>,
}

/// Represents the different states of the command popup
/// Each state determines what content is displayed to the user
#[derive(Debug, Clone, PartialEq)]
pub enum PopupState {
    /// Asking user to confirm command execution
    Confirm,
    /// Command is currently executing in the background
    Executing,
    /// Command completed successfully with optional message
    Success(String),
    /// Command failed with error message
    Error(String),
}

impl CommandPopup {
    /// Creates a new command popup in the confirmation state
    ///
    /// # Arguments
    /// * `command_name` - Display name of the command (e.g., "start")
    /// * `command_text` - Actual command to execute (e.g., "npm start")
    /// * `app_name` - Name of the app this command is for
    /// * `project` - Project this app belongs to
    /// * `environment` - Environment context (local, docker, k8s)
    pub fn new(
        command_name: String,
        command_text: String,
        app_name: String,
        project: String,
        environment: String,
    ) -> Self {
        Self {
            command_name,
            command_text,
            app_name,
            project,
            environment: environment.clone(),
            state: PopupState::Confirm,
            output_lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            last_visible_lines: Cell::new(30), // Default estimate
            allow_env_selection: false,
            available_envs: vec![environment],
            selected_env_index: 0,
            app_status: None,
        }
    }

    /// Creates a new command popup with environment selection enabled
    ///
    /// # Arguments
    /// * `command_name` - Display name of the command (e.g., "start")
    /// * `command_text` - Actual command to execute (e.g., "npm start")
    /// * `app_name` - Name of the app this command is for
    /// * `project` - Project this app belongs to
    /// * `default_env` - Default environment to select
    pub fn new_with_env_selection(
        command_name: String,
        command_text: String,
        app_name: String,
        project: String,
        default_env: String,
    ) -> Self {
        use crate::config::models::Environment;
        let available_envs: Vec<String> = Environment::all()
            .iter()
            .map(|e| e.as_str().to_string())
            .collect();
        let selected_env_index = available_envs
            .iter()
            .position(|e| e == &default_env)
            .unwrap_or(0);

        Self {
            command_name,
            command_text,
            app_name,
            project,
            environment: available_envs[selected_env_index].clone(),
            state: PopupState::Confirm,
            output_lines: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            last_visible_lines: Cell::new(30),
            allow_env_selection: true,
            available_envs,
            selected_env_index,
            app_status: None,
        }
    }

    /// Gets the current state of the popup
    pub fn state(&self) -> &PopupState {
        &self.state
    }

    /// Sets the popup state to executing
    /// Called when user confirms and command starts running
    pub fn set_executing(&mut self) {
        self.state = PopupState::Executing;
        self.output_lines.clear();
    }

    /// Sets the popup state to success with a message
    /// Called when command completes successfully
    pub fn set_success(&mut self, message: String) {
        self.state = PopupState::Success(message);
    }

    /// Sets the popup state to error with a message
    /// Called when command fails
    pub fn set_error(&mut self, message: String) {
        self.state = PopupState::Error(message);
    }

    /// Adds an output line from the executing command
    /// Used to show real-time feedback during execution
    pub fn add_output_line(&mut self, line: String) {
        // Trim trailing whitespace
        let trimmed = line.trim_end().to_string();
        self.output_lines.push(trimmed);
        // Keep only the last 1000 lines
        if self.output_lines.len() > 1000 {
            self.output_lines.remove(0);
            // Adjust scroll offset if needed
            if self.scroll_offset > 0 {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
        }
    }

    /// Scroll up in the log output
    pub fn scroll_up(&mut self) {
        // If auto-scroll is on, we need to set scroll_offset to current bottom position first
        if self.auto_scroll {
            let max_scroll = self
                .output_lines
                .len()
                .saturating_sub(self.last_visible_lines.get());
            self.scroll_offset = max_scroll;
            self.auto_scroll = false;
        }

        // Scroll up by 1 line
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll down in the log output
    pub fn scroll_down(&mut self) {
        // If we have no lines, nothing to do
        if self.output_lines.is_empty() {
            return;
        }

        // Disable auto-scroll when user manually scrolls
        self.auto_scroll = false;

        // Calculate max scroll based on last known visible lines
        let max_scroll = self
            .output_lines
            .len()
            .saturating_sub(self.last_visible_lines.get());

        // Scroll down by 1 line
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }

        // If we reached the bottom, re-enable auto-scroll
        if self.scroll_offset >= max_scroll {
            self.auto_scroll = true;
        }
    }

    /// Enable auto-scroll to bottom
    pub fn enable_auto_scroll(&mut self) {
        self.auto_scroll = true;
    }

    /// Reset scroll to top
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = false;
    }

    /// Cycle to next environment (only if env selection is enabled)
    pub fn next_environment(&mut self) {
        if self.allow_env_selection && self.state == PopupState::Confirm {
            self.selected_env_index = (self.selected_env_index + 1) % self.available_envs.len();
            self.environment = self.available_envs[self.selected_env_index].clone();
        }
    }

    /// Cycle to previous environment (only if env selection is enabled)
    pub fn prev_environment(&mut self) {
        if self.allow_env_selection && self.state == PopupState::Confirm {
            if self.selected_env_index == 0 {
                self.selected_env_index = self.available_envs.len() - 1;
            } else {
                self.selected_env_index -= 1;
            }
            self.environment = self.available_envs[self.selected_env_index].clone();
        }
    }

    /// Check if environment selection is allowed
    pub fn allows_env_selection(&self) -> bool {
        self.allow_env_selection
    }

    /// Update the app status for display in header
    pub fn update_status(&mut self, status: String) {
        self.app_status = Some(status);
    }

    /// Renders a dimmed overlay over the entire screen
    /// This creates a subtle dimming effect by using a semi-transparent appearance
    fn render_overlay(&self, frame: &mut Frame, area: Rect, _theme: &Theme) {
        // Create a subtle dimmed background using a pattern
        // Since terminals don't support true transparency, we use:
        // 1. A dark background color
        // 2. Dim modifier to make it less intense
        let overlay_style = _theme.style_bg_overlay_dim();

        // Create a block that covers the entire area
        let overlay_block = Block::default().style(overlay_style);

        frame.render_widget(overlay_block, area);
    }

    /// Renders the popup on the screen
    /// Creates a centered modal dialog with content based on current state
    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let size = frame.area();

        // Render a dimmed overlay over the entire screen for better focus
        self.render_overlay(frame, size, theme);

        // Determine if this is a stop operation (no logs expected)
        // Restart shows logs like start
        let is_stop = self.command_name == "stop";
        let has_logs = !self.output_lines.is_empty();

        // Create popup area - smaller for stop, larger for start/restart with logs
        let popup_area = match &self.state {
            PopupState::Error(_) if has_logs => Self::centered_rect(90, 80, size), // Large for errors with logs
            PopupState::Success(_) if has_logs => Self::centered_rect(90, 80, size), // Large for success with logs
            PopupState::Error(_) => Self::centered_rect(70, 40, size), // Medium for simple errors
            PopupState::Executing if is_stop => Self::centered_rect(50, 30, size), // Small for stop
            PopupState::Executing => Self::centered_rect(90, 80, size), // Large for start/restart with logs
            _ => Self::centered_rect(50, 30, size),                     // Compact for other states
        };

        // Clear the area behind the popup for proper modal effect
        frame.render_widget(Clear, popup_area);

        // Render content based on current state
        match &self.state {
            PopupState::Confirm => self.render_confirm(frame, popup_area, theme),
            PopupState::Executing if is_stop => {
                self.render_executing_simple(frame, popup_area, theme)
            }
            PopupState::Executing => self.render_executing(frame, popup_area, theme),
            PopupState::Success(msg) => {
                if has_logs {
                    self.render_log_view(
                        frame,
                        popup_area,
                        theme,
                        "Success",
                        theme.success,
                        Some(msg),
                    )
                } else {
                    self.render_success(frame, popup_area, theme, msg)
                }
            }
            PopupState::Error(msg) => {
                if has_logs {
                    self.render_log_view(frame, popup_area, theme, "Error", theme.error, Some(msg))
                } else {
                    self.render_error(frame, popup_area, theme, msg)
                }
            }
        }
    }

    /// Renders the confirmation dialog
    /// Shows command details and asks user to confirm execution
    fn render_confirm(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Execute Command?",
                theme.style_text_primary_bold(),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Command:  ", theme.style_text_dim()),
                Span::styled(&self.command_name, theme.style_text()),
            ]),
            Line::from(vec![
                Span::styled("Script:   ", theme.style_text_dim()),
                Span::styled(&self.command_text, theme.style_text()),
            ]),
            Line::from(vec![
                Span::styled("App:      ", theme.style_text_dim()),
                Span::styled(&self.app_name, theme.style_text()),
            ]),
        ];

        // Environment line - show selection UI if enabled
        if self.allow_env_selection {
            let mut env_spans = vec![Span::styled("Env:      ", theme.style_text_dim())];

            // Show all environments with the selected one highlighted
            for (i, env) in self.available_envs.iter().enumerate() {
                if i > 0 {
                    env_spans.push(Span::styled(" | ", theme.style_text_dim()));
                }

                if i == self.selected_env_index {
                    env_spans.push(Span::styled(
                        format!("[{}]", env),
                        theme.style_text_primary_bold(),
                    ));
                } else {
                    env_spans.push(Span::styled(env.as_str(), theme.style_text_dim()));
                }
            }

            lines.push(Line::from(env_spans));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Env:      ", theme.style_text_dim()),
                Span::styled(&self.environment, theme.style_text()),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(""));

        // Controls - show arrow keys if env selection is enabled
        if self.allow_env_selection {
            lines.push(Line::from(vec![
                Span::styled("[←→] ", theme.style_text_primary_bold()),
                Span::styled("Select Env  ", theme.style_text()),
                Span::styled(
                    "[Enter] ",
                    theme.style_text_success().add_modifier(Modifier::BOLD),
                ),
                Span::styled("Execute  ", theme.style_text()),
                Span::styled(
                    "[Esc] ",
                    theme.style_text_error().add_modifier(Modifier::BOLD),
                ),
                Span::styled("Cancel", theme.style_text()),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    "[Enter] ",
                    theme.style_text_success().add_modifier(Modifier::BOLD),
                ),
                Span::styled("Execute  ", theme.style_text()),
                Span::styled(
                    "[Esc] ",
                    theme.style_text_error().add_modifier(Modifier::BOLD),
                ),
                Span::styled("Cancel", theme.style_text()),
            ]));
        }

        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(theme.style_bg_default())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style_border_focused())
                    .title("Confirm"),
            );

        frame.render_widget(paragraph, area);
    }

    /// Renders a simple executing state for stop/restart operations
    /// Shows status message without logs
    fn render_executing_simple(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let status_message = if !self.output_lines.is_empty() {
            // Show the last status line if we have any
            self.output_lines
                .last()
                .unwrap_or(&"Processing...".to_string())
                .clone()
        } else {
            format!(
                "{}...",
                if self.command_name == "stop" {
                    "Stopping"
                } else {
                    "Restarting"
                }
            )
        };

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                &status_message,
                theme.style_text_warning().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("App: ", theme.style_text_dim()),
                Span::styled(&self.app_name, theme.style_text()),
            ]),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "[Esc] ",
                    theme.style_text_dim().add_modifier(Modifier::BOLD),
                ),
                Span::styled("Close", theme.style_text_dim()),
            ]),
        ];

        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(theme.style_bg_default())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style_text_warning())
                    .title(format!(" {} ", self.command_name.to_uppercase())),
            );

        frame.render_widget(paragraph, area);
    }

    /// Renders the executing state
    /// Shows logs streaming from the process
    fn render_executing(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.render_log_view(frame, area, theme, "Logs", theme.warning, None)
    }

    /// Generic log viewer renderer
    fn render_log_view(
        &self,
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        title_suffix: &str,
        border_color: ratatui::style::Color,
        status_msg: Option<&str>,
    ) {
        use ratatui::widgets::Scrollbar;
        use ratatui::widgets::ScrollbarOrientation;
        use ratatui::widgets::ScrollbarState;

        // Create main block
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(theme.style_bg_default())
            .title(format!(" {} - {} ", self.app_name, title_suffix));

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        // Split into header, content, footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content (scrollable)
                Constraint::Length(3), // Footer
            ])
            .split(inner_area);

        // Render header (always visible)
        let mut header_spans = vec![
            Span::styled("App: ", theme.style_text_dim()),
            Span::styled(&self.app_name, theme.style_text()),
            Span::styled("  Command: ", theme.style_text_dim()),
            Span::styled(&self.command_name, theme.style_text()),
        ];

        // Add status if available
        if let Some(status) = &self.app_status {
            header_spans.push(Span::styled("  Status: ", theme.style_text_dim()));

            // Color the status based on whether it's running or stopped
            let status_color = if status.to_lowercase().contains("running") {
                theme.success
            } else {
                theme.text_dim
            };
            header_spans.push(Span::styled(status, Style::default().fg(status_color)));
        }

        // Add specific status message if provided (e.g. Error message)
        if let Some(msg) = status_msg {
            header_spans.push(Span::styled("  Result: ", theme.style_text_dim()));
            header_spans.push(Span::styled(
                msg,
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let header_lines = vec![Line::from(""), Line::from(header_spans)];
        let header = Paragraph::new(header_lines).alignment(Alignment::Left);
        frame.render_widget(header, chunks[0]);

        // Calculate scroll position for content
        let available_height = chunks[1].height as usize;

        // Update last_visible_lines for scroll calculations
        self.last_visible_lines.set(available_height);

        let scroll_pos = if self.auto_scroll && !self.output_lines.is_empty() {
            self.output_lines.len().saturating_sub(available_height)
        } else {
            self.scroll_offset
                .min(self.output_lines.len().saturating_sub(available_height))
        };

        // Render content (scrollable logs)
        let mut content_lines = Vec::new();
        if !self.output_lines.is_empty() {
            let visible_lines: Vec<_> = self
                .output_lines
                .iter()
                .skip(scroll_pos)
                .take(available_height)
                .collect();

            for output_line in visible_lines {
                // Use the shared ANSI parser to convert escape codes to styled Spans
                // This handles colors (foreground/background) and styles (bold, dim, etc.)
                // The parser is located in src/tui/utils/ansi.rs for reusability
                let spans = crate::tui::utils::ansi::parse_ansi_codes(output_line);
                content_lines.push(Line::from(spans));
            }
        } else {
            content_lines.push(Line::from(Span::styled(
                "Waiting for output...",
                theme.style_text_dim(),
            )));
        }

        let content = Paragraph::new(content_lines).alignment(Alignment::Left);
        frame.render_widget(content, chunks[1]);

        // Render scrollbar if needed
        if self.output_lines.len() > available_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            // ScrollbarState needs the total content size and viewport size
            let max_scroll = self.output_lines.len().saturating_sub(available_height);
            let mut scrollbar_state =
                ScrollbarState::new(max_scroll.max(1)).position(scroll_pos.min(max_scroll));

            frame.render_stateful_widget(scrollbar, chunks[1], &mut scrollbar_state);
        }

        // Render footer (always visible)
        let scroll_indicator = if self.output_lines.len() > available_height {
            format!(
                " [{}/{}] ",
                (scroll_pos + available_height).min(self.output_lines.len()),
                self.output_lines.len()
            )
        } else {
            String::new()
        };

        let footer_lines = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "[↑↓/jk] ",
                    Style::default()
                        .fg(theme.text_dim)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Scroll  ", theme.style_text_dim()),
                Span::styled(
                    "[Home/End] ",
                    Style::default()
                        .fg(theme.text_dim)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Top/Bottom  ", theme.style_text_dim()),
                Span::styled(
                    "[Esc/Enter] ",
                    Style::default()
                        .fg(theme.text_dim)
                        .add_modifier(Modifier::BOLD),
                ), // Allow Enter to close too
                Span::styled("Close", theme.style_text_dim()),
                Span::styled(scroll_indicator, theme.style_text_dim()),
            ]),
        ];
        let footer = Paragraph::new(footer_lines).alignment(Alignment::Left);
        frame.render_widget(footer, chunks[2]);
    }

    /// Renders the success state
    /// Shows success message and allows user to dismiss
    fn render_success(&self, frame: &mut Frame, area: Rect, theme: &Theme, message: &str) {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "✓ Success",
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Command: ", theme.style_text_dim()),
                Span::styled(&self.command_name, theme.style_text()),
            ]),
            Line::from(vec![
                Span::styled("App:     ", theme.style_text_dim()),
                Span::styled(&self.app_name, theme.style_text()),
            ]),
            Line::from(""),
            Line::from(Span::styled(message, theme.style_text())),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "[Enter/Esc] ",
                    Style::default()
                        .fg(theme.success)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("Close", theme.style_text()),
            ]),
        ];

        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(theme.style_bg_default())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style_border_success())
                    .title("Success"),
            );

        frame.render_widget(paragraph, area);
    }

    /// Renders the error state
    /// Shows error message and allows user to dismiss
    fn render_error(&self, frame: &mut Frame, area: Rect, theme: &Theme, message: &str) {
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "✗ Error",
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Command: ", theme.style_text_dim()),
                Span::styled(&self.command_name, theme.style_text()),
            ]),
            Line::from(vec![
                Span::styled("App:     ", theme.style_text_dim()),
                Span::styled(&self.app_name, theme.style_text()),
            ]),
            Line::from(""),
        ];

        // Show error message (may be multi-line)
        for msg_line in message.lines() {
            lines.push(Line::from(Span::styled(msg_line, theme.style_text_error())));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(""));

        lines.push(Line::from(vec![
            Span::styled(
                "[Enter/Esc] ",
                Style::default()
                    .fg(theme.error)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Close", theme.style_text()),
        ]));

        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(theme.style_bg_default())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style_border_error())
                    .title("Error"),
            );

        frame.render_widget(paragraph, area);
    }

    /// Helper function to create a centered rectangle
    /// Used for positioning the popup in the middle of the screen
    ///
    /// # Arguments
    /// * `percent_x` - Width as percentage of screen (0-100)
    /// * `percent_y` - Height as percentage of screen (0-100)
    /// * `r` - The full screen area
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        // Create vertical layout: top margin, content, bottom margin
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        // Create horizontal layout within the middle section
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_creation() {
        let popup = CommandPopup::new(
            "start".to_string(),
            "npm start".to_string(),
            "api-server".to_string(),
            "my-project".to_string(),
            "local".to_string(),
        );

        assert_eq!(popup.state(), &PopupState::Confirm);
        assert_eq!(popup.command_name, "start");
        assert_eq!(popup.command_text, "npm start");
        assert_eq!(popup.app_name, "api-server");
        assert_eq!(popup.project, "my-project");
        assert_eq!(popup.environment, "local");
    }

    #[test]
    fn test_popup_state_transitions() {
        let mut popup = CommandPopup::new(
            "start".to_string(),
            "npm start".to_string(),
            "api-server".to_string(),
            "my-project".to_string(),
            "local".to_string(),
        );

        // Initial state is Confirm
        assert_eq!(popup.state(), &PopupState::Confirm);

        // Transition to Executing
        popup.set_executing();
        assert_eq!(popup.state(), &PopupState::Executing);

        // Transition to Success
        popup.set_success("App started successfully".to_string());
        assert!(matches!(popup.state(), PopupState::Success(_)));

        // Reset and transition to Error
        popup.state = PopupState::Confirm;
        popup.set_error("Failed to start app".to_string());
        assert!(matches!(popup.state(), PopupState::Error(_)));
    }

    #[test]
    fn test_output_lines_limit() {
        let mut popup = CommandPopup::new(
            "start".to_string(),
            "npm start".to_string(),
            "api-server".to_string(),
            "my-project".to_string(),
            "local".to_string(),
        );

        // Add more than 1000 lines
        for i in 0..1050 {
            popup.add_output_line(format!("Line {}", i));
        }

        // Should only keep the last 1000 lines
        assert_eq!(popup.output_lines.len(), 1000);
        assert_eq!(popup.output_lines[0], "Line 50");
        assert_eq!(popup.output_lines[999], "Line 1049");
    }
}
