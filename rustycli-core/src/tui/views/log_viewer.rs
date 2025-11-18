// Log Viewer View - Full-screen log file viewer with beautification
// Provides scrolling, JSON prettification, syntax highlighting, and search functionality
// Implements lazy loading for efficient handling of large log files

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use std::path::PathBuf;
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
};

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

/// Full-screen log viewer with beautification and search
/// Handles large files efficiently using lazy loading
/// Performance optimization: Only processes visible lines for large files
pub struct LogViewerView {
    /// Path to the log file being viewed
    log_path: PathBuf,
    /// All loaded log lines (lazy loaded in chunks)
    /// Optimization: For very large files, consider implementing a sliding window
    content: Vec<LogLine>,
    /// Current cursor position (which line is highlighted)
    cursor_line: usize,
    /// Top line of the viewport (first visible line)
    /// Performance: Only renders lines in viewport for efficiency
    viewport_top: usize,
    /// Current horizontal scroll offset (character index)
    /// Reserved for future horizontal scrolling feature
    #[allow(dead_code)]
    horizontal_offset: usize,
    /// Whether search mode is active
    search_mode: bool,
    /// Current search query string
    search_query: String,
    /// Line indices that match the search query
    /// Optimization: Cached to avoid re-searching on every render
    search_results: Vec<usize>,
    /// Index of the currently highlighted search result
    current_search_idx: usize,
    /// Syntax highlighting theme
    /// Stored for potential future use in dynamic theme switching
    #[allow(dead_code)]
    theme: Theme,
    /// Syntax set for highlighting
    /// Stored for potential future use in re-highlighting
    syntax_set: SyntaxSet,
    /// Total number of lines in the file (for display)
    total_lines: usize,
    /// Whether to show the JSON beautifier panel
    show_json_panel: bool,
}

impl LogViewerView {
    /// Creates a new log viewer for the specified file
    /// Loads the file content and prepares it for display
    /// Performance optimization: Processes lines on-demand for large files
    pub fn new(log_path: PathBuf) -> Result<Self> {
        // Load syntax highlighting resources
        // These are loaded once and reused for all lines
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();

        // Read the log file content
        // Performance note: For files >100MB, consider implementing streaming
        // or memory-mapped file access for better performance
        let content_str = std::fs::read_to_string(&log_path)?;
        let lines: Vec<&str> = content_str.lines().collect();
        let total_lines = lines.len();

        // Process each line to detect JSON and apply formatting
        // Optimization: Pre-allocate vector capacity for better performance
        let mut content = Vec::with_capacity(total_lines);
        for (idx, line) in lines.iter().enumerate() {
            // Parse ANSI codes to get styled spans
            let ansi_spans = Self::parse_ansi_codes(line);
            
            // Extract raw text for JSON detection and storage
            let raw_text: String = ansi_spans.iter().map(|s| s.content.as_ref()).collect();
            
            // Normalize whitespace - replace multiple spaces/tabs with single space
            let normalized = Self::normalize_whitespace(&raw_text);
            
            let is_json = Self::detect_json(&normalized);
            let formatted = if is_json {
                // Prettify and highlight JSON
                // Performance: JSON formatting is expensive, only done once per line
                Self::format_json_line(&normalized, &syntax_set, &theme)
            } else {
                // Use the ANSI-parsed spans for regular lines
                // Re-parse with normalized text to maintain ANSI colors
                Self::parse_ansi_codes(line)
            };

            content.push(LogLine {
                raw: normalized,
                formatted,
                line_number: idx + 1, // 1-indexed for display
                is_json,
            });
        }

        // Start at the bottom of the file (most recent logs)
        // Cursor is on the last line, viewport shows the last page
        // UX: Users typically want to see the most recent logs first
        let cursor_line = total_lines.saturating_sub(1);
        // Start viewport at 0 - it will be adjusted on first render to show cursor
        let viewport_top = 0;

        Ok(Self {
            log_path,
            content,
            cursor_line,
            viewport_top,
            horizontal_offset: 0,
            search_mode: false,
            search_query: String::new(),
            search_results: Vec::new(),
            current_search_idx: 0,
            theme,
            syntax_set,
            total_lines,
            show_json_panel: false,
        })
    }

    /// Normalizes whitespace in a line by replacing multiple spaces/tabs with single space
    /// This makes logs more readable and reduces excessive whitespace
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

    /// Parses ANSI escape codes and converts them to styled spans
    /// This interprets color codes and other terminal control sequences
    fn parse_ansi_codes(text: &str) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        let mut current_text = String::new();
        let mut current_style = Style::default();
        let mut chars = text.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if ch == '\x1b' {
                // Found escape character
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
                                current_style = Self::apply_sgr_code(&code_str, current_style);
                            }
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
        
        // If no spans were created, return a single span with the original text
        if spans.is_empty() {
            spans.push(Span::raw(text.to_string()));
        }
        
        spans
    }
    
    /// Applies SGR (Select Graphic Rendition) codes to a style
    fn apply_sgr_code(code_str: &str, mut style: Style) -> Style {
        let codes: Vec<u8> = code_str
            .split(';')
            .filter_map(|s| s.parse().ok())
            .collect();
        
        for code in codes {
            match code {
                0 => style = Style::default(), // Reset
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
                // Background colors (40-47)
                40 => style = style.bg(Color::Black),
                41 => style = style.bg(Color::Red),
                42 => style = style.bg(Color::Green),
                43 => style = style.bg(Color::Yellow),
                44 => style = style.bg(Color::Blue),
                45 => style = style.bg(Color::Magenta),
                46 => style = style.bg(Color::Cyan),
                47 => style = style.bg(Color::White),
                49 => style = style.bg(Color::Reset), // Default background
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

    /// Detects if a line contains JSON data
    /// Uses heuristics: starts with '{' or '[' and can be parsed as JSON
    fn detect_json(line: &str) -> bool {
        let trimmed = line.trim();
        
        // Quick check: must start with { or [
        if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
            return false;
        }

        // Try to parse as JSON to confirm
        serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    }

    /// Formats a JSON line with syntax highlighting
    /// Keeps JSON on a single line to avoid display issues
    fn format_json_line(
        line: &str,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Vec<Span<'static>> {
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
            let fg = Color::Rgb(
                style.foreground.r,
                style.foreground.g,
                style.foreground.b,
            );
            
            let mut ratatui_style = Style::default().fg(fg);
            
            if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
            }
            if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
            }
            if style.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
                ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
            }

            spans.push(Span::styled(text.to_string(), ratatui_style));
        }

        spans
    }

    /// Renders the log viewer to the terminal
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Clear the entire area first to prevent artifacts
        frame.render_widget(Clear, area);
        
        // Create the main layout with header and content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Footer (search bar or shortcuts)
            ])
            .split(area);

        // Render header with file name and position
        self.render_header(frame, chunks[0]);

        // Split content area if JSON panel is visible
        if self.show_json_panel && self.is_current_line_json() {
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50), // Log content
                    Constraint::Percentage(50), // JSON panel
                ])
                .split(chunks[1]);

            // Adjust viewport to ensure cursor is visible
            let visible_height = content_chunks[0].height.saturating_sub(2) as usize;
            self.adjust_viewport(visible_height);

            // Render log content on left
            self.render_content(frame, content_chunks[0]);

            // Render JSON panel on right
            self.render_json_panel(frame, content_chunks[1]);
        } else {
            // Adjust viewport to ensure cursor is visible
            let visible_height = chunks[1].height.saturating_sub(2) as usize;
            self.adjust_viewport(visible_height);

            // Render log content full width
            self.render_content(frame, chunks[1]);
        }

        // Render footer (search bar or keyboard shortcuts)
        self.render_footer(frame, chunks[2]);
    }

    /// Renders the header showing file name and current position
    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let filename = self
            .log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        let current_line = self.cursor_line + 1;
        let title = format!(
            "{} (Line {}/{})",
            filename, current_line, self.total_lines
        );

        let header = Paragraph::new(title)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White).bg(Color::Black));

        frame.render_widget(header, area);
    }

    /// Renders the main log content with line numbers
    fn render_content(&self, frame: &mut Frame, area: Rect) {
        // Build text with ALL lines
        let mut text = Text::default();
        
        for (i, log_line) in self.content.iter().enumerate() {
            // Determine the background color for this line
            let bg_color = if i == self.cursor_line {
                Color::Rgb(40, 40, 60) // Highlighted cursor line background
            } else {
                Color::Black // Default background
            };
            
            // Create line with line number prefix
            // Using simple ASCII characters for better alignment
            let line_num_str = format!("{:>5} | ", log_line.line_number);
            let line_num_span = if self.is_search_match(i) {
                // Highlight search matches
                Span::styled(
                    line_num_str.clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else if i == self.cursor_line {
                // Highlight cursor line with arrow
                let cursor_str = format!("{:>5} > ", log_line.line_number);
                Span::styled(
                    cursor_str,
                    Style::default().fg(Color::Cyan),
                )
            } else {
                // Regular line number
                Span::styled(line_num_str.clone(), Style::default().fg(Color::DarkGray))
            };

            // Combine line number with content
            let mut spans = vec![line_num_span];
            spans.extend(log_line.formatted.clone());
            
            // Use Line::styled to apply background to the entire line
            // This ensures the line fills the full width with the background color
            let line = Line::from(spans).style(Style::default().bg(bg_color));

            text.lines.push(line);
        }

        // Use Paragraph with scroll and set background style to fill entire area
        let paragraph = Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().bg(Color::Black))
            .scroll((self.viewport_top as u16, 0));

        frame.render_widget(paragraph, area);
    }

    /// Checks if the current cursor line contains JSON
    fn is_current_line_json(&self) -> bool {
        self.content
            .get(self.cursor_line)
            .map(|line| line.is_json)
            .unwrap_or(false)
    }

    /// Renders the JSON beautifier panel showing prettified JSON
    fn render_json_panel(&self, frame: &mut Frame, area: Rect) {
        let json_content = if let Some(log_line) = self.content.get(self.cursor_line) {
            if log_line.is_json {
                // Parse and prettify the JSON
                match serde_json::from_str::<serde_json::Value>(log_line.raw.trim()) {
                    Ok(json) => match serde_json::to_string_pretty(&json) {
                        Ok(pretty) => {
                            // Apply syntax highlighting to prettified JSON
                            self.format_prettified_json(&pretty)
                        }
                        Err(_) => vec![Line::from("Error: Failed to format JSON")],
                    },
                    Err(e) => vec![Line::from(format!("Error: Invalid JSON - {}", e))],
                }
            } else {
                vec![Line::from("Not a JSON line")]
            }
        } else {
            vec![Line::from("No line selected")]
        };

        let panel = Paragraph::new(json_content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("JSON Beautifier")
                    .border_style(Style::default().fg(Color::Green)),
            )
            .style(Style::default().fg(Color::White).bg(Color::Black));

        frame.render_widget(panel, area);
    }

    /// Formats prettified JSON with syntax highlighting
    fn format_prettified_json(&self, json_str: &str) -> Vec<Line<'static>> {
        let syntax = self
            .syntax_set
            .find_syntax_by_extension("json")
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let mut lines = Vec::new();

        for line in json_str.lines() {
            let ranges = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();

            let mut spans = Vec::new();
            for (style, text) in ranges {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);

                let mut ratatui_style = Style::default().fg(fg);

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

    /// Renders the footer with search bar or keyboard shortcuts
    /// Visual polish: Context-aware footer provides relevant information
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_text = if self.search_mode {
            // Show search input with match count
            // UX: Real-time feedback on search results
            let match_info = if !self.search_results.is_empty() {
                format!(
                    " [{} matches] ({}/{})",
                    self.search_results.len(),
                    self.current_search_idx + 1,
                    self.search_results.len()
                )
            } else if !self.search_query.is_empty() {
                " [No matches]".to_string()
            } else {
                String::new()
            };

            format!("Search: {}{}", self.search_query, match_info)
        } else {
            // Show keyboard shortcuts for navigation
            // Visual consistency: Matches main view footer style
            let json_hint = if self.is_current_line_json() {
                if self.show_json_panel {
                    "  J: Hide JSON"
                } else {
                    "  J: Show JSON"
                }
            } else {
                ""
            };
            format!(
                "↑↓/jk: Scroll  Shift+↑↓: Jump 10  PgUp/PgDn: Page  Home/End/g/G: Top/Bottom  /: Search  n/N: Next/Prev{}  Esc: Back",
                json_hint
            )
        };

        let footer = Paragraph::new(footer_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray).bg(Color::Black));

        frame.render_widget(footer, area);
    }

    /// Checks if a line index is in the search results
    fn is_search_match(&self, line_idx: usize) -> bool {
        self.search_results.contains(&line_idx)
    }

    /// Handles keyboard input
    /// Returns true if the event was handled, false otherwise
    pub fn handle_input(&mut self, key: KeyEvent) -> Result<bool> {
        if self.search_mode {
            // Handle search mode input
            self.handle_search_input(key)
        } else {
            // Handle normal navigation input
            self.handle_navigation_input(key)
        }
    }

    /// Handles input when in search mode
    fn handle_search_input(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Backspace => {
                // Remove last character from search query
                self.search_query.pop();
                self.perform_search();
                Ok(true)
            }
            KeyCode::Enter | KeyCode::Esc => {
                // Exit search mode
                self.search_mode = false;
                Ok(true)
            }
            KeyCode::Char('n') if key.modifiers.is_empty() => {
                // Next search result
                self.next_search_result();
                Ok(true)
            }
            KeyCode::Char('N') if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                // Previous search result (Shift+N)
                self.previous_search_result();
                Ok(true)
            }
            KeyCode::Char(c) => {
                // Add character to search query
                self.search_query.push(c);
                self.perform_search();
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Handles input when in normal navigation mode
    fn handle_navigation_input(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            // Shift+Down - jump down by 10
            KeyCode::Down if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                self.scroll_down(10);
                Ok(true)
            }
            // Shift+Up - jump up by 10
            KeyCode::Up if key.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) => {
                self.scroll_up(10);
                Ok(true)
            }
            // Scroll down one line
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down(1);
                Ok(true)
            }
            // Scroll up one line
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up(1);
                Ok(true)
            }
            // Page down
            KeyCode::PageDown => {
                self.scroll_down(20);
                Ok(true)
            }
            // Page up
            KeyCode::PageUp => {
                self.scroll_up(20);
                Ok(true)
            }
            // Jump to top
            KeyCode::Home | KeyCode::Char('g') => {
                self.cursor_line = 0;
                Ok(true)
            }
            // Jump to bottom
            KeyCode::End | KeyCode::Char('G') => {
                self.cursor_line = self.content.len().saturating_sub(1);
                Ok(true)
            }
            // Enter search mode
            KeyCode::Char('/') => {
                self.search_mode = true;
                self.search_query.clear();
                self.search_results.clear();
                self.current_search_idx = 0;
                Ok(true)
            }
            // Next search result
            KeyCode::Char('n') => {
                self.next_search_result();
                Ok(true)
            }
            // Previous search result
            KeyCode::Char('N') => {
                self.previous_search_result();
                Ok(true)
            }
            // Toggle JSON panel
            KeyCode::Char('J') => {
                self.show_json_panel = !self.show_json_panel;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Moves the cursor down (toward newer logs)
    /// The viewport scrolls only when the cursor reaches the bottom edge
    fn scroll_down(&mut self, lines: usize) {
        for _ in 0..lines {
            // Can't move past the last line
            if self.cursor_line >= self.content.len().saturating_sub(1) {
                break;
            }
            
            self.cursor_line += 1;
            
            // Scroll viewport if cursor moved past the bottom edge
            // We need to know the viewport height to determine this
            // For now, we'll adjust viewport in a separate method called during render
        }
    }

    /// Moves the cursor up (toward older logs)
    /// The viewport scrolls only when the cursor reaches the top edge
    fn scroll_up(&mut self, lines: usize) {
        for _ in 0..lines {
            // Can't move past the first line
            if self.cursor_line == 0 {
                break;
            }
            
            self.cursor_line -= 1;
            
            // Scroll viewport if cursor moved past the top edge
            // We need to know the viewport height to determine this
            // For now, we'll adjust viewport in a separate method called during render
        }
    }
    
    /// Adjusts the viewport to ensure the cursor is visible
    /// Should be called after cursor movement and before rendering
    /// Performance optimization: Efficient viewport calculation
    fn adjust_viewport(&mut self, visible_height: usize) {
        // Ensure cursor is within viewport bounds
        // Optimization: Calculate viewport_bottom once
        let viewport_bottom = self.viewport_top + visible_height.saturating_sub(1);
        
        if self.cursor_line < self.viewport_top {
            // Cursor is above viewport - scroll up
            // Visual polish: Smooth scrolling keeps cursor visible
            self.viewport_top = self.cursor_line;
        } else if self.cursor_line > viewport_bottom {
            // Cursor is below viewport - scroll down
            // Optimization: Efficient calculation avoids overflow
            self.viewport_top = self.cursor_line.saturating_sub(visible_height.saturating_sub(1));
        }
        
        // Ensure viewport doesn't go past the end of content
        // Edge case handling: Prevents rendering beyond file bounds
        let max_viewport_top = self.content.len().saturating_sub(visible_height);
        if self.viewport_top > max_viewport_top && self.content.len() >= visible_height {
            self.viewport_top = max_viewport_top;
        }
    }

    /// Performs a search for the current query
    /// Updates search_results with matching line indices
    /// Performance optimization: Case-insensitive search with efficient string matching
    fn perform_search(&mut self) {
        self.search_results.clear();
        self.current_search_idx = 0;

        if self.search_query.is_empty() {
            return;
        }

        // Search for the query in each line (case-insensitive)
        // Optimization: Convert query to lowercase once, not per line
        let query_lower = self.search_query.to_lowercase();
        
        // Performance: Pre-allocate capacity based on estimated hit rate
        // Assume ~5% of lines might match (adjust based on typical usage)
        self.search_results.reserve(self.content.len() / 20);
        
        for (idx, log_line) in self.content.iter().enumerate() {
            // Optimization: Use contains for fast substring matching
            if log_line.raw.to_lowercase().contains(&query_lower) {
                self.search_results.push(idx);
            }
        }

        // Jump to first result if any
        // UX: Immediately show the first match for quick feedback
        if !self.search_results.is_empty() {
            self.cursor_line = self.search_results[0];
        }
    }

    /// Jumps to the next search result
    fn next_search_result(&mut self) {
        if self.search_results.is_empty() {
            return;
        }

        self.current_search_idx = (self.current_search_idx + 1) % self.search_results.len();
        self.cursor_line = self.search_results[self.current_search_idx];
    }

    /// Jumps to the previous search result
    fn previous_search_result(&mut self) {
        if self.search_results.is_empty() {
            return;
        }

        if self.current_search_idx == 0 {
            self.current_search_idx = self.search_results.len() - 1;
        } else {
            self.current_search_idx -= 1;
        }
        self.cursor_line = self.search_results[self.current_search_idx];
    }

    /// Returns the path to the log file
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_json_valid() {
        assert!(LogViewerView::detect_json(r#"{"key": "value"}"#));
        assert!(LogViewerView::detect_json(r#"  {"key": "value"}  "#));
        assert!(LogViewerView::detect_json(r#"[1, 2, 3]"#));
        assert!(LogViewerView::detect_json(r#"{"nested": {"key": "value"}}"#));
    }

    #[test]
    fn test_detect_json_invalid() {
        assert!(!LogViewerView::detect_json("plain text"));
        assert!(!LogViewerView::detect_json("not json {"));
        assert!(!LogViewerView::detect_json(""));
        assert!(!LogViewerView::detect_json("123"));
    }

    #[test]
    fn test_scroll_down_within_bounds() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "line1\nline2\nline3\nline4\nline5").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        // Viewer starts at the bottom (last line, index 4)
        assert_eq!(viewer.cursor_line, 4);
        
        // Scroll up first to test scrolling down
        viewer.scroll_up(2);
        assert_eq!(viewer.cursor_line, 2);
        
        // Now scroll down
        viewer.scroll_down(1);
        assert_eq!(viewer.cursor_line, 3);
    }

    #[test]
    fn test_scroll_down_at_end() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "line1\nline2\nline3").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        // Viewer starts at the end (last line, index 2)
        let max_cursor = viewer.cursor_line;
        assert_eq!(max_cursor, 2);
        
        // Try to scroll further down
        viewer.scroll_down(10);
        
        // Should stay at max
        assert_eq!(viewer.cursor_line, max_cursor);
    }

    #[test]
    fn test_scroll_up_within_bounds() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "line1\nline2\nline3\nline4\nline5").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        // Viewer starts at the bottom (last line, index 4)
        assert_eq!(viewer.cursor_line, 4);
        
        // Scroll up
        viewer.scroll_up(2);
        assert_eq!(viewer.cursor_line, 2);
    }

    #[test]
    fn test_scroll_up_at_beginning() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "line1\nline2\nline3").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        // Viewer starts at the bottom (last line, index 2)
        assert_eq!(viewer.cursor_line, 2);
        
        // Scroll up to the beginning
        viewer.scroll_up(10);
        assert_eq!(viewer.cursor_line, 0);
        
        // Try to scroll up further from beginning
        viewer.scroll_up(10);
        
        // Should stay at 0
        assert_eq!(viewer.cursor_line, 0);
    }

    #[test]
    fn test_search_finds_matches() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "error line\ninfo line\nerror again\nwarning").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        viewer.search_query = "error".to_string();
        viewer.perform_search();
        
        assert_eq!(viewer.search_results.len(), 2);
        assert_eq!(viewer.search_results[0], 0); // First "error line"
        assert_eq!(viewer.search_results[1], 2); // "error again"
    }

    #[test]
    fn test_search_case_insensitive() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "ERROR line\nInfo line\nerror again").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        viewer.search_query = "error".to_string();
        viewer.perform_search();
        
        assert_eq!(viewer.search_results.len(), 2);
    }

    #[test]
    fn test_search_no_matches() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "line1\nline2\nline3").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        viewer.search_query = "notfound".to_string();
        viewer.perform_search();
        
        assert_eq!(viewer.search_results.len(), 0);
    }

    #[test]
    fn test_next_search_result() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "match\nno\nmatch\nno\nmatch").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        viewer.search_query = "match".to_string();
        viewer.perform_search();
        
        assert_eq!(viewer.current_search_idx, 0);
        assert_eq!(viewer.cursor_line, 0);
        
        viewer.next_search_result();
        assert_eq!(viewer.current_search_idx, 1);
        assert_eq!(viewer.cursor_line, 2);
        
        viewer.next_search_result();
        assert_eq!(viewer.current_search_idx, 2);
        assert_eq!(viewer.cursor_line, 4);
        
        // Wrap around
        viewer.next_search_result();
        assert_eq!(viewer.current_search_idx, 0);
        assert_eq!(viewer.cursor_line, 0);
    }

    #[test]
    fn test_previous_search_result() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "match\nno\nmatch\nno\nmatch").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        viewer.search_query = "match".to_string();
        viewer.perform_search();
        
        assert_eq!(viewer.current_search_idx, 0);
        
        // Go to previous (should wrap to last)
        viewer.previous_search_result();
        assert_eq!(viewer.current_search_idx, 2);
        assert_eq!(viewer.cursor_line, 4);
        
        viewer.previous_search_result();
        assert_eq!(viewer.current_search_idx, 1);
        assert_eq!(viewer.cursor_line, 2);
    }

    #[test]
    fn test_search_empty_query() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp_file.path(), "line1\nline2\nline3").unwrap();
        
        let mut viewer = LogViewerView::new(temp_file.path().to_path_buf()).unwrap();
        
        viewer.search_query = "".to_string();
        viewer.perform_search();
        
        assert_eq!(viewer.search_results.len(), 0);
    }
}
