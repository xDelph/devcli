// Tests for config management functionality
// Note: Most config management methods are private, so we test them indirectly
// through the public handle_input API
//
// IMPORTANT: These tests use TestConfigGuard to ensure they NEVER modify the real config.json
// Each test runs in an isolated temporary directory.

use crate::config::loader::save_config;
use crate::config::models::{App, Commands, Config, Defaults, Project};
use crate::process::tracker::ProcessTracker;
use crate::tui::state::AppState;
use crate::tui::views::main_view::{ConfigField, ConfigMode, MainTab, MainView, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
struct TestConfigGuard {
    _temp_dir: TempDir,
    original_dir: Option<String>,
}
impl TestConfigGuard {
    fn new() -> Self {
        let original_dir = std::env::var("RUSTYCLI_CONFIG_DIR").ok();
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("RUSTYCLI_CONFIG_DIR", temp_dir.path());
        Self {
            _temp_dir: temp_dir,
            original_dir,
        }
    }
impl Drop for TestConfigGuard {
    fn drop(&mut self) {
        // Restore original config dir or remove the var
        match &self.original_dir {
            Some(dir) => std::env::set_var("RUSTYCLI_CONFIG_DIR", dir),
            None => std::env::remove_var("RUSTYCLI_CONFIG_DIR"),
fn setup_test_config_dir() -> TestConfigGuard {
    TestConfigGuard::new()
fn create_test_config() -> Config {
    let mut projects = HashMap::new();
    let mut apps = HashMap::new();
    
    let mut local_cmds = HashMap::new();
    local_cmds.insert("start".to_string(), "npm start".to_string());
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
    Config { projects }
fn create_test_state_arc(config: &Config) -> Arc<Mutex<AppState>> {
    let tracker = ProcessTracker::new().unwrap();
    let state = AppState::from_config(config, &tracker).unwrap();
    Arc::new(Mutex::new(state))
#[test]
fn test_config_mode_starts_in_view() {
    let view = MainView::new();
    assert_eq!(view.config_mode, ConfigMode::View);
fn test_config_tab_navigation() {
    let _temp_dir = setup_test_config_dir();
    let config = create_test_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    let mut view = MainView::new();
    view.active_tab = MainTab::Config;
    view.focus = PanelFocus::DetailPanel;
    // Press 'a' to enter add command mode
    let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::AddCommand);
    // Press Esc to return to view mode
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
fn test_config_form_field_editing() {
    view.config_mode = ConfigMode::AddCommand;
    view.config_focused_field = ConfigField::EditCommandName;
    // Type some characters
    let key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);
    assert_eq!(view.config_form.edit_command_name, "t");
    let key = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
    assert_eq!(view.config_form.edit_command_name, "te");
    let key = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(view.config_form.edit_command_name, "tes");
    assert_eq!(view.config_form.edit_command_name, "test");
fn test_config_form_backspace() {
    view.config_form.edit_command_name = "test".to_string();
    view.config_form.cursor_edit_command_name = 4;
    // Backspace should remove last character
    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(view.config_form.cursor_edit_command_name, 3);
fn test_config_form_cursor_movement() {
    // Move cursor left
    let key = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
    // Move cursor right
    let key = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
    assert_eq!(view.config_form.cursor_edit_command_name, 4);
fn test_config_form_tab_navigation() {
    view.config_focused_field = ConfigField::EditCommandEnv;
    // Tab should cycle through fields
    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(view.config_focused_field, ConfigField::EditCommandName);
    assert_eq!(view.config_focused_field, ConfigField::EditCommandValue);
    assert_eq!(view.config_focused_field, ConfigField::EditCommandEnv);
fn test_dependencies_mode_navigation() {
    view.config_mode = ConfigMode::View;
    // Press 'D' to enter dependencies mode
    let key = KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE);
    assert_eq!(view.config_mode, ConfigMode::EditDependencies);
fn test_add_dependency_mode_navigation() {
    view.config_mode = ConfigMode::EditDependencies;
    // Press 'a' to enter add dependency mode
    assert_eq!(view.config_mode, ConfigMode::AddDependency);
    // Press Esc to return to dependencies mode
fn test_config_edit_app_mode() {
    // Press 'E' to enter edit app mode
    let key = KeyEvent::new(KeyCode::Char('E'), KeyModifiers::NONE);
    assert_eq!(view.config_mode, ConfigMode::Edit);
    // Verify form was populated with app data
    assert_eq!(view.config_form.project_name, "test-project");
    assert_eq!(view.config_form.app_name, "test-app");
    assert_eq!(view.config_form.app_type, "nodejs");
fn test_config_edit_command_mode() {
    view.selected_config_command_idx = 0;
    // Press 'e' to enter edit command mode
    assert_eq!(view.config_mode, ConfigMode::EditCommand);
