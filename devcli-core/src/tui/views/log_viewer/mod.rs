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

/// Result of handling input in LogViewerView
#[derive(Debug, PartialEq)]
pub enum LogInputResult {
    Handled,
    Ignored,

    RequestAddPanel,
    RequestSelectLog,
}

/// Item in the log selection list
#[derive(Debug, PartialEq)]
pub enum SelectionItem {
    Header(String),
    SubHeader(String),
    Option { label: String, path: PathBuf },
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SelectionMode {
    Replace(usize),
    Add,
}

pub struct LogSelectionState {
    pub active: bool,
    pub candidates: Vec<SelectionItem>,
    pub selected_idx: usize,
    pub mode: SelectionMode,
}

impl Default for LogSelectionState {
    fn default() -> Self {
        Self {
            active: false,
            candidates: Vec::new(),
            selected_idx: 0,
            mode: SelectionMode::Replace(0),
        }
    }
}

/// Full-screen log viewer with beautification and search
/// Handles multiple log panels in a split-screen layout
pub struct LogViewerView {
    /// List of active log panels
    panels: Vec<SingleLogView>,
    /// Index of the currently active panel
    active_panel_idx: usize,
    /// State for the log selection popup
    selection_state: LogSelectionState,
}

impl LogViewerView {
    /// Creates a new log viewer with initial log paths
    pub fn new(log_paths: Vec<PathBuf>) -> Result<Self> {
        let mut panels = Vec::new();
        for path in log_paths {
            panels.push(SingleLogView::new(path)?);
        }

        if panels.is_empty() {
            // Should ideally not happen or handle empty state, but for now let's assume at least one path
        }

        Ok(Self {
            panels,
            active_panel_idx: 0,
            selection_state: LogSelectionState::default(),
        })
    }

    /// Renders the log viewer to the terminal with support for up to 4 panels
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if self.panels.is_empty() && !self.selection_state.active {
            // If completely empty and no popup, render a hint to press 'n' or 'L'
            let hints =
                Paragraph::new("No active logs.\nPress 'n' to add a panel or 'L' to select a log.")
                    .alignment(ratatui::layout::Alignment::Center)
                    .block(Block::default().borders(Borders::ALL))
                    .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(hints, area);
            return;
        }

        // Split main layout into Content (top) and Footer (bottom)
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Content
                Constraint::Length(3), // Unified Footer
            ])
            .split(area);

        let content_area = main_chunks[0];
        let footer_area = main_chunks[1];

        // Calculate layout based on number of panels using content_area
        let chunks = match self.panels.len() {
            1 => vec![content_area],
            2 => Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(content_area)
                .to_vec(),
            3 => {
                let layout_split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(content_area);
                let right_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(layout_split[1]);
                vec![layout_split[0], right_chunks[0], right_chunks[1]]
            }
            4 => {
                let layout_split = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(content_area);
                let top_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(layout_split[0]);
                let bottom_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(layout_split[1]);
                vec![
                    top_chunks[0],
                    top_chunks[1],
                    bottom_chunks[0],
                    bottom_chunks[1],
                ]
            }
            _ => vec![content_area], // Fallback
        };

        for (i, panel) in self.panels.iter_mut().enumerate() {
            if let Some(chunk) = chunks.get(i) {
                let is_active = i == self.active_panel_idx;
                panel.render(frame, *chunk, is_active);
            }
        }

        // Render Unified Footer
        // Get status text from active panel
        let active_status = if let Some(panel) = self.panels.get(self.active_panel_idx) {
            panel.get_status_text()
        } else {
            String::new()
        };

        // Render the footer
        let footer = Paragraph::new(active_status)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::Gray).bg(Color::Rgb(0, 0, 0)));

        frame.render_widget(footer, footer_area);

        // Render Popup if active
        if self.selection_state.active {
            self.render_log_selection(frame, area);
        }
    }

    fn render_log_selection(&self, frame: &mut Frame, area: Rect) {
        let popup_width = 80; // Wider for formatted names
        let popup_height = 20;

        // Center the popup
        let area = if area.width >= popup_width && area.height >= popup_height {
            let x = (area.width - popup_width) / 2;
            let y = (area.height - popup_height) / 2;
            Rect::new(area.x + x, area.y + y, popup_width, popup_height)
        } else {
            area
        };

        frame.render_widget(ratatui::widgets::Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Select Log Source")
            .style(Style::default().bg(Color::Rgb(20, 20, 20)));

        frame.render_widget(block.clone(), area);

        let inner_area = block.inner(area);

        let items: Vec<Line> = self
            .selection_state
            .candidates
            .iter()
            .enumerate()
            .map(|(i, item)| match item {
                SelectionItem::Header(title) => Line::from(Span::styled(
                    format!("--- {} ---", title),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                SelectionItem::SubHeader(title) => Line::from(Span::styled(
                    format!("  {}", title),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )),
                SelectionItem::Option { label, .. } => {
                    let mut style = if i == self.selection_state.selected_idx {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    // Bold "Today" even if not selected
                    if label.contains("Today") {
                        style = style.add_modifier(Modifier::BOLD);
                    }

                    let prefix = if i == self.selection_state.selected_idx {
                        "  > "
                    } else {
                        "    "
                    };
                    Line::from(vec![Span::styled(format!("{}{}", prefix, label), style)])
                }
            })
            .collect();

        // Calculate scroll to keep selected item visible - simple centering logic or ensure visible
        let list_height = inner_area.height as usize;
        let scroll = if self.selection_state.selected_idx >= list_height {
            self.selection_state.selected_idx - list_height + 1
        } else {
            0
        } as u16;

        let content = Paragraph::new(items).scroll((scroll, 0));

        frame.render_widget(content, inner_area);
    }

    /// Handles keyboard input
    pub fn handle_input(&mut self, key: KeyEvent) -> Result<LogInputResult> {
        if self.selection_state.active {
            return self.handle_selection_input(key);
        }

        // Global navigation keys
        match key.code {
            KeyCode::Tab => {
                // Cycle active panel
                if !self.panels.is_empty() {
                    self.active_panel_idx = (self.active_panel_idx + 1) % self.panels.len();
                }
                return Ok(LogInputResult::Handled);
            }
            KeyCode::Char('w') => {
                // Close active panel if there's more than one
                if self.panels.len() > 1 {
                    self.panels.remove(self.active_panel_idx);
                    if self.active_panel_idx >= self.panels.len() {
                        self.active_panel_idx = self.panels.len().saturating_sub(1);
                    }
                    return Ok(LogInputResult::Handled);
                }
                // If only 1 panel, let standard Esc handle exit
            }
            _ => {}
        }

        // Delegate to active panel
        if let Some(panel) = self.panels.get_mut(self.active_panel_idx) {
            return panel.handle_input(key);
        }

        Ok(LogInputResult::Ignored)
    }

    /// Add a new panel
    pub fn add_panel(&mut self, path: PathBuf) -> Result<()> {
        if self.panels.len() < 4 {
            self.panels.push(SingleLogView::new(path)?);
            // Switch focus to new panel
            self.active_panel_idx = self.panels.len() - 1;
        }
        Ok(())
    }

    /// Replaces the panel at the given index with a new log file
    pub fn replace_panel(&mut self, index: usize, path: PathBuf) -> Result<()> {
        if index < self.panels.len() {
            self.panels[index] = SingleLogView::new(path)?;
        }
        Ok(())
    }

    /// Returns the index of the active panel
    pub fn active_panel_idx(&self) -> usize {
        self.active_panel_idx
    }

    pub fn start_log_selection(&mut self, candidates: Vec<SelectionItem>, mode: SelectionMode) {
        self.selection_state.active = true;
        self.selection_state.candidates = candidates;
        self.selection_state.mode = mode;

        // Find first selectable item index (skip headers)
        self.selection_state.selected_idx = 0;
        self.select_next_selectable(true); // Search forward including current
    }

    /// Helper to find next selectable index
    fn select_next_selectable(&mut self, include_current: bool) {
        let start = if include_current {
            self.selection_state.selected_idx
        } else {
            self.selection_state.selected_idx + 1
        };
        for i in start..self.selection_state.candidates.len() {
            if matches!(
                self.selection_state.candidates[i],
                SelectionItem::Option { .. }
            ) {
                self.selection_state.selected_idx = i;
                return;
            }
        }
        // Wrap around or fallback?
        // If we are at end, maybe check from 0?
    }

    /// Helper to find previous selectable index
    fn select_prev_selectable(&mut self) {
        if self.selection_state.selected_idx == 0 {
            return;
        }
        for i in (0..self.selection_state.selected_idx).rev() {
            if matches!(
                self.selection_state.candidates[i],
                SelectionItem::Option { .. }
            ) {
                self.selection_state.selected_idx = i;
                return;
            }
        }
    }

    fn handle_selection_input(&mut self, key: KeyEvent) -> Result<LogInputResult> {
        match key.code {
            KeyCode::Esc => {
                self.selection_state.active = false;
                Ok(LogInputResult::Handled)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_prev_selectable();
                Ok(LogInputResult::Handled)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.select_next_selectable(false);
                Ok(LogInputResult::Handled)
            }
            KeyCode::Enter => {
                if let Some(SelectionItem::Option { path, .. }) = self
                    .selection_state
                    .candidates
                    .get(self.selection_state.selected_idx)
                {
                    match self.selection_state.mode {
                        SelectionMode::Replace(idx) => {
                            // If empty, treat replace as add (start state)
                            if self.panels.is_empty() {
                                self.add_panel(path.clone())?;
                            } else {
                                self.replace_panel(idx, path.clone())?;
                            }
                        }
                        SelectionMode::Add => {
                            self.add_panel(path.clone())?;
                        }
                    }
                }
                self.selection_state.active = false;
                Ok(LogInputResult::Handled)
            }
            _ => Ok(LogInputResult::Ignored),
        }
    }

    /// Returns the paths of logs being viewed (for knowing what's open)
    pub fn log_paths(&self) -> Vec<PathBuf> {
        self.panels.iter().map(|p| p.log_path.clone()).collect()
    }

    /// Refreshes log content for all panels
    pub fn refresh(&mut self) -> Result<bool> {
        let mut any_updated = false;
        for panel in &mut self.panels {
            if panel.refresh()? {
                any_updated = true;
            }
        }
        Ok(any_updated)
    }
}

/// Single log panel viewer
/// Handles display logic for a single log file
pub struct SingleLogView {
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

impl SingleLogView {
    /// Creates a new log viewer for the specified file
    /// Loads the file content and prepares it for display
    pub fn new(log_path: PathBuf) -> Result<Self> {
        crate::debug!(
            "[LogViewer] SingleLogView::new() called for path: {:?}",
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
                                       // Footer removed from here, handled by parent
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

            // Adjust viewport to ensure cursor is visible
            let visible_height = content_chunks[0].height.saturating_sub(2) as usize;
            self.viewport
                .adjust_viewport(visible_height, self.total_lines);

            // Render log content on left
            self.render_content(frame, content_chunks[0], is_active);

            // Render JSON panel on right
            self.render_json_panel(frame, content_chunks[1]);
        } else {
            // Adjust viewport to ensure cursor is visible
            let visible_height = chunks[1].height.saturating_sub(2) as usize;
            self.viewport
                .adjust_viewport(visible_height, self.total_lines);

            // Render log content full width
            self.render_content(frame, chunks[1], is_active);
        }

        // Footer rendering removed from here
    }

    /// Renders the header showing file name and current position
    fn render_header(&self, frame: &mut Frame, area: Rect, is_active: bool) {
        let filename = self
            .log_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown");

        // Attempt slightly smarter formatting
        // Assuming {project}_{app}_...
        let parts: Vec<&str> = filename.splitn(3, '_').collect();
        let display_title = if parts.len() >= 3 {
            let proj = parts[0];
            let app = parts[1];
            let rest = parts[2].replace(".log", "");
            // Replace dashes or underscores in date part if needed, but usually ISO date is fine
            format!("{} - {} [{}]", proj, app, rest)
        } else {
            filename.replace(".log", "")
        };

        let current_line = self.viewport.cursor_line + 1;
        let title = format!(
            "{} (Line {}/{})",
            display_title, current_line, self.total_lines
        );

        // Active panel gets a distinct border color (Green or Cyan usually implies activity)
        let border_color = if is_active {
            Color::Green
        } else {
            Color::DarkGray
        };

        let header = Paragraph::new(title)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color)),
            )
            .style(Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 0)));

        frame.render_widget(header, area);
    }

    /// Renders the main log content with line numbers
    fn render_content(&self, frame: &mut Frame, area: Rect, is_active: bool) {
        // Build text with ALL lines
        let mut text = Text::default();

        for (i, log_line) in self.content.iter().enumerate() {
            // Determine the background color for this line
            let bg_color = if i == self.viewport.cursor_line {
                if is_active {
                    Color::Rgb(40, 40, 60) // Active cursor
                } else {
                    Color::Rgb(20, 20, 30) // Inactive but selected line
                }
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
        let border_color = if is_active {
            Color::White // Content border for active
        } else {
            Color::DarkGray
        };

        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
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

    /// Returns status text for the unified footer
    pub fn get_status_text(&self) -> String {
        if self.search.active {
            // Show search input with match count
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

    /// Handles input when in search mode
    fn handle_search_input(&mut self, key: KeyEvent) -> Result<LogInputResult> {
        match key.code {
            KeyCode::Backspace => {
                // Remove last character from search query
                self.search.query.pop();
                if let Some(idx) = self.search.perform_search(&self.content) {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            KeyCode::Enter | KeyCode::Esc => {
                // Exit search mode
                self.search.active = false;
                Ok(LogInputResult::Handled)
            }
            KeyCode::Char('n') if key.modifiers.is_empty() => {
                // Next search result
                if let Some(idx) = self.search.next_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
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
                Ok(LogInputResult::Handled)
            }
            KeyCode::Char(c) => {
                // Add character to search query
                self.search.query.push(c);
                if let Some(idx) = self.search.perform_search(&self.content) {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            _ => Ok(LogInputResult::Ignored),
        }
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
                self.viewport.cursor_line = 0;
                Ok(LogInputResult::Handled)
            }
            // Jump to bottom
            KeyCode::End | KeyCode::Char('G') => {
                self.viewport.cursor_line = self.content.len().saturating_sub(1);
                Ok(LogInputResult::Handled)
            }
            // Enter search mode
            KeyCode::Char('/') => {
                self.search.active = true;
                self.search.query.clear();
                self.search.results.clear();
                self.search.current_idx = 0;
                Ok(LogInputResult::Handled)
            }
            // 'n' for Add Panel -- CHANGED FROM NEXT SEARCH RESULT
            KeyCode::Char('n') => Ok(LogInputResult::RequestAddPanel),
            // Previous search result
            KeyCode::Char('N') => {
                // Reuse N for next result in non-search mode?
                // Or keep it for previous result if search is active (it's not here)
                // Let's implement Next Search Result on another key if needed
                // Currently n is hijacked for panel.
                // We'll leave N for Previous Search Result if someone used / before
                if let Some(idx) = self.search.previous_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
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
