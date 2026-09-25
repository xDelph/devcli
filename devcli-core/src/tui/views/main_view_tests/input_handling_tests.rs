// Tests for input handling functionality

use crate::test_utils::{AppBuilder, ConfigBuilder};
use crate::tui::state::AppState;
use crate::tui::views::main_view::{ConfigField, ConfigMode, MainTab, MainView, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::{Arc, Mutex};

fn create_test_state() -> Arc<Mutex<AppState>> {
    let app = AppBuilder::new("nodejs", "/test/path")
        .with_local_command("start", "npm start")
        .with_local_command("test", "npm test")
        .with_local_default("start")
        .build();

    let config = ConfigBuilder::new()
        .with_app("test-project", "test-app", app)
        .build();

    let state = AppState::from_config(&config).unwrap();
    Arc::new(Mutex::new(state))
}

#[test]
fn test_tab_switching_with_numbers() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();

    let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.active_tab, MainTab::Status);

    let key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.active_tab, MainTab::Config);
}

#[test]
fn test_tab_cycling_with_tab_key() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();
    view.active_tab = MainTab::Status;

    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.active_tab, MainTab::Config);

    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.active_tab, MainTab::Status);
}

#[test]
fn test_panel_focus_switching() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();

    assert_eq!(view.focus, PanelFocus::AppList);

    let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.focus, PanelFocus::DetailPanel);

    let key = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.focus, PanelFocus::AppList);
}

#[test]
fn test_navigation_in_app_list() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();
    view.focus = PanelFocus::AppList;

    let initial_idx = {
        let s = state.lock().unwrap();
        s.selected_app_idx
    };

    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());

    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());

    let final_idx = {
        let s = state.lock().unwrap();
        s.selected_app_idx
    };
    assert_eq!(initial_idx, final_idx);
}

#[test]
fn test_config_mode_escape() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();
    view.active_tab = MainTab::Config;
    view.config_mode = ConfigMode::Add;

    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::View);
}

#[test]
fn test_form_field_navigation() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();
    view.active_tab = MainTab::Config;
    view.config_mode = ConfigMode::Add;
    view.config_focused_field = ConfigField::ProjectName;

    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_focused_field, ConfigField::AppName);

    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_focused_field, ConfigField::AppType);

    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_focused_field, ConfigField::Path);
}

#[test]
fn test_form_field_editing() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();
    view.active_tab = MainTab::Config;
    view.config_mode = ConfigMode::Add;
    view.config_focused_field = ConfigField::ProjectName;

    for ch in ['t', 'e', 's', 't'] {
        let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
        assert!(view.handle_input(key, &state).unwrap());
    }

    assert_eq!(view.config_form.project_name.content(), "test");
}

#[test]
fn test_form_field_backspace() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();
    view.active_tab = MainTab::Config;
    view.config_mode = ConfigMode::Add;
    view.config_focused_field = ConfigField::ProjectName;
    view.config_form
        .project_name
        .set_content("test".to_string());

    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_form.project_name.content(), "tes");
    assert_eq!(view.config_form.project_name.cursor_position(), 3);
}

#[test]
fn test_space_toggles_project_expansion() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();
    view.focus = PanelFocus::AppList;

    let initial_expanded = {
        let s = state.lock().unwrap();
        s.projects[s.selected_project_idx].expanded
    };

    let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());

    let after_expanded = {
        let s = state.lock().unwrap();
        s.projects[s.selected_project_idx].expanded
    };
    assert_ne!(initial_expanded, after_expanded);
}

#[test]
fn test_vim_style_navigation() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();
    view.focus = PanelFocus::AppList;

    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());

    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
}

#[test]
fn test_azerty_keyboard_tab_switching() {
    let mut view = MainView::new().unwrap();
    let state = create_test_state();

    let key = KeyEvent::new(KeyCode::Char('&'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.active_tab, MainTab::Status);

    let key = KeyEvent::new(KeyCode::Char('é'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.active_tab, MainTab::Config);
}
