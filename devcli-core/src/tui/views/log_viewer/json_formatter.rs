use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use syntect::{easy::HighlightLines, highlighting::Theme, parsing::SyntaxSet};

pub struct JsonFormatter;

impl JsonFormatter {
    /// Formats a JSON line with syntax highlighting
    /// Keeps JSON on a single line to avoid display issues
    pub fn format_json_line(
        line: &str,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Vec<Span<'static>> {
        crate::debug!("[JsonFormatter] format_json_line called");
        // Don't prettify - keep on single line to avoid wrapping issues
        // Just apply syntax highlighting to the original line
        let syntax = syntax_set
            .find_syntax_by_extension("json")
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut spans = Vec::new();

        // Highlight the single line
        let ranges = highlighter
            .highlight_line(line, syntax_set)
            .unwrap_or_default();

        for (style, text) in ranges {
            // Convert syntect style to ratatui style
            let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);

            // IMPORTANT: Use black background instead of syntect theme's gray background
            let mut ratatui_style = Style::default().fg(fg).bg(Color::Rgb(0, 0, 0));

            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::BOLD)
            {
                ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
            }
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::ITALIC)
            {
                ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
            }
            if style
                .font_style
                .contains(syntect::highlighting::FontStyle::UNDERLINE)
            {
                ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
            }

            spans.push(Span::styled(text.to_string(), ratatui_style));
        }

        spans
    }

    /// Formats prettified JSON with syntax highlighting
    pub fn format_prettified_json(
        json_str: &str,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Vec<Line<'static>> {
        let syntax = syntax_set
            .find_syntax_by_extension("json")
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut lines = Vec::new();

        for line in json_str.lines() {
            let ranges = highlighter
                .highlight_line(line, syntax_set)
                .unwrap_or_default();

            let mut spans = Vec::new();
            for (style, text) in ranges {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);

                // IMPORTANT: Use black background instead of syntect theme's gray background
                let mut ratatui_style = Style::default().fg(fg).bg(Color::Rgb(0, 0, 0));

                if style
                    .font_style
                    .contains(syntect::highlighting::FontStyle::BOLD)
                {
                    ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                }
                if style
                    .font_style
                    .contains(syntect::highlighting::FontStyle::ITALIC)
                {
                    ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                }
                if style
                    .font_style
                    .contains(syntect::highlighting::FontStyle::UNDERLINE)
                {
                    ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
                }

                spans.push(Span::styled(text.to_string(), ratatui_style));
            }

            lines.push(Line::from(spans));
        }

        lines
    }
}
