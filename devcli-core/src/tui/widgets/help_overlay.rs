// Help overlay widget
// Displays all available keyboard shortcuts in a modal dialog
// Activated by pressing '?' key from any view

use crate::tui::theme::Theme;
use ratatui::{
    layout::Alignment,
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
    /// First line shown (the help is longer than most terminals)
    scroll: u16,
    /// Largest useful `scroll`, known once rendered
    max_scroll: std::cell::Cell<u16>,
}

impl HelpOverlay {
    /// Creates a new help overlay (initially hidden)
    pub fn new() -> Self {
        Self {
            visible: false,
            scroll: 0,
            max_scroll: std::cell::Cell::new(u16::MAX),
        }
    }

    /// Shows the help overlay, from the top
    pub fn show(&mut self) {
        self.visible = true;
        self.scroll = 0;
    }

    /// Scrolls the help up by `lines`
    pub fn scroll_up(&mut self, lines: u16) {
        self.scroll = self.scroll.saturating_sub(lines);
    }

    /// Scrolls the help down by `lines` (not past its end)
    pub fn scroll_down(&mut self, lines: u16) {
        self.scroll = self.scroll.saturating_add(lines).min(self.max_scroll.get());
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
        if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Renders the help overlay on the screen
    /// Creates a centered modal dialog with all keyboard shortcuts
    pub fn render(&self, frame: &mut Frame, theme: &Theme) {
        if !self.visible {
            return;
        }

        let size = frame.area();

        // Create a centered popup area (70% width, 80% height)
        let popup_area = crate::tui::utils::layout::centered_rect(70, 80, size);

        // Clear the area behind the popup for proper modal effect
        frame.render_widget(Clear, popup_area);

        // Build the help content
        let lines = self.build_help_content(theme);
        let visible = popup_area.height.saturating_sub(2);
        let max_scroll = u16::try_from(lines.len())
            .unwrap_or(u16::MAX)
            .saturating_sub(visible);
        self.max_scroll.set(max_scroll);
        let scroll = self.scroll.min(max_scroll);

        let paragraph = Paragraph::new(lines)
            .alignment(Alignment::Left)
            .scroll((scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.style_text_primary())
                    .title(" Keyboard Shortcuts - ↑↓/PgUp/PgDn: scroll, any other key: close "),
            );

        frame.render_widget(paragraph, popup_area);
    }

    /// Builds the help content with all keyboard shortcuts
    /// Organizes shortcuts by category for easy reference
    ///
    /// Written from the key handlers (input/router.rs, main_view/input_handler.rs,
    /// input/edit_mode.rs, popups/manager.rs, log_viewer/{mod,single_view}.rs):
    /// keep it in step with them, and with the TUI section of
    /// docs/commands-reference.md.
    fn build_help_content<'a>(&self, theme: &'a Theme) -> Vec<Line<'a>> {
        let mut lines = Vec::new();
        lines.push(Line::from(""));

        let sections: &[(&str, &[(&'static str, &'static str)])] = &[
            (
                "Global",
                &[
                    ("q", "Quit (not while typing, nor in a popup)"),
                    ("Ctrl+C", "Quit"),
                    ("?", "Show this help (↑↓/PgUp/PgDn scroll it)"),
                    ("Esc", "Close a popup / go back"),
                ],
            ),
            (
                "App list and panels",
                &[
                    ("1 / 2", "Status tab / Config tab"),
                    ("Tab", "Next tab"),
                    ("← / →", "Focus the app list / the detail panel"),
                    ("↑↓ / j k", "Move in the focused panel"),
                    ("Shift+↑↓", "Move by 10"),
                    ("Space", "Expand/collapse the project (app list)"),
                    ("l", "Open the app's latest log (running), else pick a log"),
                ],
            ),
            (
                "Status tab",
                &[
                    ("s", "Start the app (pick the environment) / stop it"),
                    ("r", "Restart the running app"),
                    ("Enter", "Open the selected log (detail panel)"),
                ],
            ),
            (
                "Config tab",
                &[
                    ("Enter", "Run the selected command (detail panel)"),
                    ("a", "Add a command"),
                    ("e", "Edit the selected command (detail panel)"),
                    ("s", "Make the selected command its default (detail panel)"),
                    ("d", "Delete the selected command (detail) / the app (list)"),
                    ("E", "Edit the app (type, path, default commands)"),
                    ("D", "Dependencies: a add, d remove"),
                    ("f", "Env files: a add, e edit, d delete"),
                    ("r", "Restart the running app (app list)"),
                ],
            ),
            (
                "Forms",
                &[
                    ("Tab", "Next field (↑↓ too in Edit app)"),
                    ("← / →", "Move the cursor"),
                    ("Space", "Next environment (Add command, environment field)"),
                    ("Enter", "Save"),
                    ("Esc", "Cancel"),
                    ("y / n", "Confirm / cancel a deletion"),
                ],
            ),
            (
                "Command popup",
                &[
                    ("← / →", "Choose the environment (start)"),
                    ("Enter", "Confirm / close once done"),
                    ("↑↓ / j k", "Scroll the output"),
                    ("Home / End", "Top / follow the output"),
                    ("Esc", "Close"),
                ],
            ),
            (
                "Log viewer",
                &[
                    ("↑↓ / j k", "Move line by line (Shift: by 10)"),
                    ("PgUp / PgDn", "Move by 20 lines"),
                    ("g / G", "Top / bottom (Home / End)"),
                    ("/", "Search: type, ↑↓ results, Enter keep, Esc cancel"),
                    ("n / N", "Next / previous result of the kept search"),
                    ("Esc", "Clear the kept search, else leave the viewer"),
                    ("n", "Add a panel (no search kept; up to 4)"),
                    ("L", "Show another log in this panel"),
                    ("Tab", "Next panel"),
                    ("w", "Close the panel (when there are several)"),
                    ("J", "Show/hide the JSON panel (JSON lines)"),
                ],
            ),
        ];
        for (title, shortcuts) in sections {
            lines.push(Line::from(Span::styled(
                *title,
                Style::default()
                    .fg(theme.primary)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            )));
            lines.push(Line::from(""));
            for (key, description) in *shortcuts {
                self.add_shortcut(&mut lines, theme, key, description);
            }
            lines.push(Line::from(""));
        }

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
            Span::styled("Running  ", theme.style_text()),
            Span::styled("  ○ ", Style::default().fg(theme.stopped)),
            Span::styled("Stopped", theme.style_text()),
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
    fn add_shortcut<'a>(
        &self,
        lines: &mut Vec<Line<'a>>,
        theme: &Theme,
        key: &'a str,
        description: &'a str,
    ) {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("{:12}", key),
                Style::default()
                    .fg(theme.success)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(description, theme.style_text()),
        ]));
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
        let content_str = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        for section in [
            "Global",
            "App list and panels",
            "Status tab",
            "Config tab",
            "Forms",
            "Command popup",
            "Log viewer",
            "Status Indicators",
        ] {
            assert!(content_str.contains(section), "{section}");
        }
        // What it used to get wrong.
        assert!(!content_str.contains("Logs Tab"));
        assert!(!content_str.contains("Commands Tab"));
        assert!(!content_str.contains("Switch panel focus"));
    }
}
