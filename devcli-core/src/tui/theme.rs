// Theme management for the TUI
// Provides color schemes and visual styling inspired by GitUI
// Colors are chosen to work well in both light and dark terminals

use ratatui::style::{Color, Modifier, Style};

/// Theme defines the color palette for the TUI
/// All colors are carefully selected to provide good contrast and readability
#[derive(Debug, Clone)]
pub struct Theme {
    /// Primary accent color - used for highlights and focus
    pub primary: Color,
    /// Secondary accent color - used for less important highlights
    pub secondary: Color,
    /// Success color - used for positive states (e.g., running apps)
    pub success: Color,
    /// Warning color - used for cautionary states
    pub warning: Color,
    /// Error color - used for error states and stopped apps
    pub error: Color,
    /// Running indicator color - green to show active processes
    pub running: Color,
    /// Stopped indicator color - red/gray to show inactive processes
    pub stopped: Color,
    /// Selected background color - highlights the currently selected item
    pub selected_bg: Color,
    /// Border color - used for panel borders and separators
    pub border: Color,
    /// Primary text color - used for most text content
    pub text: Color,
    /// Dimmed text color - used for secondary information
    pub text_dim: Color,
    /// Background color - used for popup backgrounds
    pub bg: Color,
    /// Overlay background color - used for popup overlays
    pub bg_overlay: Color,
    /// Highlight background color - used for highlighted lines
    pub bg_highlight: Color,
}

impl Theme {
    /// Creates the default theme with GitUI-inspired colors
    /// This theme works well in most terminal environments
    /// Visual polish: Carefully chosen colors for optimal contrast and readability
    pub fn new() -> Self {
        Self {
            // Cyan for primary highlights - stands out without being harsh
            // Used for focused borders, selected tabs, and important UI elements
            primary: Color::Rgb(0, 255, 255),
            // Blue for secondary elements - complements primary color
            // Used for section headers and less prominent highlights
            secondary: Color::Rgb(100, 149, 237),
            // Green for success states - universally recognized positive indicator
            success: Color::Rgb(0, 255, 0),
            // Yellow for warnings - draws attention without alarming
            warning: Color::Rgb(255, 255, 0),
            // Red for errors - clear danger signal
            error: Color::Rgb(255, 0, 0),
            // Bright green for running status - highly visible and positive
            // Optimization: Uses LightGreen for better visibility in dark terminals
            running: Color::Rgb(144, 238, 144),
            // Dark gray for stopped status - subdued to indicate inactive state
            // Visual consistency: Muted color reduces visual noise
            stopped: Color::Rgb(169, 169, 169),
            // Dark gray background for selected items - subtle but clear
            // Provides good contrast without being too bright
            selected_bg: Color::Rgb(169, 169, 169),
            // Gray for borders - visible but not distracting
            // Visual polish: Keeps focus on content, not chrome
            border: Color::Rgb(128, 128, 128),
            // White for primary text - maximum readability
            // Ensures text is crisp and easy to read
            text: Color::Rgb(255, 255, 255),
            // Gray for dimmed text - de-emphasizes secondary info
            // Visual hierarchy: Helps users focus on important information
            text_dim: Color::Rgb(128, 128, 128),
            // Black background for popups and overlays
            bg: Color::Rgb(0, 0, 0),
            // Very dark blue-gray for overlay backgrounds
            bg_overlay: Color::Rgb(30, 30, 35),
            // Dark blue-gray for highlighted lines
            bg_highlight: Color::Rgb(40, 40, 60),
        }
    }

    // ========================================================================
    // Style preset methods - use these instead of inline Style::default()
    // ========================================================================

    /// Returns a base style with no colors or modifiers
    pub fn style_default(&self) -> Style {
        Style::default()
    }

    /// Regular text style - white text, no modifiers
    pub fn style_text(&self) -> Style {
        Style::default().fg(self.text)
    }

    /// Dimmed text style - for secondary information
    pub fn style_text_dim(&self) -> Style {
        Style::default().fg(self.text_dim)
    }

    /// Bold text style
    pub fn style_text_bold(&self) -> Style {
        Style::default().fg(self.text).add_modifier(Modifier::BOLD)
    }

    /// Primary colored text
    pub fn style_text_primary(&self) -> Style {
        Style::default().fg(self.primary)
    }

    /// Bold primary colored text - for focused/important elements
    pub fn style_text_primary_bold(&self) -> Style {
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Secondary colored text
    pub fn style_text_secondary(&self) -> Style {
        Style::default().fg(self.secondary)
    }

    /// Bold secondary colored text
    pub fn style_text_secondary_bold(&self) -> Style {
        Style::default()
            .fg(self.secondary)
            .add_modifier(Modifier::BOLD)
    }

    /// Error colored text
    pub fn style_text_error(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Warning colored text
    pub fn style_text_warning(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Success colored text
    pub fn style_text_success(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Running status colored text - bright green
    pub fn style_text_running(&self) -> Style {
        Style::default().fg(self.running)
    }

    /// Stopped status colored text - dark gray
    pub fn style_text_stopped(&self) -> Style {
        Style::default().fg(self.stopped)
    }

    /// Style with default black background
    pub fn style_bg_default(&self) -> Style {
        Style::default().bg(self.bg)
    }

    /// Style with overlay background - for popup overlays
    pub fn style_bg_overlay(&self) -> Style {
        Style::default().bg(self.bg_overlay)
    }

    /// Style with overlay background and dimmed - for subtle overlays
    pub fn style_bg_overlay_dim(&self) -> Style {
        Style::default()
            .bg(self.bg_overlay)
            .add_modifier(Modifier::DIM)
    }

    /// Style with selected background
    pub fn style_bg_selected(&self) -> Style {
        Style::default().bg(self.selected_bg)
    }

    /// Selected background with text color
    pub fn style_bg_selected_text(&self) -> Style {
        Style::default().bg(self.selected_bg).fg(self.text)
    }

    /// Selected background with primary color
    pub fn style_bg_selected_primary(&self) -> Style {
        Style::default().bg(self.selected_bg).fg(self.primary)
    }

    /// Selected background with secondary color
    pub fn style_bg_selected_secondary(&self) -> Style {
        Style::default().bg(self.selected_bg).fg(self.secondary)
    }

    /// Style with highlight background - for cursor lines
    pub fn style_bg_highlight(&self) -> Style {
        Style::default().bg(self.bg_highlight)
    }

    /// Default border style - gray
    pub fn style_border_default(&self) -> Style {
        Style::default().fg(self.border)
    }

    /// Focused border style - primary color
    pub fn style_border_focused(&self) -> Style {
        Style::default().fg(self.primary)
    }

    /// Error border style - red
    pub fn style_border_error(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Success border style - green
    pub fn style_border_success(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Section header style - bold and underlined
    pub fn style_section_header(&self) -> Style {
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    /// Error block style - error background with text
    pub fn style_error_block(&self) -> Style {
        Style::default().bg(self.error).fg(self.text)
    }

    /// Text with error background and bold - for critical errors
    pub fn style_error_bold(&self) -> Style {
        Style::default()
            .bg(self.error)
            .fg(self.text)
            .add_modifier(Modifier::BOLD)
    }

    /// Text with default background - common for log viewers
    pub fn style_text_with_bg(&self) -> Style {
        Style::default().fg(self.text).bg(self.bg)
    }

    /// Dimmed text with default background
    pub fn style_text_dim_with_bg(&self) -> Style {
        Style::default().fg(self.text_dim).bg(self.bg)
    }

    /// Creates a theme based on terminal color detection
    /// This can be extended to detect terminal capabilities and adjust colors
    /// For now, it returns the default theme
    pub fn from_terminal() -> Self {
        // TODO: Implement terminal color detection
        // Could check TERM environment variable or use terminal queries
        Self::default()
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_theme_creation() {
        let theme = Theme::default();
        // Verify key colors are set
        assert_eq!(theme.running, Color::LightGreen);
        assert_eq!(theme.stopped, Color::DarkGray);
        assert_eq!(theme.error, Color::Red);
    }

    #[test]
    fn test_from_terminal_returns_valid_theme() {
        let theme = Theme::from_terminal();
        // Should return a valid theme (currently same as default)
        assert_eq!(theme.primary, Color::Cyan);
    }
}
