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
            environment,
            state: PopupState::Confirm,
            output_lines: Vec::new(),
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
        self.output_lines.push(line);
        // Keep only the last 10 lines to avoid memory issues
        if self.output_lines.len() > 10 {
            self.output_lines.remove(0);
        }
    }

    /// Renders the popup on the screen
    /// Creates a centered modal dialog with content based on current state
    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        let size = frame.size();
        
        // Create a centered popup area (60% width, 40% height)
        let popup_area = Self::centered_rect(60, 40, size);
        
        // Clear the area behind the popup for proper modal effect
        frame.render_widget(Clear, popup_area);
        
        // Render content based on current state
        match &self.state {
            PopupState::Confirm => self.render_confirm(frame, popup_area, theme),
            PopupState::Executing => self.render_executing(frame, popup_area, theme),
            PopupState::Success(msg) => self.render_success(frame, popup_area, theme, msg),
            PopupState::Error(msg) => self.render_error(frame, popup_area, theme, msg),
        }
    }

    /// Renders the confirmation dialog
    /// Shows command details and asks user to confirm execution
    fn render_confirm(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let mut lines = Vec::new();
        
        // Title
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Execute Command?",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        
        // Command details
        lines.push(Line::from(vec![
            Span::styled("Command:  ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.command_name, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(vec![
            Span::styled("Script:   ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.command_text, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(vec![
            Span::styled("App:      ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.app_name, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(vec![
            Span::styled("Env:      ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.environment, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        
        // Action buttons
        lines.push(Line::from(vec![
            Span::styled("[Enter] ", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
            Span::styled("Execute  ", Style::default().fg(theme.text)),
            Span::styled("[Esc] ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", Style::default().fg(theme.text)),
        ]));
        
        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.primary))
                    .title("Confirm")
            );
        
        frame.render_widget(paragraph, area);
    }

    /// Renders the executing state
    /// Shows a spinner or progress indicator while command runs
    fn render_executing(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        let mut lines = Vec::new();
        
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Executing...",
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        
        lines.push(Line::from(vec![
            Span::styled("Command: ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.command_name, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(vec![
            Span::styled("App:     ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.app_name, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(""));
        
        // Show recent output lines if any
        if !self.output_lines.is_empty() {
            lines.push(Line::from(Span::styled(
                "Output:",
                Style::default().fg(theme.text_dim),
            )));
            for output_line in &self.output_lines {
                lines.push(Line::from(Span::styled(
                    format!("  {}", output_line),
                    Style::default().fg(theme.text_dim),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled(
                "Starting process...",
                Style::default().fg(theme.text_dim),
            )));
        }
        
        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.warning))
                    .title("Executing")
            );
        
        frame.render_widget(paragraph, area);
    }

    /// Renders the success state
    /// Shows success message and allows user to dismiss
    fn render_success(&self, frame: &mut Frame, area: Rect, theme: &Theme, message: &str) {
        let mut lines = Vec::new();
        
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "✓ Success",
            Style::default()
                .fg(theme.success)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        
        lines.push(Line::from(vec![
            Span::styled("Command: ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.command_name, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(vec![
            Span::styled("App:     ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.app_name, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(""));
        
        // Show success message
        lines.push(Line::from(Span::styled(
            message,
            Style::default().fg(theme.text),
        )));
        
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        
        lines.push(Line::from(vec![
            Span::styled("[Enter/Esc] ", Style::default().fg(theme.success).add_modifier(Modifier::BOLD)),
            Span::styled("Close", Style::default().fg(theme.text)),
        ]));
        
        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.success))
                    .title("Success")
            );
        
        frame.render_widget(paragraph, area);
    }

    /// Renders the error state
    /// Shows error message and allows user to dismiss
    fn render_error(&self, frame: &mut Frame, area: Rect, theme: &Theme, message: &str) {
        let mut lines = Vec::new();
        
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "✗ Error",
            Style::default()
                .fg(theme.error)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
        
        lines.push(Line::from(vec![
            Span::styled("Command: ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.command_name, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(vec![
            Span::styled("App:     ", Style::default().fg(theme.text_dim)),
            Span::styled(&self.app_name, Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(""));
        
        // Show error message (may be multi-line)
        for msg_line in message.lines() {
            lines.push(Line::from(Span::styled(
                msg_line,
                Style::default().fg(theme.error),
            )));
        }
        
        lines.push(Line::from(""));
        lines.push(Line::from(""));
        
        lines.push(Line::from(vec![
            Span::styled("[Enter/Esc] ", Style::default().fg(theme.error).add_modifier(Modifier::BOLD)),
            Span::styled("Close", Style::default().fg(theme.text)),
        ]));
        
        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.error))
                    .title("Error")
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
        
        // Add more than 10 lines
        for i in 0..15 {
            popup.add_output_line(format!("Line {}", i));
        }
        
        // Should only keep the last 10 lines
        assert_eq!(popup.output_lines.len(), 10);
        assert_eq!(popup.output_lines[0], "Line 5");
        assert_eq!(popup.output_lines[9], "Line 14");
    }
}
