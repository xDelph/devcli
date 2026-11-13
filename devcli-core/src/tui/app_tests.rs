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

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// The mouse wheel scrolls; it does not close the help or dismiss errors.
#[test]
fn wheel_scrolling_closes_and_dismisses_nothing() {
    let mut app = TuiApp::new_test();
    app.state.lock().unwrap().error_message = Some("boom".to_string());
    app.handle_scroll_event(key(KeyCode::Down)).unwrap();
    assert_eq!(
        app.state.lock().unwrap().error_message.as_deref(),
        Some("boom")
    );

    app.handle_key_event(key(KeyCode::Char('?'))).unwrap();
    assert!(app.is_help_visible());
    app.handle_scroll_event(key(KeyCode::Down)).unwrap();
    app.handle_key_event(key(KeyCode::PageDown)).unwrap();
    assert!(app.is_help_visible(), "scrolling keeps the help open");
    app.handle_key_event(key(KeyCode::Char('x'))).unwrap();
    assert!(!app.is_help_visible());
}

/// An error raised in the log viewer is drawn there too.
#[test]
fn errors_are_shown_in_the_log_viewer() {
    let mut app = TuiApp::new_test();
    {
        let mut state = app.state.lock().unwrap();
        state.current_view = super::state::ViewType::LogViewer { log_paths: vec![] };
        state.error_message = Some("Cannot open x.log".to_string());
    }
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 20)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let screen: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(screen.contains("Cannot open x.log"), "{screen}");
}
