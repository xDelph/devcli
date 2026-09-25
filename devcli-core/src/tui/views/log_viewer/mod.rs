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
    viewport::{ScrollAnchor, ViewportState},
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
            KeyCode::Char('w') => {
                // Close active panel if there's more than one
                if self.panels.len() > 1 {
                    self.panels.remove(self.active_panel_idx);
                    if self.active_panel_idx >= self.panels.len() {
                        self.active_panel_idx = self.panels.len().saturating_sub(1);
                    }

                    // Reset viewport state for remaining panels to handle layout change
                    // When going from multi-panel to single-panel, the viewport needs adjustment
                    for panel in &mut self.panels {
                        // If the cursor was at the bottom, keep following the latest logs;
                        // otherwise just reveal the cursor in the new layout.
                        if panel.viewport.cursor_line + 1 >= panel.total_lines {
                            panel.viewport.follow_bottom(panel.total_lines);
                        } else {
                            panel.viewport.anchor = ScrollAnchor::None;
                        }
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
            app_name,
            app_color,
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

        // Parse filename: {project}_{app}_{context}_{date}.log
        let parts: Vec<&str> = filename.splitn(4, '_').collect();
        let (project, context_date) = if parts.len() >= 4 {
            let proj = parts[0];
            let context = parts[2];
            let date = parts[3].replace(".log", "");
            (proj.to_string(), format!("{} [{}]", context, date))
        } else {
            ("Unknown".to_string(), filename.replace(".log", ""))
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

    /// Calculates how many visual rows a log line will occupy when wrapped
    fn calculate_wrapped_rows(&self, log_line: &LogLine, available_width: usize) -> usize {
        let prefix_width = 8; // "12345 | " format
        let app_name_width = self.app_name.len() + 3; // "[appName] " format
        let content_width = log_line
            .formatted
            .iter()
            .map(|span| span.content.len())
            .sum::<usize>();
        let total_line_width = prefix_width + app_name_width + content_width;

        if total_line_width == 0 {
            1
        } else {
            total_line_width.div_ceil(available_width).max(1)
        }
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

    /// Renders the main log content with line numbers and text wrapping
    ///
    /// This method handles text wrapping by:
    /// 1. Calculating visual row positions for each logical line
    /// 2. Determining scroll offset to keep cursor visible
    /// 3. Rendering all lines with proper highlighting and wrapping
    fn render_content(&mut self, frame: &mut Frame, area: Rect, is_active: bool) {
        let available_width = area.width.saturating_sub(2) as usize;
        let visible_height = area.height.saturating_sub(2) as usize;

        // Calculate visual row position for each logical line
        // This accounts for text wrapping - a long line may span multiple visual rows
        let mut visual_row_positions: Vec<usize> = Vec::new();
        let mut current_visual_row = 0;

        for log_line in &self.content {
            visual_row_positions.push(current_visual_row);
            let wrapped_rows = self.calculate_wrapped_rows(log_line, available_width);
            current_visual_row += wrapped_rows;
        }

        // A shrunk/rotated file can leave the cursor past the content — never
        // let the selection point into the void (keeps the highlight visible).
        let max_cursor = self.content.len().saturating_sub(1);
        if self.viewport.cursor_line > max_cursor {
            self.viewport.cursor_line = max_cursor;
        }

        // Get visual row position for cursor
        let total_visual_rows = current_visual_row;
        let cursor_visual_row = visual_row_positions
            .get(self.viewport.cursor_line)
            .copied()
            .unwrap_or(0);

        // Standard anchored scrolling, decided here because the wrapped visual
        // layout is only known at render time:
        // - scroll up (Top): cursor becomes the FIRST visible line
        // - scroll down (Bottom): cursor becomes the LAST visible line
        // - otherwise: keep the current position, only reveal the cursor if it
        //   leaves the visible range (jumps, search, layout changes)
        // Always clamped, so the first/last lines are always reachable and the
        // view never scrolls past the content.
        let anchor = std::mem::take(&mut self.viewport.anchor);
        let max_scroll = total_visual_rows.saturating_sub(visible_height);
        let scroll_offset = if total_visual_rows <= visible_height {
            // Everything fits on screen - never hide the first lines
            0
        } else {
            match anchor {
                ScrollAnchor::Top => cursor_visual_row.min(max_scroll),
                ScrollAnchor::Bottom => (cursor_visual_row + 1)
                    .saturating_sub(visible_height)
                    .min(max_scroll),
                ScrollAnchor::None => {
                    let current = self.viewport.last_visual_scroll.min(max_scroll);
                    if cursor_visual_row < current {
                        cursor_visual_row
                    } else if cursor_visual_row >= current + visible_height {
                        (cursor_visual_row + 1)
                            .saturating_sub(visible_height)
                            .min(max_scroll)
                    } else {
                        current
                    }
                }
            }
        };

        // Remember scroll position for next frame
        self.viewport.last_visual_scroll = scroll_offset;

        // Build text with ALL lines
        let mut text = Text::default();

        for (i, log_line) in self.content.iter().enumerate() {
            let bg_color = if i == self.viewport.cursor_line {
                if is_active {
                    Color::Rgb(40, 40, 60)
                } else {
                    Color::Rgb(20, 20, 30)
                }
            } else {
                Color::Rgb(0, 0, 0)
            };

            let line_num_str = format!("{:>5} | ", log_line.line_number);
            let line_num_span = if self.search.is_current_result(i) {
                // Current search result gets bright yellow with arrow
                let cursor_str = format!("{:>5} ▶ ", log_line.line_number);
                Span::styled(
                    cursor_str,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else if self.search.is_match(i) {
                // Other search matches get dimmer yellow
                Span::styled(
                    line_num_str.clone(),
                    Style::default()
                        .fg(Color::Rgb(200, 200, 100))
                        .add_modifier(Modifier::BOLD),
                )
            } else if i == self.viewport.cursor_line {
                let cursor_str = format!("{:>5} > ", log_line.line_number);
                Span::styled(cursor_str, Style::default().fg(Color::Cyan))
            } else {
                Span::styled(line_num_str.clone(), Style::default().fg(Color::DarkGray))
            };

            let mut spans = vec![line_num_span];

            // Add colored app name prefix: [appName]
            let app_name_span = Span::styled(
                format!("[{}] ", self.app_name),
                Style::default()
                    .fg(self.app_color)
                    .add_modifier(Modifier::BOLD)
                    .bg(bg_color),
            );
            spans.push(app_name_span);

            // Apply search highlighting if this line has matches
            let content_spans = if let Some(match_ranges) = self.search.get_match_ranges(i) {
                self.apply_search_highlighting(
                    &log_line.formatted,
                    match_ranges,
                    bg_color,
                    self.search.is_current_result(i),
                )
            } else {
                // No search matches, just apply background color
                log_line
                    .formatted
                    .iter()
                    .map(|s| {
                        let mut style = s.style;
                        style.bg = Some(bg_color);
                        Span::styled(s.content.clone(), style)
                    })
                    .collect()
            };

            spans.extend(content_spans);

            // Calculate the current line width to add padding for full-width background
            let current_width: usize = spans.iter().map(|span| span.content.len()).sum();
            let remaining_width = available_width.saturating_sub(current_width);

            // Add padding span to fill the remaining width with background color
            if remaining_width > 0 {
                spans.push(Span::styled(
                    " ".repeat(remaining_width),
                    Style::default().bg(bg_color),
                ));
            }

            let line = Line::from(spans).style(Style::default().bg(bg_color));
            text.lines.push(line);
        }

        let border_color = if is_active {
            Color::White
        } else {
            Color::DarkGray
        };

        // Use wrapping - scroll by visual rows
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
                    .style(Style::default().bg(Color::Rgb(0, 0, 0))),
            )
            .style(Style::default().bg(Color::Rgb(0, 0, 0)))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((scroll_offset as u16, 0));

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

        // Shrunk/rotated file: clamp a cursor that fell past the new content
        // and re-reveal the closest line so the selection stays visible.
        if self.viewport.cursor_line >= new_total_lines {
            self.viewport.cursor_line = new_total_lines.saturating_sub(1);
            self.viewport.anchor = ScrollAnchor::None;
        }

        // If we were at the bottom, stay at the bottom (auto-scroll)
        if was_at_bottom && new_total_lines > 0 {
            self.viewport.follow_bottom(new_total_lines);
        }

        Ok(true)
    }
}
