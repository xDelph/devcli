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

    // If help overlay is visible, handle it first
    // Any key except '?' closes the help overlay
    if app.help_overlay.is_visible() {
        match key.code {
            KeyCode::Char('?') => {
                app.help_overlay.toggle();
            }
            KeyCode::Esc | KeyCode::Enter => {
                app.help_overlay.hide();
            }
            _ => {
                // Any other key also closes help
                app.help_overlay.hide();
            }
        }
        return Ok(());
    }

    // If a popup is active, handle popup-specific input first
    if app.popup_manager.is_active() {
        return app.handle_popup_input(key);
    }

    // If there's an error message, any key dismisses it
    // This allows users to acknowledge and clear error messages
    {
        let mut state = app.state.lock().expect("Failed to lock state");
        if state.error_message.is_some() {
            state.error_message = None;
            // Don't return - let the key event continue to be processed
        }
    }

    // Get current view type by locking state briefly
    let current_view = {
        let state = app.state.lock().expect("Failed to lock state");
        state.current_view.clone()
    };

    // View-specific handling - let views handle keys first
    let handled = match &current_view {
        ViewType::Main => app.handle_main_view_input(key)?,
        ViewType::CommandList { .. } => app.handle_command_list_input(key)?,
        ViewType::LogBrowser { .. } => app.handle_log_browser_input(key)?,
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
