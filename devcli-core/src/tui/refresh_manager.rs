use std::time::Instant;

/// Manages the dirty flag pattern for efficient rendering
/// Only redraws the UI when state changes or events occur
pub struct RefreshManager {
    /// Flag to track if UI needs redrawing
    needs_redraw: bool,
    /// Flag to clear terminal on next render (for view transitions)
    needs_clear: bool,
    /// Last render timestamp for performance tracking
    last_render: Instant,
}

impl RefreshManager {
    pub fn new() -> Self {
        Self {
            needs_redraw: true, // Initial render needed
            needs_clear: false,
            last_render: Instant::now(),
        }
    }

    /// Requests a redraw of the UI
    pub fn request_redraw(&mut self) {
        self.needs_redraw = true;
    }

    /// Requests a terminal clear on the next render
    pub fn request_clear(&mut self) {
        self.needs_clear = true;
        self.needs_redraw = true; // Clear implies redraw
    }

    /// Checks if a redraw is needed
    pub fn needs_redraw(&self) -> bool {
        self.needs_redraw
    }

    /// Checks if a terminal clear is needed
    pub fn needs_clear(&self) -> bool {
        self.needs_clear
    }

    /// Marks the render as complete
    pub fn on_render_complete(&mut self) {
        self.needs_redraw = false;
        self.needs_clear = false;
        self.last_render = Instant::now();
    }

    /// Returns the timestamp of the last render
    pub fn last_render(&self) -> Instant {
        self.last_render
    }
}

impl Default for RefreshManager {
    fn default() -> Self {
        Self::new()
    }
}
