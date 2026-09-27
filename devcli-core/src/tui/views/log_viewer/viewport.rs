#[derive(Default)]
pub struct ViewportState {
    /// Current cursor position (which line is highlighted)
    pub cursor_line: usize,
    /// Current horizontal scroll offset (character index)
    pub horizontal_offset: usize,
    /// First visible *visual* row (text-wrapping aware).
    ///
    /// This is the persistent scroll position: the renderer keeps it while the
    /// cursor stays inside the visible range, and re-anchors it when the cursor
    /// leaves or the layout changes. Always clamped, so it never goes stale.
    pub last_visual_scroll: usize,
}

impl ViewportState {
    pub fn new(total_lines: usize) -> Self {
        Self {
            // Start at the bottom of the file (most recent logs)
            cursor_line: total_lines.saturating_sub(1),
            horizontal_offset: 0,
            last_visual_scroll: 0,
        }
    }

    /// Moves the cursor down (toward newer logs), clamped to the last line.
    pub fn scroll_down(&mut self, lines: usize, total_lines: usize) {
        self.cursor_line = self
            .cursor_line
            .saturating_add(lines)
            .min(total_lines.saturating_sub(1));
    }

    /// Moves the cursor up (toward older logs), clamped to the first line.
    pub fn scroll_up(&mut self, lines: usize) {
        self.cursor_line = self.cursor_line.saturating_sub(lines);
    }

    /// Jumps to the first line.
    pub fn jump_top(&mut self) {
        self.cursor_line = 0;
    }

    /// Jumps to the last line.
    pub fn jump_bottom(&mut self, total_lines: usize) {
        self.cursor_line = total_lines.saturating_sub(1);
    }

    /// Clamps the cursor back into `[0, total_lines)` after the content shrank
    /// or was replaced (log rotation).
    pub fn clamp_cursor(&mut self, total_lines: usize) {
        self.cursor_line = self.cursor_line.min(total_lines.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_bottom() {
        let vp = ViewportState::new(10);
        assert_eq!(vp.cursor_line, 9);
    }

    #[test]
    fn scroll_up_clamps_at_zero() {
        let mut vp = ViewportState::new(10); // cursor 9
        vp.scroll_up(3);
        assert_eq!(vp.cursor_line, 6);

        vp.scroll_up(100);
        assert_eq!(vp.cursor_line, 0);
    }

    #[test]
    fn scroll_down_clamps_at_last_line() {
        let mut vp = ViewportState::new(10); // cursor 9
        vp.scroll_up(9); // cursor 0
        vp.scroll_down(4, 10);
        assert_eq!(vp.cursor_line, 4);

        vp.scroll_down(100, 10);
        assert_eq!(vp.cursor_line, 9);
    }

    #[test]
    fn jumps_and_clamp_cursor() {
        let mut vp = ViewportState::new(10);
        vp.jump_top();
        assert_eq!(vp.cursor_line, 0);

        vp.jump_bottom(10);
        assert_eq!(vp.cursor_line, 9);

        // simulate a file that shrunk to 3 lines
        vp.jump_bottom(10);
        vp.clamp_cursor(3);
        assert_eq!(vp.cursor_line, 2);
    }

    #[test]
    fn empty_file_starts_at_zero() {
        let vp = ViewportState::new(0);
        assert_eq!(vp.cursor_line, 0);
    }
}
