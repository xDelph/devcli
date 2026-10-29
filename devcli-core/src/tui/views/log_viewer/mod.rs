// Log Viewer View - Full-screen log file viewer with beautification
// Provides scrolling, JSON prettification, syntax highlighting, and search functionality
// Implements lazy loading for efficient handling of large log files

pub mod app_color_manager;
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
    app_color_manager::AppColorManager,
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
    /// Something went wrong that the user should see (shown in the error bar).
    Error(String),
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
    /// Manages color assignment for app names across all panels
    color_manager: AppColorManager,
}

impl LogViewerView {
    /// Creates a new log viewer with initial log paths
    pub fn new(log_paths: Vec<PathBuf>) -> Result<Self> {
        let mut color_manager = AppColorManager::new();
        let mut panels = Vec::new();

        for path in log_paths {
            // Extract app name and assign color
            let app_name = AppColorManager::extract_app_name_from_path(&path);
            let app_color = color_manager.get_color_for_app(&app_name);

            panels.push(SingleLogView::new(path, app_name, app_color)?);
        }

        if panels.is_empty() {
            // Should ideally not happen or handle empty state, but for now let's assume at least one path
        }

        Ok(Self {
            panels,
            active_panel_idx: 0,
            selection_state: LogSelectionState::default(),
            color_manager,
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
            KeyCode::Char('w') if self.panels.len() > 1 => {
                // Close active panel if there's more than one
                self.panels.remove(self.active_panel_idx);
                if self.active_panel_idx >= self.panels.len() {
                    self.active_panel_idx = self.panels.len().saturating_sub(1);
                }

                // Reset viewport state for remaining panels to handle layout change.
                // The renderer re-normalizes the scroll from the cursor, so we
                // only need to keep the cursor inside the content.
                for panel in &mut self.panels {
                    if panel.viewport.cursor_line + 1 >= panel.total_lines {
                        panel.viewport.cursor_line = panel.total_lines.saturating_sub(1);
                    }
                }

                return Ok(LogInputResult::Handled);
            }
            // If only 1 panel, let standard Esc handle exit
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
            // Extract app name and assign color
            let app_name = AppColorManager::extract_app_name_from_path(&path);
            let app_color = self.color_manager.get_color_for_app(&app_name);

            self.panels
                .push(SingleLogView::new(path, app_name, app_color)?);
            // Switch focus to new panel
            self.active_panel_idx = self.panels.len() - 1;
        }
        Ok(())
    }

    /// Replaces the panel at the given index with a new log file
    pub fn replace_panel(&mut self, index: usize, path: PathBuf) -> Result<()> {
        if index < self.panels.len() {
            // Extract app name and assign color
            let app_name = AppColorManager::extract_app_name_from_path(&path);
            let app_color = self.color_manager.get_color_for_app(&app_name);

            self.panels[index] = SingleLogView::new(path, app_name, app_color)?;
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
                    let path = path.clone();
                    let opened = match self.selection_state.mode {
                        // If empty, treat replace as add (start state)
                        SelectionMode::Replace(_) if self.panels.is_empty() => {
                            self.add_panel(path.clone())
                        }
                        SelectionMode::Replace(idx) => self.replace_panel(idx, path.clone()),
                        SelectionMode::Add => self.add_panel(path.clone()),
                    };
                    // A log that cannot be opened (deleted since it was listed,
                    // unreadable) used to propagate up and quit the whole TUI.
                    if let Err(e) = opened {
                        self.selection_state.active = false;
                        return Ok(LogInputResult::Error(format!(
                            "Cannot open {}: {e}",
                            path.display()
                        )));
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
    ///
    /// A panel that fails to refresh no longer stops the ones after it.
    pub fn refresh(&mut self) -> Result<bool> {
        let mut any_updated = false;
        for panel in &mut self.panels {
            match panel.refresh() {
                Ok(updated) => any_updated |= updated,
                Err(e) => {
                    tracing::debug!(path = %panel.log_path.display(), error = %e, "log refresh failed");
                }
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
    /// Syntax highlighting resources (shared by every panel)
    highlighter: &'static SyntaxHighlighter,
    /// Total number of lines in the file (for display)
    total_lines: usize,
    /// Whether to show the JSON beautifier panel
    show_json_panel: bool,
    /// Last file size to detect changes
    last_file_size: u64,
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

            // Viewport adjustment disabled - render_content handles scroll with wrapping
            // let visible_height = content_chunks[0].height.saturating_sub(2) as usize;
            // self.viewport
            //     .adjust_viewport(visible_height, self.total_lines);

            // Render log content on left
            self.render_content(frame, content_chunks[0], is_active);

            // Render JSON panel on right
            self.render_json_panel(frame, content_chunks[1]);
        } else {
            // Viewport adjustment disabled - render_content handles scroll with wrapping
            // let visible_height = chunks[1].height.saturating_sub(2) as usize;
            // self.viewport
            //     .adjust_viewport(visible_height, self.total_lines);

            // Render log content full width
            self.render_content(frame, chunks[1], is_active);
        }

        // Footer rendering removed from here
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
        if self.search.active {
            // Show search input with match count and navigation help
            let match_info = if !self.search.results.is_empty() {
                format!(
                    " [{} matches] ({}/{}) - ↑↓/jk/Enter: Navigate  Esc: Exit",
                    self.search.results.len(),
                    self.search.current_idx + 1,
                    self.search.results.len()
                )
            } else if !self.search.query.is_empty() {
                " [No matches] - Esc: Exit".to_string()
            } else {
                " - Type to search, Esc: Exit".to_string()
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
            KeyCode::Enter => {
                // Cycle to next search result
                if let Some(idx) = self.search.next_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            KeyCode::Esc => {
                // Exit search mode and clear all highlighting
                self.search.active = false;
                self.search.clear_results();
                Ok(LogInputResult::Handled)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                // Next search result (Down arrow or j)
                if let Some(idx) = self.search.next_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                // Previous search result (Up arrow or k)
                if let Some(idx) = self.search.previous_result() {
                    self.viewport.cursor_line = idx;
                }
                Ok(LogInputResult::Handled)
            }
            KeyCode::Char(c) if c.is_ascii() => {
                // Add character to search query (allow all printable ASCII characters including 'n')
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

    /// Load the whole file from scratch.
    fn reload(&mut self) -> Result<()> {
        let bytes = std::fs::read(&self.log_path)?;
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
        Ok(())
    }

    /// Refreshes the log file content if it has changed
    /// Returns true if the content was updated
    ///
    /// Appended output is read and parsed incrementally; the whole file used to
    /// be re-read, re-parsed and re-highlighted every time it grew (checked
    /// every 100 ms), which is constant work proportional to the log size for
    /// any app that is logging.
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

        if current_size < self.complete_bytes {
            // Truncated (rotated): start over.
            self.reload()?;
        } else {
            use std::io::{Read, Seek, SeekFrom};

            let mut file = std::fs::File::open(&self.log_path)?;
            file.seek(SeekFrom::Start(self.complete_bytes))?;
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
            // Lines past the old end (and a re-parsed tail) need measuring.
            self.row_cache.truncate(first_line_number.saturating_sub(1));
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

    #[test]
    fn selecting_a_vanished_log_reports_instead_of_failing() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let dir = tempfile::TempDir::new().unwrap();
        let mut viewer = LogViewerView::new(vec![]).unwrap();
        viewer.start_log_selection(
            vec![SelectionItem::Option {
                label: "gone".to_string(),
                path: dir.path().join("gone.log"),
            }],
            SelectionMode::Add,
        );
        let result = viewer
            .handle_input(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .expect("must not fail the TUI");
        assert!(
            matches!(result, LogInputResult::Error(ref m) if m.contains("gone.log")),
            "{result:?}"
        );
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
}
