//! ANSI escape code parser for TUI
//!
//! Handles parsing of ANSI escape codes (colors, styles) and converting them
//! to Ratatui Spans for display. This is a comprehensive parser that supports
//! foreground colors, background colors, and various text modifiers.

use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

/// Parses ANSI escape codes and converts them to styled spans
///
/// This function iterates through the input string, identifying ANSI escape sequences
/// (starting with `\x1b[`). It separates text content from style instructions and
/// maintains proper background colors for consistent display.
///
/// # Arguments
/// * `text` - The input string containing ANSI escape codes
///
/// # Returns
/// A vector of `Span`s, where each span contains a segment of text with its associated style.
/// If no ANSI codes are found, returns a single span with black background to prevent
/// terminal default gray from showing through.
pub fn parse_ansi_codes(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    // IMPORTANT: Start with black background to prevent terminal default gray
    let mut current_style = Style::default().bg(Color::Rgb(0, 0, 0));
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        // Check for start of ANSI escape sequence (ESC char)
        if ch == '\x1b' {
            // Check for Control Sequence Introducer '['
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['

                // Save current text as a span if any
                if !current_text.is_empty() {
                    spans.push(Span::styled(current_text.clone(), current_style));
                    current_text.clear();
                }

                // Parse the escape sequence
                let mut code_str = String::new();
                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    if next_ch.is_ascii_alphabetic() {
                        // End of escape sequence
                        if next_ch == 'm' {
                            // SGR (Select Graphic Rendition) - color/style codes
                            current_style = apply_sgr_code(&code_str, current_style);
                        }
                        // Other codes (like cursor movement 'A', 'B', etc.) are ignored
                        // as we only care about styling for display purposes
                        break;
                    } else {
                        code_str.push(next_ch);
                    }
                }
            }
        } else {
            current_text.push(ch);
        }
    }

    // Add remaining text
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }

    // If no spans were created, return a single span with black background
    if spans.is_empty() {
        spans.push(Span::styled(
            text.to_string(),
            Style::default().bg(Color::Rgb(0, 0, 0)),
        ));
    }

    spans
}

/// Applies SGR (Select Graphic Rendition) codes to a style
///
/// Parses semicolon-separated codes (e.g., "31;1") and updates the style accordingly.
/// Supports comprehensive ANSI styling including:
/// - Standard and bright foreground colors (30-37, 90-97)
/// - Standard and bright background colors (40-47, 100-107)
/// - Text modifiers (bold, dim, italic, underlined, reversed, crossed out)
/// - Reset codes for individual modifiers
///
/// # Arguments
/// * `code_str` - The string containing the SGR parameters (e.g., "31;1")
/// * `style` - The current style to update
///
/// # Returns
/// The updated `Style` object with proper black background maintained.
fn apply_sgr_code(code_str: &str, mut style: Style) -> Style {
    // Split codes by ';' and parse into integers
    let codes: Vec<u8> = code_str.split(';').filter_map(|s| s.parse().ok()).collect();

    for code in codes {
        match code {
            // Reset - IMPORTANT: Keep black background to prevent gray terminal default
            0 => style = Style::default().bg(Color::Rgb(0, 0, 0)),
            1 => style = style.add_modifier(Modifier::BOLD),
            2 => style = style.add_modifier(Modifier::DIM),
            3 => style = style.add_modifier(Modifier::ITALIC),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            7 => style = style.add_modifier(Modifier::REVERSED),
            9 => style = style.add_modifier(Modifier::CROSSED_OUT),
            22 => style = style.remove_modifier(Modifier::BOLD | Modifier::DIM),
            23 => style = style.remove_modifier(Modifier::ITALIC),
            24 => style = style.remove_modifier(Modifier::UNDERLINED),
            27 => style = style.remove_modifier(Modifier::REVERSED),
            29 => style = style.remove_modifier(Modifier::CROSSED_OUT),
            // Foreground colors (30-37)
            30 => style = style.fg(Color::Black),
            31 => style = style.fg(Color::Red),
            32 => style = style.fg(Color::Green),
            33 => style = style.fg(Color::Yellow),
            34 => style = style.fg(Color::Blue),
            35 => style = style.fg(Color::Magenta),
            36 => style = style.fg(Color::Cyan),
            37 => style = style.fg(Color::White),
            39 => style = style.fg(Color::Reset), // Default foreground
            // Bright foreground colors (90-97)
            90 => style = style.fg(Color::DarkGray),
            91 => style = style.fg(Color::LightRed),
            92 => style = style.fg(Color::LightGreen),
            93 => style = style.fg(Color::LightYellow),
            94 => style = style.fg(Color::LightBlue),
            95 => style = style.fg(Color::LightMagenta),
            96 => style = style.fg(Color::LightCyan),
            97 => style = style.fg(Color::Gray),
            // Background colors (40-47) - IMPORTANT: Use Black instead of Reset
            40 => style = style.bg(Color::Rgb(0, 0, 0)),
            41 => style = style.bg(Color::Red),
            42 => style = style.bg(Color::Green),
            43 => style = style.bg(Color::Yellow),
            44 => style = style.bg(Color::Blue),
            45 => style = style.bg(Color::Magenta),
            46 => style = style.bg(Color::Cyan),
            47 => style = style.bg(Color::White),
            49 => style = style.bg(Color::Rgb(0, 0, 0)), // Default background - use Black not Reset
            // Bright background colors (100-107)
            100 => style = style.bg(Color::DarkGray),
            101 => style = style.bg(Color::LightRed),
            102 => style = style.bg(Color::LightGreen),
            103 => style = style.bg(Color::LightYellow),
            104 => style = style.bg(Color::LightBlue),
            105 => style = style.bg(Color::LightMagenta),
            106 => style = style.bg(Color::LightCyan),
            107 => style = style.bg(Color::Gray),
            _ => {} // Ignore unknown codes
        }
    }

    style
}
