use ratatui::text::Span;
use syntect::{highlighting::Theme, parsing::SyntaxSet};

use super::json_formatter::JsonFormatter;
use crate::tui::utils::ansi;

/// Represents a single line in the log file with formatting information
#[derive(Debug, Clone)]
pub struct LogLine {
    /// The raw text content of the line
    pub raw: String,
    /// The formatted spans for rendering (with syntax highlighting)
    pub formatted: Vec<Span<'static>>,
    /// Line number (1-indexed for display)
    pub line_number: usize,
    /// Whether this line contains JSON data
    pub is_json: bool,
}

pub struct FileLoader;

impl FileLoader {
    /// Parse `text` into display lines numbered from `first_line_number`.
    pub fn parse_lines(
        text: &str,
        first_line_number: usize,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Vec<LogLine> {
        text.lines()
            .enumerate()
            .map(|(idx, line)| Self::parse_line(line, first_line_number + idx, syntax_set, theme))
            .collect()
    }

    fn parse_line(
        line: &str,
        line_number: usize,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> LogLine {
        // Parse ANSI codes to get styled spans
        let ansi_spans = ansi::parse_ansi_codes(line);

        // Extract raw text for JSON detection and storage
        let raw_text: String = ansi_spans.iter().map(|s| s.content.as_ref()).collect();

        // Normalize whitespace - replace multiple spaces/tabs with single space
        let normalized = Self::normalize_whitespace(&raw_text);

        let is_json = Self::detect_json(&normalized);
        let formatted = if is_json {
            // Prettify and highlight JSON
            JsonFormatter::format_json_line(&normalized, syntax_set, theme)
        } else {
            // Use the ANSI-parsed spans for regular lines
            ansi_spans
        };

        LogLine {
            raw: normalized,
            formatted,
            line_number, // 1-indexed for display
            is_json,
        }
    }

    /// Normalizes whitespace in a line by replacing multiple spaces/tabs with single space
    fn normalize_whitespace(line: &str) -> String {
        let mut result = String::with_capacity(line.len());
        let mut prev_was_space = false;

        for ch in line.chars() {
            if ch == ' ' || ch == '\t' {
                if !prev_was_space {
                    result.push(' ');
                    prev_was_space = true;
                }
            } else {
                result.push(ch);
                prev_was_space = false;
            }
        }

        result
    }

    /// Detects if a line contains JSON data
    pub fn detect_json(line: &str) -> bool {
        let trimmed = line.trim();

        // Quick check: must start with { or [
        if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
            return false;
        }

        // Try to parse as JSON to confirm
        serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    }
}
