use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::TuiApp;
use crate::tui::state::ViewType;

/// Handles keyboard input events
/// Routes events based on the current view and popup state
pub fn handle_key_event(app: &mut TuiApp, key: KeyEvent) -> Result<()> {
    // crate::debug!("[App] Key event received: {:?}", key);
    // ALWAYS handle Ctrl+C first - quit immediately
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return Ok(());
    }

    // If help overlay is visible, handle it first: the scrolling keys
    // scroll it, any other key closes it.
    if app.help_overlay.is_visible() {
        if !scroll_help(app, key) {
            app.help_overlay.hide();
        }
        return Ok(());
    }

    // If a popup is active, handle popup-specific input first
    if app.popup_manager.is_active() {
        return app.handle_popup_input(key);
    }

    // If there's an error message, the next key dismisses it - and still
    // does what it does (it is not swallowed: after "Path is required" the
    // next character typed goes into the form). Errors the key itself causes
    // are set after this, so they show.
    {
        let mut state = app
            .state
            .lock()
            .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
        if state.error_message.is_some() {
            state.error_message = None;
            // Don't return - let the key event continue to be processed
        }
    }

    // Get current view type by locking state briefly
    let current_view = {
        let state = app
            .state
            .lock()
            .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
        state.current_view.clone()
    };

    // View-specific handling - let views handle keys first
    let handled = match &current_view {
        ViewType::Main => app.handle_main_view_input(key)?,
        ViewType::LogViewer { .. } => app.handle_log_viewer_input(key)?,
    };

    // If the view handled the key, we're done
    if handled {
        return Ok(());
    }

    // Global shortcuts that work in any view (only if not handled by view)
    match key.code {
        // Show help overlay
        KeyCode::Char('?') => {
            app.help_overlay.toggle();
        }
        // Quit the application
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        // Go back to previous view
        KeyCode::Esc => {
            app.handle_back();
        }
        _ => {}
    }

    Ok(())
}

/// Scroll the help overlay for a scrolling key. Returns whether it was one.
fn scroll_help(app: &mut TuiApp, key: KeyEvent) -> bool {
    let help = &mut app.help_overlay;
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => help.scroll_up(1),
        KeyCode::Down | KeyCode::Char('j') => help.scroll_down(1),
        KeyCode::PageUp => help.scroll_up(10),
        KeyCode::PageDown => help.scroll_down(10),
        KeyCode::Home | KeyCode::Char('g') => help.scroll_up(u16::MAX),
        KeyCode::End | KeyCode::Char('G') => help.scroll_down(u16::MAX),
        _ => return false,
    }
    true
}

/// A mouse-wheel step, as the Up/Down key it maps to.
///
/// It scrolls whatever is shown - the help overlay, a popup, the view - and
/// nothing else: as a plain key press it used to close the help overlay and
/// dismiss the error bar.
pub fn handle_scroll_event(app: &mut TuiApp, key: KeyEvent) -> Result<()> {
    if app.help_overlay.is_visible() {
        scroll_help(app, key);
        return Ok(());
    }
    if app.popup_manager.is_active() {
        return app.handle_popup_input(key);
    }
    let current_view = {
        let state = app
            .state
            .lock()
            .map_err(|e| anyhow::anyhow!("State mutex poisoned: {}", e))?;
        state.current_view.clone()
    };
    match current_view {
        ViewType::Main => app.handle_main_view_input(key)?,
        ViewType::LogViewer { .. } => app.handle_log_viewer_input(key)?,
    };
    Ok(())
}
