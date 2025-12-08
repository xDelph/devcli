use anyhow::Result;
use crossterm::event::{self, Event};
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
