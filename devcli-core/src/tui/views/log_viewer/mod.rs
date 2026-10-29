// Log Viewer View - Full-screen log file viewer with beautification
// Provides scrolling, JSON prettification, syntax highlighting, and search functionality
// Implements lazy loading for efficient handling of large log files

pub mod app_color_manager;
pub mod file_loader;
pub mod json_formatter;
pub mod search;
pub mod selector;
mod single_view;
pub mod syntax_highlighter;
pub mod viewport;

pub use single_view::SingleLogView;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::path::PathBuf;

use self::app_color_manager::AppColorManager;

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
                    panel.clamp_cursor();
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
