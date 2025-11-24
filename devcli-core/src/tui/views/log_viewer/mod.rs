// Log Viewer View - Full-screen log file viewer with beautification
// Provides scrolling, JSON prettification, syntax highlighting, and search functionality
// Implements lazy loading for efficient handling of large log files

pub mod ansi_parser;
pub mod file_loader;
pub mod json_formatter;
pub mod search;
pub mod syntax_highlighter;
pub mod viewport;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::path::PathBuf;

use self::{
    file_loader::{FileLoader, LogLine},
    json_formatter::JsonFormatter,
    search::SearchState,
    syntax_highlighter::SyntaxHighlighter,
    viewport::ViewportState,
};

/// Full-screen log viewer with beautification and search
/// Handles large files efficiently using lazy loading
/// Performance optimization: Only processes visible lines for large files
pub struct LogViewerView {
    /// Path to the log file being viewed
    log_path: PathBuf,
    /// All loaded log lines (lazy loaded in chunks)
    content: Vec<LogLine>,
    /// Viewport state (cursor, scroll position)
    viewport: ViewportState,
    /// Search state (query, results)
    search: SearchState,
    /// Syntax highlighting resources
    highlighter: SyntaxHighlighter,
    /// Total number of lines in the file (for display)
    total_lines: usize,
    /// Whether to show the JSON beautifier panel
    show_json_panel: bool,
    /// Last file size to detect changes
    last_file_size: u64,
}

impl LogViewerView {
    /// Creates a new log viewer for the specified file
    /// Loads the file content and prepares it for display
    pub fn new(log_path: PathBuf) -> Result<Self> {
        crate::debug!(
            "[LogViewer] LogViewerView::new() called for path: {:?}",
            log_path
        );
        let highlighter = SyntaxHighlighter::new();

        // Load content using FileLoader
        let content = FileLoader::load(&log_path, &highlighter.syntax_set, &highlighter.theme)?;
        let total_lines = content.len();

        // Initialize viewport state
        let viewport = ViewportState::new(total_lines);

        // Get initial file size
        let last_file_size = std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0);

        Ok(Self {
            log_path,
            content,
            viewport,
            search: SearchState::default(),
            highlighter,
            total_lines,
            show_json_panel: false,
            last_file_size,
        })
    }

    /// Renders the log viewer to the terminal
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        // Set black background for the entire log viewer area
        let background = Block::default().style(Style::default().bg(Color::Rgb(0, 0, 0)));
        frame.render_widget(background, area);

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
            self.viewport
                .adjust_viewport(visible_height, self.total_lines);

            // Render log content on left
            self.render_content(frame, content_chunks[0]);

            // Render JSON panel on right
            self.render_json_panel(frame, content_chunks[1]);
        } else {
            // Adjust viewport to ensure cursor is visible
            let visible_height = chunks[1].height.saturating_sub(2) as usize;
            self.viewport
                .adjust_viewport(visible_height, self.total_lines);

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

        let current_line = self.viewport.cursor_line + 1;
        let title = format!("{} (Line {}/{})", filename, current_line, self.total_lines);

        let header = Paragraph::new(title)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .style(Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)));

        frame.render_widget(header, area);
    }

    /// Renders the main log content with line numbers
    fn render_content(&self, frame: &mut Frame, area: Rect) {
        // Build text with ALL lines
        let mut text = Text::default();

        for (i, log_line) in self.content.iter().enumerate() {
            // Determine the background color for this line
            let bg_color = if i == self.viewport.cursor_line {
                Color::Rgb(40, 40, 60) // Highlighted cursor line background
            } else {
                Color::Rgb(0, 0, 0) // Default background
            };

            // Create line with line number prefix
            // Using simple ASCII characters for better alignment
            let line_num_str = format!("{:>5} | ", log_line.line_number);
            let line_num_span = if self.search.is_match(i) {
                // Highlight search matches
                Span::styled(
                    line_num_str.clone(),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else if i == self.viewport.cursor_line {
                // Highlight cursor line with arrow
                let cursor_str = format!("{:>5} > ", log_line.line_number);
                Span::styled(cursor_str, Style::default().fg(Color::Cyan))
            } else {
                // Regular line number
                Span::styled(line_num_str.clone(), Style::default().fg(Color::DarkGray))
            };

            // Combine line number with content
            let mut spans = vec![line_num_span];

            // If this is the selected line, we need to override the background of all content spans
            // otherwise their explicit black background will hide the selection highlight
            if i == self.viewport.cursor_line {
                let content_spans: Vec<Span> = log_line
                    .formatted
                    .iter()
                    .map(|s| {
                        let mut style = s.style;
                        style.bg = Some(bg_color);
                        Span::styled(s.content.clone(), style)
                    })
                    .collect();
                spans.extend(content_spans);
            } else {
                spans.extend(log_line.formatted.clone());
            }

            // Use Line::styled to apply background to the entire line
            // This ensures the line fills the full width with the background color
            let mut line = Line::from(spans).style(Style::default().bg(bg_color));

            // Pad the selected line with spaces to ensure the background extends to the full width
            if i == self.viewport.cursor_line {
                let available_width = area.width.saturating_sub(2) as usize; // Subtract borders
                let current_width = line.width();
                if current_width < available_width {
                    let padding = available_width - current_width;
                    let padding_span =
                        Span::styled(" ".repeat(padding), Style::default().bg(bg_color));
                    line.spans.push(padding_span);
                }
            }

            text.lines.push(line);
        }

        // Use Paragraph with scroll and set background style to fill entire area
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(Style::default().bg(Color::Rgb(0, 0, 0))),
            )
            .style(Style::default().bg(Color::Rgb(0, 0, 0)))
            .scroll((self.viewport.viewport_top as u16, 0));

        frame.render_widget(paragraph, area);
    }

    /// Checks if the current cursor line contains JSON
    fn is_current_line_json(&self) -> bool {
        self.content
            .get(self.viewport.cursor_line)
            .map(|line| line.is_json)
            .unwrap_or(false)
    }

    /// Renders the JSON beautifier panel showing prettified JSON
    fn render_json_panel(&self, frame: &mut Frame, area: Rect) {
        let json_content = if let Some(log_line) = self.content.get(self.viewport.cursor_line) {
            if log_line.is_json {
                // Parse and prettify the JSON
                match serde_json::from_str::<serde_json::Value>(log_line.raw.trim()) {
                    Ok(json) => match serde_json::to_string_pretty(&json) {
                        Ok(pretty) => {
                            // Apply syntax highlighting to prettified JSON
                            JsonFormatter::format_prettified_json(
                                &pretty,
                                &self.highlighter.syntax_set,
                                &self.highlighter.theme,
                            )
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
            .style(Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)));

        frame.render_widget(panel, area);
    }

    /// Renders the footer with search bar or keyboard shortcuts
    /// Visual polish: Context-aware footer provides relevant information
    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer_text = if self.search.active {
            // Show search input with match count
            // UX: Real-time feedback on search results
            let match_info = if !self.search.results.is_empty() {
                format!(
                    " [{} matches] ({}/{})",
                    self.search.results.len(),
                    self.search.current_idx + 1,
                    self.search.results.len()
                )
            } else if !self.search.query.is_empty() {
                " [No matches]".to_string()
            } else {
                String::new()
            };

            format!("Search: {}{}", self.search.query, match_info)
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
            .style(Style::default().fg(Color::Gray).bg(Color::Rgb(0, 0, 0)));

        frame.render_widget(footer, area);
    }

    /// Handles keyboard input
    /// Returns true if the event was handled, false otherwise
    pub fn handle_input(&mut self, key: KeyEvent) -> Result<bool> {
        if self.search.active {
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
                self.search.query.pop();
                if let Some(idx) = self.search.perform_search(&self.content) {
                    self.viewport.cursor_line = idx;
                }
                Ok(true)
            }
            KeyCode::Enter | KeyCode::Esc => {
                // Exit search mode
                self.search.active = false;
                Ok(true)
            }
            KeyCode::Char('n') if key.modifiers.is_empty() => {
                // Next search result
                if let Some(idx) = self.search.next_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(true)
            }
            KeyCode::Char('N')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                // Previous search result (Shift+N)
                if let Some(idx) = self.search.previous_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(true)
            }
            KeyCode::Char(c) => {
                // Add character to search query
                self.search.query.push(c);
                if let Some(idx) = self.search.perform_search(&self.content) {
                    self.viewport.cursor_line = idx;
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Handles input when in normal navigation mode
    fn handle_navigation_input(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            // Shift+Down - jump down by 10
            KeyCode::Down
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                self.viewport.scroll_down(10, self.total_lines);
                Ok(true)
            }
            // Shift+Up - jump up by 10
            KeyCode::Up
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                self.viewport.scroll_up(10);
                Ok(true)
            }
            // Scroll down one line
            KeyCode::Down | KeyCode::Char('j') => {
                self.viewport.scroll_down(1, self.total_lines);
                Ok(true)
            }
            // Scroll up one line
            KeyCode::Up | KeyCode::Char('k') => {
                self.viewport.scroll_up(1);
                Ok(true)
            }
            // Page down
            KeyCode::PageDown => {
                self.viewport.scroll_down(20, self.total_lines);
                Ok(true)
            }
            // Page up
            KeyCode::PageUp => {
                self.viewport.scroll_up(20);
                Ok(true)
            }
            // Jump to top
            KeyCode::Home | KeyCode::Char('g') => {
                self.viewport.cursor_line = 0;
                Ok(true)
            }
            // Jump to bottom
            KeyCode::End | KeyCode::Char('G') => {
                self.viewport.cursor_line = self.content.len().saturating_sub(1);
                Ok(true)
            }
            // Enter search mode
            KeyCode::Char('/') => {
                self.search.active = true;
                self.search.query.clear();
                self.search.results.clear();
                self.search.current_idx = 0;
                Ok(true)
            }
            // Next search result
            KeyCode::Char('n') => {
                if let Some(idx) = self.search.next_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(true)
            }
            // Previous search result
            KeyCode::Char('N') => {
                if let Some(idx) = self.search.previous_result() {
                    self.viewport.cursor_line = idx;
                }
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

    /// Returns the path to the log file
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }

    /// Refreshes the log file content if it has changed
    /// Returns true if the content was updated
    pub fn refresh(&mut self) -> Result<bool> {
        // Check if file size has changed
        let current_size = std::fs::metadata(&self.log_path)
            .map(|m| m.len())
            .unwrap_or(0);

        if current_size == self.last_file_size {
            // No changes
            return Ok(false);
        }

        // Remember if we were at the bottom before refresh
        let was_at_bottom = self.viewport.cursor_line + 1 >= self.total_lines;

        // Reload the file
        let new_content = FileLoader::load(
            &self.log_path,
            &self.highlighter.syntax_set,
            &self.highlighter.theme,
        )?;
        let new_total_lines = new_content.len();

        // Update content
        self.content = new_content;
        self.total_lines = new_total_lines;
        self.last_file_size = current_size;

        // If we were at the bottom, stay at the bottom (auto-scroll)
        if was_at_bottom && new_total_lines > 0 {
            self.viewport.cursor_line = new_total_lines.saturating_sub(1);
        }

        Ok(true)
    }
}
