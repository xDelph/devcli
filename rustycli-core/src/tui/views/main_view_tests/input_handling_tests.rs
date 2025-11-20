// Tests for input handling functionality

use crate::config::models::{App, Commands, Defaults, Project};
use crate::process::tracker::ProcessTracker;
use crate::tui::state::AppState;
use crate::tui::views::main_view::{ConfigField, ConfigMode, MainTab, MainView, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
fn create_test_state() -> Arc<Mutex<AppState>> {
    let mut projects = HashMap::new();
    let mut apps = HashMap::new();
    
    let mut local_cmds = HashMap::new();
    local_cmds.insert("start".to_string(), "npm start".to_string());
    local_cmds.insert("test".to_string(), "npm test".to_string());
    apps.insert(
        "test-app".to_string(),
        App {
            app_type: "nodejs".to_string(),
            path: "/test/path".to_string(),
            commands: Commands {
                local: Some(local_cmds),
                docker: None,
                orbstack: None,
                k8s: None,
            },
            dependencies: vec![],
            defaults: Defaults {
                local: Some("start".to_string()),
            dockerfile_path: None,
        },
    );
    projects.insert("test-project".to_string(), Project { apps });
    let config = crate::config::models::Config { projects };
    let tracker = ProcessTracker::new().unwrap();
    let state = AppState::from_config(&config, &tracker).unwrap();
    Arc::new(Mutex::new(state))
}
#[test]
fn test_tab_switching_with_numbers() {
    let mut view = MainView::new();
    let state = create_test_state();
    // Test switching to Status tab (1)
    let key = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.active_tab, MainTab::Status);
    // Test switching to Commands tab (2)
    let key = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
    assert_eq!(view.active_tab, MainTab::Commands);
    // Test switching to Logs tab (3)
    let key = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE);
    assert_eq!(view.active_tab, MainTab::Logs);
    // Test switching to Config tab (4)
    let key = KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE);
    assert_eq!(view.active_tab, MainTab::Config);
fn test_tab_cycling_with_tab_key() {
    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    // Status -> Commands
    // Commands -> Logs
    // Logs -> Config
    // Config -> Status (cycle back)
fn test_panel_focus_switching() {
    assert_eq!(view.focus, PanelFocus::AppList);
    // Switch to detail panel with right arrow
    let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(view.focus, PanelFocus::DetailPanel);
    // Switch back to app list with left arrow
    let key = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
fn test_navigation_in_app_list() {
    view.focus = PanelFocus::AppList;
    let initial_idx = {
        let s = state.lock().unwrap();
        s.selected_app_idx
    };
    // Navigate down
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
    // Navigate up
    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
    let final_idx = {
    assert_eq!(initial_idx, final_idx);
fn test_command_selection_in_commands_tab() {
    view.active_tab = MainTab::Commands;
    view.focus = PanelFocus::DetailPanel;
    view.selected_command_idx = 0;
    // Navigate down in commands
    assert_eq!(view.selected_command_idx, 1);
    // Navigate up in commands
    assert_eq!(view.selected_command_idx, 0);
fn test_shift_navigation_jumps() {
    // Shift+Down should jump by 10
    let key = KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT);
    // Should be clamped to max available commands
    assert!(view.selected_command_idx <= 10);
    // Shift+Up should jump back by 10
    let key = KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT);
fn test_config_mode_escape() {
    view.active_tab = MainTab::Config;
    view.config_mode = ConfigMode::Add;
    // Escape should return to view mode
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(view.config_mode, ConfigMode::View);
fn test_form_field_navigation() {
    view.config_focused_field = ConfigField::ProjectName;
    // Tab should cycle through fields
    assert_eq!(view.config_focused_field, ConfigField::AppName);
    assert_eq!(view.config_focused_field, ConfigField::AppType);
    assert_eq!(view.config_focused_field, ConfigField::Path);
fn test_form_field_editing() {
    // Type some characters
    let key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);
    assert_eq!(view.config_form.project_name, "t");
    let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
    assert_eq!(view.config_form.project_name, "te");
    let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(view.config_form.project_name, "tes");
    assert_eq!(view.config_form.project_name, "test");
fn test_form_field_backspace() {
    view.config_form.project_name = "test".to_string();
    view.config_form.cursor_project_name = 4;
    // Backspace should remove last character
    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(view.config_form.cursor_project_name, 3);
fn test_cursor_movement() {
    // Move cursor left
    // Move cursor right
    assert_eq!(view.config_form.cursor_project_name, 4);
fn test_space_toggles_project_expansion() {
    let initial_expanded = {
        s.projects[s.selected_project_idx].expanded
    // Space should toggle expansion
    let key = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    let after_expanded = {
    assert_ne!(initial_expanded, after_expanded);
fn test_vim_style_navigation() {
    // Test 'j' for down
    let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    // Test 'k' for up
    let key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
fn test_azerty_keyboard_tab_switching() {
    // Test AZERTY keyboard shortcuts
    // & = 1, é = 2, " = 3, ' = 4
    let key = KeyEvent::new(KeyCode::Char('&'), KeyModifiers::NONE);
    let key = KeyEvent::new(KeyCode::Char('é'), KeyModifiers::NONE);
    let key = KeyEvent::new(KeyCode::Char('"'), KeyModifiers::NONE);
    let key = KeyEvent::new(KeyCode::Char('\''), KeyModifiers::NONE);
