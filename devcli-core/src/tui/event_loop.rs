use anyhow::Result;
use crossterm::event::{self, Event, KeyEvent};
use ratatui::{backend::Backend, Terminal};
use std::time::Duration;

/// Trait for application event handling
/// Allows separating the event loop logic from the application state
pub trait AppEventHandler {
    /// Handle a key press event
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<()>;

    /// Handle a tick event (periodic update)
    fn handle_tick(&mut self) -> Result<()>;

    /// Handle a resize event
    fn handle_resize(&mut self, width: u16, height: u16) -> Result<()>;

    /// Check if the application should quit
    fn should_quit(&self) -> bool;

    /// Render the application
    fn render<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()>;
}

/// Runs the main event loop
/// Handles user input and renders the UI
pub fn run_event_loop<B: Backend, A: AppEventHandler>(
    app: &mut A,
    terminal: &mut Terminal<B>,
) -> Result<()> {
    while !app.should_quit() {
        // Handle periodic updates
        app.handle_tick()?;

        // Render the UI
        app.render(terminal)?;

        // Wait for an event with a timeout
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => app.handle_key_event(key)?,
                Event::Resize(w, h) => app.handle_resize(w, h)?,
                _ => {}
            }
        }
    }

    Ok(())
}
