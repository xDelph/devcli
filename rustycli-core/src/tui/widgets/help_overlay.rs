// Help overlay widget
// Displays all available keyboard shortcuts in a modal dialog
// Activated by pressing '?' key from any view

use crate::tui::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Help overlay that displays all keyboard shortcuts
/// Shows context-sensitive help based on the current view
pub struct HelpOverlay {
    /// Whether the help overlay is currently visible
    visible: bool,
}

impl HelpOverlay {
    /// Creates a new help overlay (initially hidden)
    pub fn new() -> Self {
        Self { visible: false }
    }

    /// Shows the help overlay
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hides the help overlay
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Returns whether the help overlay is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Toggles the visibility of the help overlay
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Renders the help overlay on the screen
    /// Creates a centered modal dialog with all keyboard shortcuts
    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        if !self.visible {
            return;
        }

        let size = frame.area();
        
        // Create a centered popup area (70% width, 80% height)
        let popup_area = Self::centered_rect(70, 80, size);
        
        // Clear the area behind the popup for proper modal effect
        frame.render_widget(Clear, popup_area);
        
        // Build the help content
        let lines = self.build_help_content(theme);
        
        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Left)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.primary))
                    .title(" Keyboard Shortcuts - Press ? or Esc to close ")
            );
        
        frame.render_widget(paragraph, popup_area);
    }

    /// Builds the help content with all keyboard shortcuts
    /// Organizes shortcuts by category for easy reference
    fn build_help_content<'a>(&self, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();
        
        // Add spacing at top
        lines.push(Line::from(""));
        
        // Global shortcuts section
        lines.push(Line::from(Span::styled(
            "Global Shortcuts",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        
        self.add_shortcut(&mut lines, theme, "q", "Quit application");
        self.add_shortcut(&mut lines, theme, "Ctrl+C", "Quit application");
        self.add_shortcut(&mut lines, theme, "Esc", "Go back / Cancel / Close popup");
        self.add_shortcut(&mut lines, theme, "?", "Show/hide this help overlay");
        
        lines.push(Line::from(""));
        
        // Navigation shortcuts section
        lines.push(Line::from(Span::styled(
            "Navigation",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        
        self.add_shortcut(&mut lines, theme, "↑ / k", "Move selection up");
        self.add_shortcut(&mut lines, theme, "↓ / j", "Move selection down");
        self.add_shortcut(&mut lines, theme, "Tab", "Switch panel focus");
        self.add_shortcut(&mut lines, theme, "Enter", "Select / Execute / Confirm");
        self.add_shortcut(&mut lines, theme, "Space", "Expand/collapse project");
        
        lines.push(Line::from(""));
        
        // Tab shortcuts section
        lines.push(Line::from(Span::styled(
            "Tab Switching",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        
        self.add_shortcut(&mut lines, theme, "1", "Switch to Status tab");
        self.add_shortcut(&mut lines, theme, "2", "Switch to Commands tab");
        self.add_shortcut(&mut lines, theme, "3", "Switch to Logs tab");
        
        lines.push(Line::from(""));
        
        // Status tab shortcuts section
        lines.push(Line::from(Span::styled(
            "Status Tab",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        
        self.add_shortcut(&mut lines, theme, "Enter", "Start app (if stopped)");
        self.add_shortcut(&mut lines, theme, "s", "Stop running app");
        self.add_shortcut(&mut lines, theme, "r", "Restart running app");
        
        lines.push(Line::from(""));
        
        // Commands tab shortcuts section
        lines.push(Line::from(Span::styled(
            "Commands Tab",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        
        self.add_shortcut(&mut lines, theme, "Enter", "Execute selected command");
        
        lines.push(Line::from(""));
        
        // Logs tab shortcuts section
        lines.push(Line::from(Span::styled(
            "Logs Tab",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        
        self.add_shortcut(&mut lines, theme, "Enter", "Open log viewer");
        self.add_shortcut(&mut lines, theme, "/", "Search in current view");
        
        lines.push(Line::from(""));
        
        // Log viewer shortcuts section
        lines.push(Line::from(Span::styled(
            "Log Viewer",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        
        self.add_shortcut(&mut lines, theme, "↑ / ↓", "Scroll line by line");
        self.add_shortcut(&mut lines, theme, "PgUp / PgDn", "Scroll page by page");
        self.add_shortcut(&mut lines, theme, "Home / End", "Jump to start/end");
        self.add_shortcut(&mut lines, theme, "g / G", "Jump to top/bottom (vim-style)");
        self.add_shortcut(&mut lines, theme, "/", "Enter search mode");
        self.add_shortcut(&mut lines, theme, "n / N", "Next/previous search result");
        self.add_shortcut(&mut lines, theme, "Esc", "Exit viewer");
        
        lines.push(Line::from(""));
        
        // Status indicators section
        lines.push(Line::from(Span::styled(
            "Status Indicators",
            Style::default()
                .fg(theme.primary)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));
        lines.push(Line::from(""));
        
        lines.push(Line::from(vec![
            Span::styled("  ● ", Style::default().fg(theme.running)),
            Span::styled("Running  ", Style::default().fg(theme.text)),
            Span::styled("  ○ ", Style::default().fg(theme.stopped)),
            Span::styled("Stopped", Style::default().fg(theme.text)),
        ]));
        
        lines.push(Line::from(""));
        
        lines
    }

    /// Helper to add a shortcut line with consistent formatting
    /// 
    /// # Arguments
    /// * `lines` - Vector to append the line to
    /// * `theme` - Theme for styling
    /// * `key` - The keyboard shortcut
    /// * `description` - What the shortcut does
    fn add_shortcut<'a>(&self, lines: &mut Vec<Line<'a>>, theme: &Theme, key: &'a str, description: &'a str) {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{:12}", key),
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(description, Style::default().fg(theme.text)),
        ]));
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

impl Default for HelpOverlay {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_overlay_creation() {
        let overlay = HelpOverlay::new();
        assert!(!overlay.is_visible());
    }

    #[test]
    fn test_help_overlay_show_hide() {
        let mut overlay = HelpOverlay::new();
        
        // Initially hidden
        assert!(!overlay.is_visible());
        
        // Show
        overlay.show();
        assert!(overlay.is_visible());
        
        // Hide
        overlay.hide();
        assert!(!overlay.is_visible());
    }

    #[test]
    fn test_help_overlay_toggle() {
        let mut overlay = HelpOverlay::new();
        
        // Initially hidden
        assert!(!overlay.is_visible());
        
        // Toggle to show
        overlay.toggle();
        assert!(overlay.is_visible());
        
        // Toggle to hide
        overlay.toggle();
        assert!(!overlay.is_visible());
    }

    #[test]
    fn test_help_content_generation() {
        let overlay = HelpOverlay::new();
        let theme = Theme::default();
        
        let lines = overlay.build_help_content(&theme);
        
        // Should have content
        assert!(!lines.is_empty());
        
        // Should contain key sections (check for some expected text)
        let content_str = lines.iter()
            .map(|line| {
                line.spans.iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");
        
        assert!(content_str.contains("Global Shortcuts"));
        assert!(content_str.contains("Navigation"));
        assert!(content_str.contains("Tab Switching"));
        assert!(content_str.contains("Commands Tab"));
        assert!(content_str.contains("Logs Tab"));
        assert!(content_str.contains("Log Viewer"));
        assert!(content_str.contains("Status Indicators"));
    }
}
