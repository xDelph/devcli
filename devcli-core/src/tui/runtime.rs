use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{backend::Backend, Terminal};
use std::time::{Duration, Instant};

use super::app::TuiApp;

/// The TUI Runtime
/// Manages the main event loop, terminal updates, and input handling
pub struct Runtime {
    /// Timestamp of the last applied mouse-wheel scroll, used to debounce the
    /// burst of scroll events that smooth trackpads / mice emit per notch.
    last_wheel_scroll: Option<Instant>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            last_wheel_scroll: None,
        }
    }

    /// Rate-limits wheel scrolling so bursts of events (smooth trackpads and
    /// some mice send several scroll events per notch) don't turn into an
    /// uncontrollable fast scroll. Returns true when the scroll step may be
    /// applied; rejects everything within `WHEEL_MIN_INTERVAL` of the last one.
    fn wheel_scroll_allowed(&mut self, now: Instant) -> bool {
        const WHEEL_MIN_INTERVAL_MS: u64 = 60;
        match self.last_wheel_scroll {
            Some(prev)
                if now.duration_since(prev) < Duration::from_millis(WHEEL_MIN_INTERVAL_MS) =>
            {
                false
            }
            _ => {
                self.last_wheel_scroll = Some(now);
                true
            }
        }
    }

    /// Runs the main event loop
    pub fn run<B: Backend>(&mut self, app: &mut TuiApp, terminal: &mut Terminal<B>) -> Result<()>
    where
        <B as Backend>::Error: Send + Sync + 'static,
    {
        while !app.should_quit() {
            // Check for updates (command requests, status updates, etc.)
            app.tick()?;

            // Clear terminal if needed (for view transitions)
            if app.needs_clear() {
                terminal.clear()?;
                app.clear_needs_clear();
            }

            // Only render if something changed (dirty flag optimization)
            if app.needs_redraw() {
                let render_start = Instant::now();
                terminal.draw(|f| app.render(f))?;
                app.on_render_complete(render_start);
            }

            // Wait for an event with a timeout
            if event::poll(Duration::from_millis(100))? {
                let now = Instant::now();
                match event::read()? {
                    Event::Key(key) => {
                        app.handle_key_event(key)?;
                        app.set_needs_redraw(true);
                    }
                    Event::Mouse(mouse) => {
                        // Translate wheel scrolling into arrow keys so every view
                        // that already handles Up/Down scrolls with the mouse too.
                        // Debounced: smooth trackpads emit a *burst* of scroll
                        // events per notch — without a rate limit that becomes
                        // an uncontrollable fast scroll.
                        if let Some(key) = mouse_scroll_key(&mouse) {
                            if self.wheel_scroll_allowed(now) {
                                app.handle_key_event(key)?;
                                app.set_needs_redraw(true);
                            }
                        }
                    }
                    Event::Resize(_, _) => {
                        // Terminal was resized - force a redraw
                        terminal.clear()?;
                        app.set_needs_redraw(true);
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// Maps a mouse wheel event to the equivalent arrow key.
///
/// Returns `None` for clicks / drags / moves so the app keeps ignoring them
/// (only wheel scrolling is captured).
fn mouse_scroll_key(mouse: &MouseEvent) -> Option<KeyEvent> {
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
        MouseEventKind::ScrollDown => Some(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        _ => None,
    }
}
