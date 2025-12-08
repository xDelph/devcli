use super::app::TuiApp;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn test_app_quit_logic() {
    let mut app = TuiApp::new_test();

    // Initial state
    assert!(!app.should_quit());

    // Press 'q'
    let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let _ = app.handle_key_event(event);

    assert!(app.should_quit());
}

#[test]
fn test_app_ctrl_c_quit() {
    let mut app = TuiApp::new_test();

    // Initial state
    assert!(!app.should_quit());

    // Press Ctrl+C
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let _ = app.handle_key_event(event);

    assert!(app.should_quit());
}

#[test]
fn test_app_help_toggle() {
    let mut app = TuiApp::new_test();

    // Initial state
    assert!(!app.is_help_visible());

    // Press '?'
    let event = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    let _ = app.handle_key_event(event);

    assert!(app.is_help_visible());

    // Press '?' again to close
    let event = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    let _ = app.handle_key_event(event);

    assert!(!app.is_help_visible());

    // Press '?' to open
    let event = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    let _ = app.handle_key_event(event);
    assert!(app.is_help_visible());

    // Press Esc to close
    let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let _ = app.handle_key_event(event);
    assert!(!app.is_help_visible());
}
