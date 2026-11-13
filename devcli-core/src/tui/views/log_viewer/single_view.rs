// One log panel: loading, incremental refresh, window rendering, search and
// navigation for a single log file.

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

use super::file_loader::LogLine;
use super::{
    file_loader::FileLoader, json_formatter::JsonFormatter, search, search::SearchState,
    syntax_highlighter::SyntaxHighlighter, viewport::ViewportState, LogInputResult,
};

/// Single log panel viewer
/// Handles display logic for a single log file
pub struct SingleLogView {
    /// Path to the log file being viewed
    pub(super) log_path: PathBuf,
    /// All loaded log lines (lazy loaded in chunks)
    content: Vec<LogLine>,
    /// Viewport state (cursor, scroll position)
    viewport: ViewportState,
    /// Search state (query, results)
    search: SearchState,
    /// Syntax highlighting resources (shared by every panel)
    highlighter: &'static SyntaxHighlighter,
    /// Total number of lines in the file (for display)
    total_lines: usize,
    /// Whether to show the JSON beautifier panel
    show_json_panel: bool,
    /// Last file size to detect changes
    last_file_size: u64,
    /// Modification time, device and inode, and first bytes of the file as
    /// last read: together they tell an append from a file replaced or
    /// rewritten in place (see `refresh`).
    last_mtime: Option<std::time::SystemTime>,
    file_id: Option<(u64, u64)>,
    head: Vec<u8>,
    /// Bytes of the file parsed into complete lines (up to the last `\n`):
    /// a refresh only reads what was appended after this.
    complete_bytes: u64,
    /// Whether the last line in `content` had no trailing newline yet (it is
    /// re-parsed once the rest of it arrives).
    partial_tail: bool,
    /// Wrapped row count of each line at `row_cache_width`, computed lazily for
    /// the lines around the visible window only.
    row_cache: Vec<Option<usize>>,
    row_cache_width: usize,
    /// App name extracted from log file path
    app_name: String,
    /// Color assigned to this app for consistent display
    app_color: Color,
}

impl SingleLogView {
    /// Creates a new log viewer for the specified file
    /// Loads the file content and prepares it for display
    ///
    /// # Arguments
    /// * `log_path` - Path to the log file
    /// * `app_name` - Name of the app (extracted from file path)
    /// * `app_color` - Color assigned to this app for consistent display
    pub fn new(log_path: PathBuf, app_name: String, app_color: Color) -> Result<Self> {
        crate::debug!(
            "[LogViewer] SingleLogView::new() called for path: {:?}, app: {}, color: {:?}",
            log_path,
            app_name,
            app_color
        );
        let highlighter = SyntaxHighlighter::shared();

        let mut view = Self {
            log_path,
            content: Vec::new(),
            viewport: ViewportState::new(0),
            search: SearchState::default(),
            highlighter,
            total_lines: 0,
            show_json_panel: false,
            last_file_size: 0,
            last_mtime: None,
            file_id: None,
            head: Vec::new(),
            complete_bytes: 0,
            partial_tail: false,
            row_cache: Vec::new(),
            row_cache_width: 0,
            app_name,
            app_color,
        };
        view.reload()?;
        // Start at the bottom of the file (most recent logs)
        view.viewport = ViewportState::new(view.total_lines);
        Ok(view)
    }

    /// Renders the log viewer to the terminal
    pub fn render(&mut self, frame: &mut Frame, area: Rect, is_active: bool) {
        // Set background for the entire log viewer area
        let background = Block::default().style(Style::default().bg(Color::Rgb(0, 0, 0)));
        frame.render_widget(background, area);

        // Create the main layout with header and content
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
            ])
            .split(area);

        // Render header with file name and position
        self.render_header(frame, chunks[0], is_active);

        // Split content area if JSON panel is visible
        if self.show_json_panel && self.is_current_line_json() {
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50), // Log content
                    Constraint::Percentage(50), // JSON panel
                ])
                .split(chunks[1]);

            // Render log content on left
            self.render_content(frame, content_chunks[0], is_active);

            // Render JSON panel on right
            self.render_json_panel(frame, content_chunks[1]);
        } else {
            // Render log content full width
            self.render_content(frame, chunks[1], is_active);
        }
    }

    /// Renders the header showing file name and current position with colored app name
    fn render_header(&self, frame: &mut Frame, area: Rect, is_active: bool) {
        let filename = self
            .log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        let (project, context_date) =
            match crate::process_manager_support::LogFileName::parse(filename) {
                Some(name) => (
                    name.guess_project_app().0,
                    format!("{} [{}]", name.environment, name.date),
                ),
                None => ("Unknown".to_string(), filename.replace(".log", "")),
            };

        let current_line = self.viewport.cursor_line + 1;

        // Create title with colored app name
        let title_spans = vec![
            Span::styled(format!("{} - ", project), Style::default().fg(Color::White)),
            Span::styled(
                format!("[{}]", self.app_name),
                Style::default()
                    .fg(self.app_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " {} (Line {}/{})",
                    context_date, current_line, self.total_lines
                ),
                Style::default().fg(Color::White),
            ),
        ];

        // Active panel gets a distinct border color
        let border_color = if is_active {
            Color::Green
        } else {
            Color::DarkGray
        };

        let header = Paragraph::new(Line::from(title_spans))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)),
            )
            .style(Style::default().bg(Color::Rgb(0, 0, 0)));

        frame.render_widget(header, area);
    }

    /// Applies search highlighting to formatted spans
    /// Highlights matched text with bright background colors
    ///
    /// # Arguments
    /// * `formatted_spans` - The original formatted spans from the log line
    /// * `match_ranges` - Character ranges within the line that match the search query
    /// * `bg_color` - Background color to apply to non-highlighted text
    /// * `is_current_result` - Whether this is the currently selected search result
    ///
    /// # Returns
    /// Vector of spans with search highlighting applied
    fn apply_search_highlighting(
        &self,
        formatted_spans: &[ratatui::text::Span],
        match_ranges: &[search::MatchRange],
        bg_color: Color,
        is_current_result: bool,
    ) -> Vec<Span<'static>> {
        let mut result_spans = Vec::new();
        let mut char_pos = 0;

        for span in formatted_spans {
            let span_start = char_pos;
            let span_end = char_pos + span.content.chars().count(); // Use char count, not byte length

            // Find matches that overlap with this span
            let overlapping_matches: Vec<_> = match_ranges
                .iter()
                .filter(|m| m.start < span_end && m.end > span_start)
                .collect();

            if overlapping_matches.is_empty() {
                // No matches in this span, just apply background
                let mut style = span.style;
                style.bg = Some(bg_color);
                result_spans.push(Span::styled(span.content.to_string(), style));
            } else {
                // Split span to highlight matches
                let mut current_pos = 0;
                let span_chars: Vec<char> = span.content.chars().collect();

                for &match_range in &overlapping_matches {
                    let match_start_in_span = match_range.start.saturating_sub(span_start);
                    let match_end_in_span =
                        (match_range.end.saturating_sub(span_start)).min(span_chars.len());

                    // Add text before match
                    if current_pos < match_start_in_span {
                        let before_text: String = span_chars[current_pos..match_start_in_span]
                            .iter()
                            .collect();
                        let mut style = span.style;
                        style.bg = Some(bg_color);
                        result_spans.push(Span::styled(before_text, style));
                    }

                    // Add highlighted match
                    if match_start_in_span < match_end_in_span {
                        let match_text: String = span_chars[match_start_in_span..match_end_in_span]
                            .iter()
                            .collect();
                        let highlight_color = if is_current_result {
                            Color::Rgb(255, 255, 0) // Bright yellow for current result
                        } else {
                            Color::Rgb(200, 200, 100) // Dimmer yellow for other matches
                        };

                        let mut style = span.style;
                        style.bg = Some(highlight_color);
                        style.fg = Some(Color::Black); // Black text on yellow background
                        style = style.add_modifier(Modifier::BOLD);
                        result_spans.push(Span::styled(match_text, style));
                    }

                    current_pos = match_end_in_span;
                }

                // Add remaining text after last match
                if current_pos < span_chars.len() {
                    let after_text: String = span_chars[current_pos..].iter().collect();
                    let mut style = span.style;
                    style.bg = Some(bg_color);
                    result_spans.push(Span::styled(after_text, style));
                }
            }

            char_pos = span_end;
        }

        result_spans
    }

    /// Wrapped rows line `i` takes at `width` (cached).
    ///
    /// Measured with ratatui's own `Paragraph::line_count` on the same text the
    /// renderer draws, so scrolling never drifts from what is on screen.
    fn rows(&mut self, i: usize, width: usize) -> usize {
        if let Some(Some(rows)) = self.row_cache.get(i) {
            return *rows;
        }
        let line = &self.content[i];
        let mut text = format!("{:>5} | [{}] ", line.line_number, self.app_name);
        for span in &line.formatted {
            text.push_str(&span.content);
        }
        let rows = Paragraph::new(Line::raw(text))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .line_count(width.min(u16::MAX as usize) as u16)
            .max(1);
        if let Some(slot) = self.row_cache.get_mut(i) {
            *slot = Some(rows);
        }
        rows
    }

    /// `(top_line, top_offset)` that puts the last row of `line` on the last
    /// row of a `height`-row window.
    fn anchor_bottom(&mut self, line: usize, height: usize, width: usize) -> (usize, usize) {
        let mut top = line;
        let mut used = self.rows(line, width);
        while top > 0 && used < height {
            top -= 1;
            used += self.rows(top, width);
        }
        (top, used.saturating_sub(height))
    }

    /// Builds the styled line `i` exactly as it is displayed.
    fn display_line(&self, i: usize, available_width: usize, is_active: bool) -> Line<'static> {
        let log_line = &self.content[i];
        let bg_color = if i == self.viewport.cursor_line {
            // Deliberately bright: a too-dark band (previous Rgb(40, 40, 60))
            // was invisible on the black background, so the selected line
            // seemed to have no highlight at all.
            if is_active {
                Color::Rgb(90, 90, 135)
            } else {
                Color::Rgb(55, 55, 85)
            }
        } else {
            Color::Rgb(0, 0, 0)
        };

        let line_num_span = if self.search.is_current_result(i) {
            // Current search result gets bright yellow with arrow
            Span::styled(
                format!("{:>5} ▶ ", log_line.line_number),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        } else if self.search.is_match(i) {
            // Other search matches get dimmer yellow
            Span::styled(
                format!("{:>5} | ", log_line.line_number),
                Style::default()
                    .fg(Color::Rgb(200, 200, 100))
                    .add_modifier(Modifier::BOLD),
            )
        } else if i == self.viewport.cursor_line {
            Span::styled(
                format!("{:>5} > ", log_line.line_number),
                Style::default().fg(Color::Cyan),
            )
        } else {
            Span::styled(
                format!("{:>5} | ", log_line.line_number),
                Style::default().fg(Color::DarkGray),
            )
        };

        let mut spans = vec![
            line_num_span,
            // Colored app name prefix: [appName]
            Span::styled(
                format!("[{}] ", self.app_name),
                Style::default()
                    .fg(self.app_color)
                    .add_modifier(Modifier::BOLD)
                    .bg(bg_color),
            ),
        ];

        // Apply search highlighting if this line has matches
        if let Some(match_ranges) = self.search.get_match_ranges(i) {
            spans.extend(self.apply_search_highlighting(
                &log_line.formatted,
                match_ranges,
                bg_color,
                self.search.is_current_result(i),
            ));
        } else {
            spans.extend(log_line.formatted.iter().map(|s| {
                let mut style = s.style;
                style.bg = Some(bg_color);
                Span::styled(s.content.clone(), style)
            }));
        }

        // Pad to the full width so the background covers the row. Measured in
        // display cells: byte lengths over-padded lines with non-ASCII text,
        // pushing the padding onto an extra wrapped row.
        let current_width: usize = spans.iter().map(Span::width).sum();
        let remaining_width = available_width.saturating_sub(current_width);
        if remaining_width > 0 {
            spans.push(Span::styled(
                " ".repeat(remaining_width),
                Style::default().bg(bg_color),
            ));
        }

        Line::from(spans).style(Style::default().bg(bg_color))
    }

    /// Renders the main log content with line numbers and text wrapping
    ///
    /// Only the lines that fit in the panel are laid out and built. Rendering
    /// used to rebuild, clone and wrap every line of the file on each frame
    /// (and re-measure all of it), which froze the UI on large logs; the
    /// absolute scroll offset was also a `u16`, so past 65,535 wrapped rows the
    /// newest lines could not be reached.
    fn render_content(&mut self, frame: &mut Frame, area: Rect, is_active: bool) {
        let available_width = area.width.saturating_sub(2) as usize;
        let visible_height = area.height.saturating_sub(2) as usize;

        let border_color = if is_active {
            Color::White
        } else {
            Color::DarkGray
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(Color::Rgb(0, 0, 0)));

        if self.content.is_empty() || visible_height == 0 || available_width == 0 {
            frame.render_widget(
                Paragraph::new(Text::default())
                    .block(block)
                    .style(Style::default().bg(Color::Rgb(0, 0, 0))),
                area,
            );
            return;
        }

        if self.row_cache_width != available_width {
            self.row_cache.clear();
            self.row_cache_width = available_width;
        }
        self.row_cache.resize(self.content.len(), None);

        // A shrunk/rotated file can leave the cursor past the content — never
        // let the selection point into the void (keeps the highlight visible).
        let last = self.content.len() - 1;
        self.viewport.cursor_line = self.viewport.cursor_line.min(last);
        let cursor = self.viewport.cursor_line;

        // Standard (pager-like) scrolling:
        // - cursor on the last line: pin the view to the bottom (follow mode)
        // - cursor above the window: it becomes the first visible line
        // - cursor below the window: it becomes the last visible line
        // - otherwise keep the current position
        let mut top = self.viewport.top_line.min(last);
        let mut offset = self
            .viewport
            .top_offset
            .min(self.rows(top, available_width).saturating_sub(1));
        if cursor == last {
            (top, offset) = self.anchor_bottom(last, visible_height, available_width);
        } else if cursor < top || (cursor == top && offset > 0) {
            top = cursor;
            offset = 0;
        } else {
            let mut used = self.rows(top, available_width) - offset;
            let mut i = top;
            while i < cursor && used <= visible_height {
                i += 1;
                used += self.rows(i, available_width);
            }
            if used > visible_height {
                (top, offset) = self.anchor_bottom(cursor, visible_height, available_width);
            }
        }
        self.viewport.top_line = top;
        self.viewport.top_offset = offset;

        // Build just the lines that intersect the window.
        let mut lines = Vec::new();
        let mut shown = 0usize;
        let mut i = top;
        while i <= last && shown < visible_height + offset {
            shown += self.rows(i, available_width);
            lines.push(self.display_line(i, available_width, is_active));
            i += 1;
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .block(block)
            .style(Style::default().bg(Color::Rgb(0, 0, 0)))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((offset.min(u16::MAX as usize) as u16, 0));
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

    /// Returns status text for the unified footer
    pub fn get_status_text(&self) -> String {
        let match_info = if !self.search.results.is_empty() {
            format!(
                " [{}/{} matches]",
                self.search.current_idx + 1,
                self.search.results.len()
            )
        } else if self.search.has_query() {
            " [No matches]".to_string()
        } else {
            String::new()
        };
        if self.search.active {
            let help = if self.search.has_query() {
                "  ↑↓: Prev/Next  Enter: Done  Esc: Cancel"
            } else {
                "  Type to search, Esc: Cancel"
            };
            format!("Search: {}{match_info}{help}", self.search.query)
        } else if self.search.has_query() {
            format!(
                "Search: {}{match_info}  n/N: Next/Prev  /: New search  Esc: Clear search",
                self.search.query
            )
        } else {
            let json_hint = if self.is_current_line_json() {
                if self.show_json_panel {
                    "  J: Hide JSON"
                } else {
                    "  J: Show JSON"
                }
            } else {
                ""
            };

            // Layout is managed by parent, so we assume active context keys are relevant
            format!(
                "Tab: Switch  n: New Panel  w: Close Panel  L: Change Log  Esc: Exit  {}/jk: Scroll  /: Search  {}",
                "\u{2191}\u{2193}", // Arrows up/down
                json_hint
            )
        }
    }

    /// Handles keyboard input
    /// Returns result indicating action taken
    pub fn handle_input(&mut self, key: KeyEvent) -> Result<LogInputResult> {
        if self.search.active {
            // Handle search mode input
            self.handle_search_input(key)
        } else {
            // Handle normal navigation input
            self.handle_navigation_input(key)
        }
    }

    /// Handles input while the search query is typed.
    ///
    /// Every printable character goes into the query (`j`, `k` and non-ASCII
    /// ones used to be taken for navigation or dropped); the arrows move
    /// between results; Enter keeps the search (n/N then move between its
    /// results) and Esc cancels it.
    fn handle_search_input(&mut self, key: KeyEvent) -> Result<LogInputResult> {
        use crossterm::event::KeyModifiers;

        match key.code {
            KeyCode::Backspace => {
                // Remove last character from search query
                self.search.query.pop();
                if let Some(idx) = self.search.perform_search(&self.content) {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            KeyCode::Enter => {
                self.search.active = false;
                if !self.search.has_query() {
                    self.search.clear_results();
                } else if let Some(&idx) = self.search.results.get(self.search.current_idx) {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            KeyCode::Esc => {
                // Cancel the search and clear all highlighting
                self.clear_search();
                Ok(LogInputResult::Handled)
            }
            KeyCode::Down => {
                if let Some(idx) = self.search.next_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            KeyCode::Up => {
                if let Some(idx) = self.search.previous_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            // Printable characters, AltGr ones included (Ctrl+Alt on some
            // systems); Ctrl+key is not text.
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.search.query.push(c);
                if let Some(idx) = self.search.perform_search(&self.content) {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            // Typing: nothing else acts behind the query.
            _ => Ok(LogInputResult::Handled),
        }
    }

    fn clear_search(&mut self) {
        self.search.active = false;
        self.search.query.clear();
        self.search.clear_results();
    }

    /// Handles input when in normal navigation mode
    fn handle_navigation_input(&mut self, key: KeyEvent) -> Result<LogInputResult> {
        match key.code {
            // Shift+Down - jump down by 10
            KeyCode::Down
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                self.viewport.scroll_down(10, self.total_lines);
                Ok(LogInputResult::Handled)
            }
            // Shift+Up - jump up by 10
            KeyCode::Up
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::SHIFT) =>
            {
                self.viewport.scroll_up(10);
                Ok(LogInputResult::Handled)
            }
            // Scroll down one line
            KeyCode::Down | KeyCode::Char('j') => {
                self.viewport.scroll_down(1, self.total_lines);
                Ok(LogInputResult::Handled)
            }
            // Scroll up one line
            KeyCode::Up | KeyCode::Char('k') => {
                self.viewport.scroll_up(1);
                Ok(LogInputResult::Handled)
            }
            // Page down
            KeyCode::PageDown => {
                self.viewport.scroll_down(20, self.total_lines);
                Ok(LogInputResult::Handled)
            }
            // Page up
            KeyCode::PageUp => {
                self.viewport.scroll_up(20);
                Ok(LogInputResult::Handled)
            }
            // Jump to top
            KeyCode::Home | KeyCode::Char('g') => {
                self.viewport.jump_top();
                Ok(LogInputResult::Handled)
            }
            // Jump to bottom
            KeyCode::End | KeyCode::Char('G') => {
                self.viewport.jump_bottom(self.total_lines);
                Ok(LogInputResult::Handled)
            }
            // Enter search mode
            KeyCode::Char('/') => {
                self.clear_search();
                self.search.active = true;
                Ok(LogInputResult::Handled)
            }
            // While a search is kept, n/N move between its results...
            KeyCode::Char('n') if self.search.has_query() => {
                if let Some(idx) = self.search.next_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            KeyCode::Char('N') if self.search.has_query() => {
                if let Some(idx) = self.search.previous_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            // ...and Esc clears it (then Esc leaves the viewer).
            KeyCode::Esc if self.search.has_query() => {
                self.clear_search();
                Ok(LogInputResult::Handled)
            }
            // Otherwise `n` adds a panel.
            KeyCode::Char('n') => Ok(LogInputResult::RequestAddPanel),
            // Add Select Log shortcut (L)
            KeyCode::Char('L') => Ok(LogInputResult::RequestSelectLog),
            // Toggle JSON panel
            KeyCode::Char('J') => {
                self.show_json_panel = !self.show_json_panel;
                Ok(LogInputResult::Handled)
            }
            _ => Ok(LogInputResult::Ignored),
        }
    }

    /// Keep the cursor inside the content (after a layout change).
    pub(super) fn clamp_cursor(&mut self) {
        self.viewport.clamp_cursor(self.total_lines);
    }

    /// Returns the path to the log file
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }

    /// Load the whole file from scratch.
    fn reload(&mut self) -> Result<()> {
        // Taken before reading: a write in between shows as a change next
        // time, and is then read (an append) or reloaded.
        let meta = std::fs::metadata(&self.log_path)?;
        let bytes = std::fs::read(&self.log_path)?;
        self.last_mtime = meta.modified().ok();
        self.file_id = file_id(&meta);
        self.head = bytes[..bytes.len().min(HEAD_LEN)].to_vec();
        let text = String::from_utf8_lossy(&bytes);
        self.content = FileLoader::parse_lines(
            &text,
            1,
            &self.highlighter.syntax_set,
            &self.highlighter.theme,
        );
        self.total_lines = self.content.len();
        self.last_file_size = bytes.len() as u64;
        self.complete_bytes = bytes
            .iter()
            .rposition(|&b| b == b'\n')
            .map_or(0, |pos| pos as u64 + 1);
        self.partial_tail = self.complete_bytes < self.last_file_size;
        self.row_cache.clear();
        self.search.rescan_from(&self.content, 0);
        Ok(())
    }

    /// Whether the file still starts with the bytes it started with.
    fn head_unchanged(&self) -> Result<bool> {
        use std::io::Read;

        let mut head = Vec::with_capacity(self.head.len());
        std::fs::File::open(&self.log_path)?
            .take(self.head.len() as u64)
            .read_to_end(&mut head)?;
        Ok(head == self.head)
    }

    /// Refreshes the log file content if it has changed
    /// Returns true if the content was updated
    ///
    /// Appended output is read and parsed incrementally; the whole file used to
    /// be re-read, re-parsed and re-highlighted every time it grew (checked
    /// every 100 ms), which is constant work proportional to the log size for
    /// any app that is logging.
    ///
    /// A file that was replaced (another inode: rotated by renaming) or
    /// rewritten (shorter than what was read, or its first bytes changed:
    /// truncated and written again) is reloaded from the start. Only a
    /// shrink used to be noticed, so a log truncated and refilled to the
    /// same or a larger size showed stale or garbled lines.
    pub fn refresh(&mut self) -> Result<bool> {
        // A file that is gone (being rotated) keeps what is shown.
        let Ok(meta) = std::fs::metadata(&self.log_path) else {
            return Ok(false);
        };
        let current_size = meta.len();
        let mtime = meta.modified().ok();
        let id = file_id(&meta);
        if current_size == self.last_file_size && mtime == self.last_mtime && id == self.file_id {
            // No changes
            return Ok(false);
        }

        // Remember if we were at the bottom before refresh
        let was_at_bottom = self.viewport.cursor_line + 1 >= self.total_lines;

        let replaced =
            id != self.file_id || current_size < self.complete_bytes || !self.head_unchanged()?;
        if replaced {
            self.reload()?;
        } else if current_size == self.last_file_size {
            // Touched, same content as far as can be told.
            self.last_mtime = mtime;
            return Ok(false);
        } else {
            use std::io::{Read, Seek, SeekFrom};

            let read_from = self.complete_bytes;
            let mut file = std::fs::File::open(&self.log_path)?;
            file.seek(SeekFrom::Start(read_from))?;
            let mut appended = Vec::new();
            file.read_to_end(&mut appended)?;

            if self.partial_tail {
                // The previous last line was incomplete: it is in `appended`.
                self.content.pop();
            }
            let complete = appended
                .iter()
                .rposition(|&b| b == b'\n')
                .map_or(0, |pos| pos + 1);
            let text = String::from_utf8_lossy(&appended);
            let first_line_number = self.content.len() + 1;
            self.content.extend(FileLoader::parse_lines(
                &text,
                first_line_number,
                &self.highlighter.syntax_set,
                &self.highlighter.theme,
            ));
            self.total_lines = self.content.len();
            self.complete_bytes += complete as u64;
            self.last_file_size = self.complete_bytes + (appended.len() - complete) as u64;
            self.partial_tail = complete < appended.len();
            self.last_mtime = mtime;
            // A head shorter than HEAD_LEN is the whole file as it was:
            // complete it with the bytes that follow it.
            if self.head.len() < HEAD_LEN {
                let skip = (self.head.len() as u64).saturating_sub(read_from) as usize;
                let more = appended.get(skip..).unwrap_or_default();
                let wanted = HEAD_LEN - self.head.len();
                self.head.extend_from_slice(&more[..more.len().min(wanted)]);
            }
            // Lines past the old end (and a re-parsed tail) need measuring.
            self.row_cache.truncate(first_line_number.saturating_sub(1));
            // ... and matching against the search.
            self.search
                .rescan_from(&self.content, first_line_number.saturating_sub(1));
        }

        // Shrunk/rotated file: clamp a cursor that fell past the new content
        // so the selection stays visible.
        self.viewport.clamp_cursor(self.total_lines);

        // If we were at the bottom, stay at the bottom (auto-scroll)
        if was_at_bottom && self.total_lines > 0 {
            self.viewport.jump_bottom(self.total_lines);
        }

        Ok(true)
    }
}

/// How many first bytes of a log are compared to tell a rewrite from an
/// append.
const HEAD_LEN: usize = 256;

/// Device and inode of a file, where the platform has them.
fn file_id(meta: &std::fs::Metadata) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Some((meta.dev(), meta.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view_with_lines(dir: &tempfile::TempDir, count: usize) -> SingleLogView {
        let path = dir.path().join("p_api_local_20260101.log");
        let body: String = (0..count).map(|i| format!("line {i}\n")).collect();
        std::fs::write(&path, body).unwrap();
        SingleLogView::new(path, "api".to_string(), Color::White).unwrap()
    }

    fn render_once(view: &mut SingleLogView, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| view.render_content(frame, frame.area(), true))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    /// A long log renders its newest lines (bottom pinned) without laying out
    /// the whole file, and the scroll position stays line-based.
    #[test]
    fn large_log_shows_the_newest_lines() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut view = view_with_lines(&dir, 100_000);
        let screen = render_once(&mut view, 60, 12);
        assert!(screen.contains("line 99999"), "{screen}");
        assert!(!screen.contains("line 99980"), "{screen}");
        // Only the window (and a little context) was measured.
        let measured = view.row_cache.iter().filter(|r| r.is_some()).count();
        assert!(measured < 50, "measured {measured} lines");
        assert_eq!(view.viewport.top_line, 100_000 - 10);
    }

    #[test]
    fn moving_the_cursor_up_scrolls_the_window() {
        let dir = tempfile::TempDir::new().unwrap();
        let mut view = view_with_lines(&dir, 1_000);
        render_once(&mut view, 60, 12);
        view.viewport.scroll_up(500);
        let screen = render_once(&mut view, 60, 12);
        assert!(screen.contains("line 499"), "{screen}");
        assert!(!screen.contains("line 999"), "{screen}");
    }

    /// Appended output is parsed incrementally, a partial last line included.
    #[test]
    fn refresh_appends_and_completes_the_partial_line() {
        use std::io::Write;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("p_api_local_20260101.log");
        std::fs::write(&path, "one\ntw").unwrap();
        let mut view = SingleLogView::new(path.clone(), "api".to_string(), Color::White).unwrap();
        assert_eq!(view.total_lines, 2);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"o\nthree\n").unwrap();
        assert!(view.refresh().unwrap());
        let raws: Vec<&str> = view.content.iter().map(|l| l.raw.as_str()).collect();
        assert_eq!(raws, vec!["one", "two", "three"]);
        assert_eq!(view.content[2].line_number, 3);

        // Rotation (truncate) reloads from scratch.
        std::fs::write(&path, "fresh\n").unwrap();
        assert!(view.refresh().unwrap());
        assert_eq!(view.total_lines, 1);
    }

    #[test]
    fn logs_with_invalid_utf8_open() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("p_api_local_20260101.log");
        std::fs::write(&path, b"ok\nbad \xff\xfe bytes\nlast\n").unwrap();
        let view = SingleLogView::new(path, "api".to_string(), Color::White).unwrap();
        assert_eq!(view.total_lines, 3);
    }

    /// Highlighting a search hit must not duplicate or shift text.
    #[test]
    fn search_highlight_keeps_the_text_intact() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("p_api_local_20260101.log");
        std::fs::write(&path, "café aaaa\n").unwrap();
        let mut view = SingleLogView::new(path, "api".to_string(), Color::White).unwrap();
        view.search.query = "aa".to_string();
        view.search.perform_search(&view.content);

        let spans = view.apply_search_highlighting(
            &view.content[0].formatted,
            view.search.get_match_ranges(0).unwrap(),
            Color::Black,
            true,
        );
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "café aaaa");
        let highlighted: Vec<&str> = spans
            .iter()
            .filter(|s| s.style.bg == Some(Color::Rgb(255, 255, 0)))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(highlighted, vec!["aa", "aa"]);
    }

    fn press(view: &mut SingleLogView, code: KeyCode) -> LogInputResult {
        view.handle_input(KeyEvent::new(code, crossterm::event::KeyModifiers::NONE))
            .unwrap()
    }

    fn type_query(view: &mut SingleLogView, query: &str) {
        press(view, KeyCode::Char('/'));
        for c in query.chars() {
            press(view, KeyCode::Char(c));
        }
    }

    fn log(dir: &tempfile::TempDir, body: &str) -> (PathBuf, SingleLogView) {
        let path = dir.path().join("p_api_local_20260101.log");
        std::fs::write(&path, body).unwrap();
        let view = SingleLogView::new(path.clone(), "api".to_string(), Color::White).unwrap();
        (path, view)
    }

    /// Every printable character is part of the query: `j`/`k` used to
    /// navigate and non-ASCII ones were dropped.
    #[test]
    fn the_search_query_takes_every_character() {
        let dir = tempfile::TempDir::new().unwrap();
        let (_, mut view) = log(&dir, "jkété\nother\n");
        type_query(&mut view, "jkété");
        assert_eq!(view.search.query, "jkété");
        assert_eq!(view.search.results, vec![0]);
    }

    /// Enter keeps the search: n/N move between its results, Esc clears it,
    /// and `n` adds a panel again.
    #[test]
    fn a_confirmed_search_is_navigated_with_n() {
        let dir = tempfile::TempDir::new().unwrap();
        let (_, mut view) = log(&dir, "hit 1\nmiss\nhit 2\nhit 3\n");
        type_query(&mut view, "hit");
        press(&mut view, KeyCode::Enter);
        assert!(!view.search.active);
        assert_eq!(view.viewport.cursor_line, 0);

        assert_eq!(
            press(&mut view, KeyCode::Char('n')),
            LogInputResult::Handled
        );
        assert_eq!(view.viewport.cursor_line, 2);
        press(&mut view, KeyCode::Char('n'));
        assert_eq!(view.viewport.cursor_line, 3);
        press(&mut view, KeyCode::Char('N'));
        assert_eq!(view.viewport.cursor_line, 2);

        assert_eq!(press(&mut view, KeyCode::Esc), LogInputResult::Handled);
        assert!(view.search.results.is_empty());
        assert_eq!(
            press(&mut view, KeyCode::Char('n')),
            LogInputResult::RequestAddPanel
        );
        assert_eq!(press(&mut view, KeyCode::Esc), LogInputResult::Ignored);
    }

    /// New lines are matched against the search; a reload drops stale hits.
    #[test]
    fn refresh_updates_the_search() {
        use std::io::Write;

        let dir = tempfile::TempDir::new().unwrap();
        let (path, mut view) = log(&dir, "error one\nok\n");
        type_query(&mut view, "error");
        press(&mut view, KeyCode::Enter);
        assert_eq!(view.search.results, vec![0]);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"error two\n").unwrap();
        assert!(view.refresh().unwrap());
        assert_eq!(view.search.results, vec![0, 2]);
        assert!(view.search.is_match(2));

        std::fs::write(&path, b"ok\n").unwrap();
        assert!(view.refresh().unwrap());
        assert!(view.search.results.is_empty());
        assert!(!view.search.is_match(0));
    }

    /// A log truncated and written again, to a larger size, is reloaded
    /// (only a shrink used to be noticed).
    #[test]
    fn a_rewritten_log_is_reloaded() {
        let dir = tempfile::TempDir::new().unwrap();
        let (path, mut view) = log(&dir, "first run\n");
        std::fs::write(&path, b"second run, longer\nand more\n").unwrap();
        assert!(view.refresh().unwrap());
        let raws: Vec<&str> = view.content.iter().map(|l| l.raw.as_str()).collect();
        assert_eq!(raws, vec!["second run, longer", "and more"]);
    }
}
