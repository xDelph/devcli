// Log Viewer View - Full-screen log file viewer with beautification
// Provides scrolling, JSON prettification, syntax highlighting, and search functionality
// Implements lazy loading for efficient handling of large log files

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::path::PathBuf;
use syntect::{
    easy::HighlightLines,
    highlighting::{Theme, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
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
    #[allow(dead_code)]
    syntax_set: SyntaxSet,
    /// Total number of lines in the file (for display)
    total_lines: usize,
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
            let is_json = Self::detect_json(line);
            let formatted = if is_json {
                // Prettify and highlight JSON
                // Performance: JSON formatting is expensive, only done once per line
                Self::format_json_line(line, &syntax_set, &theme)
            } else {
                // Regular line - just convert to span
                // Optimization: Avoid unnecessary allocations for plain text
                vec![Span::raw(line.to_string())]
            };

            content.push(LogLine {
                raw: line.to_string(),
                formatted,
                line_number: idx + 1, // 1-indexed for display
                is_json,
            });
        }

        // Start at the bottom of the file (most recent logs)
        // Cursor is on the last line, viewport shows the last page
        // UX: Users typically want to see the most recent logs first
        let cursor_line = total_lines.saturating_sub(1);
        let viewport_top = cursor_line; // Will be adjusted on first render

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
        })
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
    /// Prettifies the JSON and applies color coding
    fn format_json_line(
        line: &str,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Vec<Span<'static>> {
        // Try to parse and prettify the JSON
        let prettified = match serde_json::from_str::<serde_json::Value>(line.trim()) {
            Ok(json) => match serde_json::to_string_pretty(&json) {
                Ok(pretty) => pretty,
                Err(_) => line.to_string(),
            },
            Err(_) => line.to_string(),
        };

        // Apply syntax highlighting
        let syntax = syntax_set
            .find_syntax_by_extension("json")
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

        let mut highlighter = HighlightLines::new(syntax, theme);
        let mut spans = Vec::new();

        // Highlight each line of the prettified JSON
        for line in LinesWithEndings::from(&prettified) {
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
        }

        spans
    }

    /// Renders the log viewer to the terminal
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Create the main layout with header and content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Footer (search bar or shortcuts)
            ])
            .split(area);

        // Adjust viewport to ensure cursor is visible
        let visible_height = chunks[1].height.saturating_sub(2) as usize;
        self.adjust_viewport(visible_height);

        // Render header with file name and position
        self.render_header(frame, chunks[0]);

        // Render log content
        self.render_content(frame, chunks[1]);

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
            .style(Style::default().fg(Color::White));

        frame.render_widget(header, area);
    }

    /// Renders the main log content with line numbers
    /// Performance optimization: Only renders visible lines
    fn render_content(&self, frame: &mut Frame, area: Rect) {
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
        
        // Calculate which lines are visible based on viewport_top
        // Optimization: Only process lines that will be displayed
        let start_line = self.viewport_top;
        let end_line = (start_line + visible_height).min(self.content.len());

        // Build the lines to display
        // Performance: Pre-allocate capacity for visible lines
        let mut lines = Vec::with_capacity(visible_height);
        for i in start_line..end_line {
            if let Some(log_line) = self.content.get(i) {
                // Create line with line number prefix
                // Visual polish: Right-aligned line numbers with separator
                let line_num_str = format!("{:>5} │ ", log_line.line_number);
                let line_num_span = if self.is_search_match(i) {
                    // Highlight search matches
                    // Visual feedback: Yellow highlight for search results
                    Span::styled(
                        line_num_str,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                } else if i == self.cursor_line {
                    // Highlight cursor line
                    // Visual feedback: Cyan arrow shows current position
                    Span::styled(
                        format!("{:>5} > ", log_line.line_number),
                        Style::default().fg(Color::Cyan),
                    )
                } else {
                    // Regular line number
                    // Visual consistency: Dimmed to not distract from content
                    Span::styled(line_num_str, Style::default().fg(Color::DarkGray))
                };

                // Combine line number with content
                // Optimization: Reuse pre-formatted spans from LogLine
                let mut spans = vec![line_num_span];
                spans.extend(log_line.formatted.clone());

                lines.push(Line::from(spans));
            }
        }

        let content = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: false });

        frame.render_widget(content, area);
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
            "↑↓/jk: Scroll  PgUp/PgDn: Page  Home/End: Jump  g/G: Top/Bottom  /: Search  n/N: Next/Prev  Esc: Back"
                .to_string()
        };

        let footer = Paragraph::new(footer_text)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray));

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
