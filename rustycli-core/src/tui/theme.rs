// Theme management for the TUI
// Provides color schemes and visual styling inspired by GitUI
// Colors are chosen to work well in both light and dark terminals

use ratatui::style::Color;

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
}

impl Theme {
    /// Creates the default theme with GitUI-inspired colors
    /// This theme works well in most terminal environments
    /// Visual polish: Carefully chosen colors for optimal contrast and readability
    pub fn new() -> Self {
        Self {
            // Cyan for primary highlights - stands out without being harsh
            // Used for focused borders, selected tabs, and important UI elements
            primary: Color::Cyan,
            // Blue for secondary elements - complements primary color
            // Used for section headers and less prominent highlights
            secondary: Color::Blue,
            // Green for success states - universally recognized positive indicator
            success: Color::Green,
            // Yellow for warnings - draws attention without alarming
            warning: Color::Yellow,
            // Red for errors - clear danger signal
            error: Color::Red,
            // Bright green for running status - highly visible and positive
            // Optimization: Uses LightGreen for better visibility in dark terminals
            running: Color::LightGreen,
            // Dark gray for stopped status - subdued to indicate inactive state
            // Visual consistency: Muted color reduces visual noise
            stopped: Color::DarkGray,
            // Dark gray background for selected items - subtle but clear
            // Provides good contrast without being too bright
            selected_bg: Color::DarkGray,
            // Gray for borders - visible but not distracting
            // Visual polish: Keeps focus on content, not chrome
            border: Color::Gray,
            // White for primary text - maximum readability
            // Ensures text is crisp and easy to read
            text: Color::White,
            // Gray for dimmed text - de-emphasizes secondary info
            // Visual hierarchy: Helps users focus on important information
            text_dim: Color::Gray,
        }
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
