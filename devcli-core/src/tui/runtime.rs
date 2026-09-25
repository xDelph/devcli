use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{backend::Backend, Terminal};
use std::time::{Duration, Instant};

use super::app::TuiApp;

/// The TUI Runtime
/// Manages the main event loop, terminal updates, and input handling
pub struct Runtime;

impl Runtime {
    pub fn new() -> Self {
        Self
    }

    /// Runs the main event loop
    pub fn run<B: Backend>(&self, app: &mut TuiApp, terminal: &mut Terminal<B>) -> Result<()>
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
                match event::read()? {
                    Event::Key(key) => {
                        app.handle_key_event(key)?;
                        app.set_needs_redraw(true);
                    }
                    Event::Mouse(mouse) => {
                        // Translate wheel scrolling into arrow keys so every view
                        // that already handles Up/Down scrolls with the mouse too.
                        if let Some(key) = mouse_scroll_key(&mouse) {
                            app.handle_key_event(key)?;
                            app.set_needs_redraw(true);
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
