//! ANSI escape code parser for TUI
//!
//! Handles parsing of ANSI escape codes (colors, styles) and converting them
//! to Ratatui Spans for display.

use ratatui::{
    style::{Modifier, Style},
    text::Span,
};

/// Parses ANSI escape codes and converts them to styled spans
///
/// This function iterates through the input string, identifying ANSI escape sequences
/// (starting with `\x1b[`). It separates text content from style instructions.
///
/// # Arguments
/// * `text` - The input string containing ANSI escape codes
///
/// # Returns
/// A vector of `Span`s, where each span contains a segment of text with its associated style.
/// If no ANSI codes are found, returns a single span with the original text.
pub fn parse_ansi_codes(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut current_style = Style::default();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        // Check for start of ANSI escape sequence (ESC char)
        if ch == '\x1b' {
            // Check for Control Sequence Introducer '['
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['

                // Push any accumulated text with the *previous* style
                if !current_text.is_empty() {
                    spans.push(Span::styled(current_text.clone(), current_style));
                    current_text.clear();
                }

                // Parse the escape code parameters (e.g., "31;1" in "\x1b[31;1m")
                let mut code_str = String::new();
                while let Some(&next_ch) = chars.peek() {
                    chars.next();
                    if next_ch.is_ascii_alphabetic() {
                        // 'm' indicates SGR (Select Graphic Rendition) - style/color change
                        if next_ch == 'm' {
                            current_style = apply_sgr_code(&code_str, current_style);
                        }
                        // Other codes (like cursor movement 'A', 'B', etc.) are ignored for now
                        // as we only care about styling for the log viewer
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

    // Push any remaining text
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style));
    }

    // Fallback for empty result (shouldn't happen if text wasn't empty)
    if spans.is_empty() {
        spans.push(Span::raw(text.to_string()));
    }

    spans
}

/// Applies SGR (Select Graphic Rendition) codes to a style
///
/// Parses semicolon-separated codes (e.g., "31;1") and updates the style accordingly.
/// Supports standard ANSI colors (30-37), bright colors (90-97), and modifiers (bold, dim, underline).
///
/// # Arguments
/// * `code_str` - The string containing the SGR parameters (e.g., "31;1")
/// * `style` - The current style to update
///
/// # Returns
/// The updated `Style` object.
fn apply_sgr_code(code_str: &str, mut style: Style) -> Style {
    use ratatui::style::Color;

    // Split codes by ';' and parse into integers
    let codes: Vec<u8> = code_str.split(';').filter_map(|s| s.parse().ok()).collect();

    for code in codes {
        match code {
            0 => style = Style::default(), // Reset
            1 => style = style.add_modifier(Modifier::BOLD),
            2 => style = style.add_modifier(Modifier::DIM),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            // Foreground colors
            30 => style = style.fg(Color::Black),
            31 => style = style.fg(Color::Red),
            32 => style = style.fg(Color::Green),
            33 => style = style.fg(Color::Yellow),
            34 => style = style.fg(Color::Blue),
            35 => style = style.fg(Color::Magenta),
            36 => style = style.fg(Color::Cyan),
            37 => style = style.fg(Color::White),
            39 => style = style.fg(Color::Reset),
            // Bright foreground colors
            90 => style = style.fg(Color::DarkGray),
            91 => style = style.fg(Color::LightRed),
            92 => style = style.fg(Color::LightGreen),
            93 => style = style.fg(Color::LightYellow),
            94 => style = style.fg(Color::LightBlue),
            95 => style = style.fg(Color::LightMagenta),
            96 => style = style.fg(Color::LightCyan),
            97 => style = style.fg(Color::Gray),
            _ => {} // Ignore unsupported codes
        }
    }

    style
}
