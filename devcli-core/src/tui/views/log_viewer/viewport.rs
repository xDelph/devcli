#[derive(Default)]
pub struct ViewportState {
    /// Current cursor position (which line is highlighted)
    pub cursor_line: usize,
    /// Top line of the viewport (first visible line)
    pub viewport_top: usize,
    /// Current horizontal scroll offset (character index)
    pub horizontal_offset: usize,
}

impl ViewportState {
    pub fn new(total_lines: usize) -> Self {
        // Start at the bottom of the file (most recent logs)
        let cursor_line = total_lines.saturating_sub(1);
        Self {
            cursor_line,
            viewport_top: 0, // Adjusted on first render
            horizontal_offset: 0,
        }
    }

    /// Moves the cursor down (toward newer logs)
    pub fn scroll_down(&mut self, lines: usize, total_lines: usize) {
        for _ in 0..lines {
            if self.cursor_line >= total_lines.saturating_sub(1) {
                break;
            }
            self.cursor_line += 1;
        }
    }

    /// Moves the cursor up (toward older logs)
    pub fn scroll_up(&mut self, lines: usize) {
        for _ in 0..lines {
            if self.cursor_line == 0 {
                break;
            }
            self.cursor_line -= 1;
        }
    }

    /// Adjusts the viewport to ensure the cursor is visible
    pub fn adjust_viewport(&mut self, visible_height: usize, total_lines: usize) {
        let viewport_bottom = self.viewport_top + visible_height.saturating_sub(1);

        if self.cursor_line < self.viewport_top {
            self.viewport_top = self.cursor_line;
        } else if self.cursor_line > viewport_bottom {
            self.viewport_top = self
                .cursor_line
                .saturating_sub(visible_height.saturating_sub(1));
        }

        let max_viewport_top = total_lines.saturating_sub(visible_height);
        if self.viewport_top > max_viewport_top && total_lines >= visible_height {
            self.viewport_top = max_viewport_top;
        }
    }
}
