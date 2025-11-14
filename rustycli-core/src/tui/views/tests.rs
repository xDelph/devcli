// Tests for the views module
// Focuses on navigation state, selection logic, and view behavior

use super::main_view::{MainTab, MainView, PanelFocus};
use crate::config::models::{App, Commands, Config, Defaults, Project};
use crate::process::tracker::ProcessTracker;
use crate::tui::state::AppState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

/// Helper function to create a test configuration
fn create_test_config() -> Config {
    let mut projects = HashMap::new();
    
    // Create a test project with two apps
    let mut apps = HashMap::new();
    
    let mut local_commands = HashMap::new();
    local_commands.insert("start".to_string(), "npm start".to_string());
    local_commands.insert("test".to_string(), "npm test".to_string());
    
    apps.insert(
        "app1".to_string(),
        App {
            app_type: "nodejs".to_string(),
            path: "/path/to/app1".to_string(),
            commands: Commands {
                local: Some(local_commands.clone()),
                docker: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                k8s: None,
            },
        },
    );
    
    apps.insert(
        "app2".to_string(),
        App {
            app_type: "python".to_string(),
            path: "/path/to/app2".to_string(),
            commands: Commands {
                local: Some(local_commands),
                docker: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                k8s: None,
            },
        },
    );
    
    projects.insert(
        "test-project".to_string(),
        Project { apps },
    );
    
    Config { projects }
}

use std::sync::{Arc, Mutex};

/// Helper function to create a test app state
fn create_test_state() -> AppState {
    let config = create_test_config();
    let tracker = ProcessTracker::new().unwrap();
    AppState::from_config(&config, &tracker).unwrap()
}

/// Helper function to create a test app state wrapped in Arc<Mutex<>>
fn create_test_state_arc() -> Arc<Mutex<AppState>> {
    Arc::new(Mutex::new(create_test_state()))
}

#[test]
fn test_main_view_creation() {
    let view = MainView::new();
    assert_eq!(view.active_tab, MainTab::Status);
    assert_eq!(view.focus, PanelFocus::AppList);
}

#[test]
fn test_main_view_default() {
    let view = MainView::default();
    assert_eq!(view.active_tab, MainTab::Status);
    assert_eq!(view.focus, PanelFocus::AppList);
}

#[test]
fn test_tab_switching_with_number_keys() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    // Switch to Commands tab
    let key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    assert!(handled);
    assert_eq!(view.active_tab, MainTab::Commands);
    
    // Switch to Logs tab
    let key = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    assert!(handled);
    assert_eq!(view.active_tab, MainTab::Logs);
    
    // Switch back to Status tab
    let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    assert!(handled);
    assert_eq!(view.active_tab, MainTab::Status);
}

#[test]
fn test_panel_focus_switching() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    // Initially focused on AppList
    assert_eq!(view.focus, PanelFocus::AppList);
    
    // Press Tab to switch to DetailPanel
    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    assert!(handled);
    assert_eq!(view.focus, PanelFocus::DetailPanel);
    
    // Press Tab again to switch back to AppList
    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    assert!(handled);
    assert_eq!(view.focus, PanelFocus::AppList);
}

#[test]
fn test_navigation_in_app_list() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    // Ensure focus is on app list
    view.focus = PanelFocus::AppList;
    
    let initial_project_idx = state.lock().unwrap().selected_project_idx;
    let initial_app_idx = state.lock().unwrap().selected_app_idx;
    
    // Press down arrow
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    view.handle_input(key, &state).unwrap();
    
    // Selection should have changed
    let state_locked = state.lock().unwrap();
    assert!(
        state_locked.selected_project_idx != initial_project_idx 
        || state_locked.selected_app_idx != initial_app_idx
    );
}

#[test]
fn test_navigation_with_vim_keys() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    view.focus = PanelFocus::AppList;
    
    let initial_project_idx = state.lock().unwrap().selected_project_idx;
    let initial_app_idx = state.lock().unwrap().selected_app_idx;
    
    // Press 'j' (vim down)
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
    view.handle_input(key, &state).unwrap();
    
    // Selection should have changed
    {
        let state_locked = state.lock().unwrap();
        assert!(
            state_locked.selected_project_idx != initial_project_idx 
            || state_locked.selected_app_idx != initial_app_idx
        );
    }
    
    // Press 'k' (vim up)
    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty());
    view.handle_input(key, &state).unwrap();
    
    // Should be back to initial position
    let state_locked = state.lock().unwrap();
    assert_eq!(state_locked.selected_project_idx, initial_project_idx);
    assert_eq!(state_locked.selected_app_idx, initial_app_idx);
}

#[test]
fn test_space_toggles_project_expansion() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    view.focus = PanelFocus::AppList;
    
    let initial_expanded = state.lock().unwrap().projects[0].expanded;
    
    // Press space
    let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    
    assert!(handled);
    assert_eq!(state.lock().unwrap().projects[0].expanded, !initial_expanded);
}

#[test]
fn test_space_only_works_in_app_list() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    // Switch focus to detail panel
    view.focus = PanelFocus::DetailPanel;
    
    let initial_expanded = state.lock().unwrap().projects[0].expanded;
    
    // Press space - should not toggle expansion
    let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    
    // Event is not handled when in detail panel
    assert!(!handled);
    // Expansion state should not change
    assert_eq!(state.lock().unwrap().projects[0].expanded, initial_expanded);
}

#[test]
fn test_detail_panel_scrolling() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    // Switch focus to detail panel
    view.focus = PanelFocus::DetailPanel;
    
    assert_eq!(view.detail_scroll, 0);
    
    // Press down arrow
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    view.handle_input(key, &state).unwrap();
    
    assert_eq!(view.detail_scroll, 1);
    
    // Press up arrow
    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());
    view.handle_input(key, &state).unwrap();
    
    assert_eq!(view.detail_scroll, 0);
}

#[test]
fn test_scroll_reset_on_tab_switch() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    // Set some scroll offset
    view.detail_scroll = 5;
    
    // Switch tabs
    let key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty());
    view.handle_input(key, &state).unwrap();
    
    // Scroll should be reset
    assert_eq!(view.detail_scroll, 0);
}

#[test]
fn test_unhandled_keys_return_false() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    // Press an unhandled key
    let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    
    assert!(!handled);
}

#[test]
fn test_enter_key_is_handled() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    // Press Enter
    let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    
    // Enter is handled (even if it doesn't do anything yet)
    assert!(handled);
}

#[test]
fn test_format_duration_seconds() {
    use chrono::Duration;
    
    let duration = Duration::seconds(45);
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "45s");
}

#[test]
fn test_format_duration_minutes() {
    use chrono::Duration;
    
    let duration = Duration::seconds(125); // 2m 5s
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "2m 5s");
}

#[test]
fn test_format_duration_hours() {
    use chrono::Duration;
    
    let duration = Duration::seconds(7325); // 2h 2m 5s
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "2h 2m 5s");
}

// Tests for status formatting and uptime calculations

#[test]
fn test_format_duration_zero_seconds() {
    use chrono::Duration;
    
    let duration = Duration::seconds(0);
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "0s");
}

#[test]
fn test_format_duration_exact_minute() {
    use chrono::Duration;
    
    let duration = Duration::seconds(60);
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "1m 0s");
}

#[test]
fn test_format_duration_exact_hour() {
    use chrono::Duration;
    
    let duration = Duration::seconds(3600);
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "1h 0m 0s");
}

#[test]
fn test_format_duration_complex() {
    use chrono::Duration;
    
    let duration = Duration::seconds(3661); // 1h 1m 1s
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "1h 1m 1s");
}

#[test]
fn test_format_duration_large_value() {
    use chrono::Duration;
    
    let duration = Duration::seconds(86400); // 24 hours
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "24h 0m 0s");
}

#[test]
fn test_app_state_includes_path_and_dependencies() {
    let config = create_test_config();
    let tracker = ProcessTracker::new().unwrap();
    let state = AppState::from_config(&config, &tracker).unwrap();
    
    // Check that apps have path and dependencies fields
    if let Some(project) = state.projects.first() {
        if let Some(app) = project.apps.first() {
            // Path should be set from config
            assert!(app.path.is_some());
            // Dependencies should be initialized (even if empty)
            assert_eq!(app.dependencies.len(), 0);
        }
    }
}

#[test]
fn test_tab_switching_with_french_keyboard() {
    let mut view = MainView::new();
    let state = create_test_state_arc();
    
    // Test French AZERTY keyboard layout keys
    // & = 1, é = 2, " = 3
    
    // Switch to Commands tab with 'é' (French 2)
    let key = KeyEvent::new(KeyCode::Char('é'), KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    assert!(handled);
    assert_eq!(view.active_tab, MainTab::Commands);
    
    // Switch to Logs tab with '"' (French 3)
    let key = KeyEvent::new(KeyCode::Char('"'), KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    assert!(handled);
    assert_eq!(view.active_tab, MainTab::Logs);
    
    // Switch back to Status tab with '&' (French 1)
    let key = KeyEvent::new(KeyCode::Char('&'), KeyModifiers::empty());
    let handled = view.handle_input(key, &state).unwrap();
    assert!(handled);
    assert_eq!(view.active_tab, MainTab::Status);
}
