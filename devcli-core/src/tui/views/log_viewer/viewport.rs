/// What the vertical scroll should do on the next render.
///
/// The actual pixel decision is made in `render_content` where the
/// text-wrapping layout (visual rows) is known; this enum only carries the
/// *intent* set by input handlers.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAnchor {
    /// Pin the cursor to the FIRST visible line (standard scroll-up).
    Top,
    /// Pin the cursor to the LAST visible line (standard scroll-down /
    /// following the newest logs).
    Bottom,
    /// No explicit anchor: keep the current view and only reveal the cursor
    /// if it leaves the visible range (jumps, search, layout changes).
    #[default]
    None,
}

#[derive(Default)]
pub struct ViewportState {
    /// Current cursor position (which line is highlighted)
    pub cursor_line: usize,
    /// Current horizontal scroll offset (character index)
    pub horizontal_offset: usize,
    /// First visible *visual* row (text-wrapping aware).
    ///
    /// This is the single source of truth for vertical scrolling — it is
    /// re-derived from `cursor_line` + `ScrollAnchor` on every render, so it
    /// can never go stale (window resize, panel close, wrapped lines…).
    pub last_visual_scroll: usize,
    /// Scroll intent for the next render.
    pub anchor: ScrollAnchor,
}

impl ViewportState {
    pub fn new(total_lines: usize) -> Self {
        Self {
            // Start at the bottom of the file (most recent logs)
            cursor_line: total_lines.saturating_sub(1),
            horizontal_offset: 0,
            last_visual_scroll: 0,
            anchor: ScrollAnchor::Bottom,
        }
    }

    /// Moves the cursor down (toward newer logs) and asks the renderer to pin
    /// it to the last visible line — standard scroll-down.
    pub fn scroll_down(&mut self, lines: usize, total_lines: usize) {
        self.cursor_line = self
            .cursor_line
            .saturating_add(lines)
            .min(total_lines.saturating_sub(1));
        self.anchor = ScrollAnchor::Bottom;
    }

    /// Moves the cursor up (toward older logs) and asks the renderer to pin
    /// it to the first visible line — standard scroll-up.
    pub fn scroll_up(&mut self, lines: usize) {
        self.cursor_line = self.cursor_line.saturating_sub(lines);
        self.anchor = ScrollAnchor::Top;
    }

    /// Jumps to the first line; the renderer reveals it (no explicit anchor).
    pub fn jump_top(&mut self) {
        self.cursor_line = 0;
        self.anchor = ScrollAnchor::None;
    }

    /// Jumps to the last line and pins the view to the bottom (follow logs).
    pub fn jump_bottom(&mut self, total_lines: usize) {
        self.cursor_line = total_lines.saturating_sub(1);
        self.anchor = ScrollAnchor::Bottom;
    }

    /// Keeps following the newest lines (called after the log is refreshed).
    pub fn follow_bottom(&mut self, total_lines: usize) {
        self.cursor_line = total_lines.saturating_sub(1);
        self.anchor = ScrollAnchor::Bottom;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_bottom_and_follows() {
        let mut vp = ViewportState::new(10);
        assert_eq!(vp.cursor_line, 9);
        assert_eq!(vp.anchor, ScrollAnchor::Bottom);

        vp.follow_bottom(15);
        assert_eq!(vp.cursor_line, 14);
        assert_eq!(vp.anchor, ScrollAnchor::Bottom);
    }

    #[test]
    fn scroll_up_clamps_at_zero_and_anchors_top() {
        let mut vp = ViewportState::new(10); // cursor 9
        vp.scroll_up(3);
        assert_eq!(vp.cursor_line, 6);
        assert_eq!(vp.anchor, ScrollAnchor::Top);

        vp.scroll_up(100);
        assert_eq!(vp.cursor_line, 0);
        assert_eq!(vp.anchor, ScrollAnchor::Top);
    }

    #[test]
    fn scroll_down_clamps_at_last_line_and_anchors_bottom() {
        let mut vp = ViewportState::new(10); // cursor 9
        vp.scroll_up(9); // cursor 0
        vp.scroll_down(4, 10);
        assert_eq!(vp.cursor_line, 4);
        assert_eq!(vp.anchor, ScrollAnchor::Bottom);

        vp.scroll_down(100, 10);
        assert_eq!(vp.cursor_line, 9);
        assert_eq!(vp.anchor, ScrollAnchor::Bottom);
    }

    #[test]
    fn jumps_set_cursor_and_anchor() {
        let mut vp = ViewportState::new(10);
        vp.jump_top();
        assert_eq!(vp.cursor_line, 0);
        assert_eq!(vp.anchor, ScrollAnchor::None);

        vp.jump_bottom(10);
        assert_eq!(vp.cursor_line, 9);
        assert_eq!(vp.anchor, ScrollAnchor::Bottom);
    }

    #[test]
    fn empty_file_starts_at_zero() {
        let vp = ViewportState::new(0);
        assert_eq!(vp.cursor_line, 0);
    }
}
