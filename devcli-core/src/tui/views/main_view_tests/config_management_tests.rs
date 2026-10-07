// Tests for config management functionality
//
// IMPORTANT: These tests use TestConfigGuard to ensure they NEVER modify the real config.json

use crate::config::loader::save_config;
use crate::config::models::{App, Commands, Config, Defaults, Project};
use crate::tui::state::AppState;
use crate::tui::views::main_view::{ConfigField, ConfigMode, MainTab, MainView, PanelFocus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use process_manager::StateStore;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use tempfile::TempDir;

struct TestConfigGuard {
    _lock: MutexGuard<'static, ()>,
    _temp_dir: TempDir,
    original_dir: Option<String>,
}

impl TestConfigGuard {
    fn new() -> Self {
        // Use the *shared* env lock, not a private one: the loader tests clear
        // devcli_CONFIG_DIR, and two different mutexes would not exclude each
        // other, so save_config could still land on the real ~/.devcli.
        let _lock = crate::test_utils::env_guard();
        let original_dir = std::env::var("devcli_CONFIG_DIR").ok();
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("devcli_CONFIG_DIR", temp_dir.path());
        Self {
            _lock,
            _temp_dir: temp_dir,
            original_dir,
        }
    }
}

impl Drop for TestConfigGuard {
    fn drop(&mut self) {
        match &self.original_dir {
            Some(dir) => std::env::set_var("devcli_CONFIG_DIR", dir),
            None => std::env::remove_var("devcli_CONFIG_DIR"),
        }
    }
}

fn setup_test_config_dir() -> TestConfigGuard {
    TestConfigGuard::new()
}

fn create_test_config() -> Config {
    let mut projects = HashMap::new();
    let mut apps = HashMap::new();

    let mut local_cmds = HashMap::new();
    local_cmds.insert("start".to_string(), "npm start".to_string());

    apps.insert(
        "test-app".to_string(),
        App {
            app_type: "nodejs".to_string(),
            alternative_name: None,
            path: "/test/path".to_string(),
            commands: Commands {
                local: Some(local_cmds),
                docker: None,
                orbstack: None,
                k8s: None,
                ..Default::default()
            },
            dependencies: vec![],
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                orbstack: None,
                k8s: None,
                ..Default::default()
            },
            dockerfile_path: None,
            env_files: None,
            default_stages: None,
            health_check: None,
            restart_policy: None,
        },
    );

    projects.insert(
        "test-project".to_string(),
        Project {
            apps,
            alternative_name: None,
        },
    );

    Config { projects }
}

/// Build state against a throwaway process store so the test never reads or
/// writes the real `~/.devcli/processes`.
fn create_test_state_arc(config: &Config) -> Arc<Mutex<AppState>> {
    let dir = tempfile::tempdir().unwrap();
    let store = StateStore::new(dir.path().to_path_buf()).unwrap();
    let state = AppState::from_config_with_store(config, &store).unwrap();
    Arc::new(Mutex::new(state))
}

#[test]
fn test_config_mode_starts_in_view() {
    let view = MainView::new().unwrap();
    assert_eq!(view.config_mode, ConfigMode::View);
}

#[test]
fn test_config_tab_navigation() {
    let _guard = setup_test_config_dir();
    let config = create_test_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    let mut view = MainView::new().unwrap();
    view.active_tab = MainTab::Config;
    view.focus = PanelFocus::DetailPanel;

    let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::AddCommand);

    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::View);
}

#[test]
fn test_config_form_field_editing() {
    let _guard = setup_test_config_dir();
    let config = create_test_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    let mut view = MainView::new().unwrap();
    view.active_tab = MainTab::Config;
    view.config_mode = ConfigMode::AddCommand;
    view.config_focused_field = ConfigField::EditCommandName;

    for ch in ['t', 'e', 's', 't'] {
        let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
        assert!(view.handle_input(key, &state).unwrap());
    }

    assert_eq!(view.config_form.edit_command_name.content(), "test");
}

#[test]
fn test_config_form_backspace() {
    let _guard = setup_test_config_dir();
    let config = create_test_config();
    let state = create_test_state_arc(&config);
    let mut view = MainView::new().unwrap();
    view.active_tab = MainTab::Config;
    view.config_mode = ConfigMode::AddCommand;
    view.config_focused_field = ConfigField::EditCommandName;
    view.config_form
        .edit_command_name
        .set_content("test".to_string());

    let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_form.edit_command_name.content(), "tes");
    assert_eq!(view.config_form.edit_command_name.cursor_position(), 3);
}

#[test]
fn test_dependencies_mode_navigation() {
    let _guard = setup_test_config_dir();
    let config = create_test_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    let mut view = MainView::new().unwrap();
    view.active_tab = MainTab::Config;
    view.focus = PanelFocus::DetailPanel;
    view.config_mode = ConfigMode::View;

    let key = KeyEvent::new(KeyCode::Char('D'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::EditDependencies);
}

#[test]
fn test_add_dependency_mode_navigation() {
    let _guard = setup_test_config_dir();
    let config = create_test_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    let mut view = MainView::new().unwrap();
    view.active_tab = MainTab::Config;
    view.focus = PanelFocus::DetailPanel;
    view.config_mode = ConfigMode::EditDependencies;

    let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::AddDependency);

    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::EditDependencies);
}

#[test]
fn test_config_edit_app_mode() {
    let _guard = setup_test_config_dir();
    let config = create_test_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    let mut view = MainView::new().unwrap();
    view.active_tab = MainTab::Config;
    view.focus = PanelFocus::DetailPanel;
    view.config_mode = ConfigMode::View;

    let key = KeyEvent::new(KeyCode::Char('E'), KeyModifiers::NONE);
    assert!(view.handle_input(key, &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::Edit);
    assert_eq!(view.config_form.project_name.content(), "test-project");
    assert_eq!(view.config_form.app_name.content(), "test-app");
    assert_eq!(view.config_form.app_type.content(), "nodejs");
}

#[test]
fn test_config_edit_command_mode() {
    let _guard = setup_test_config_dir();
    let config = create_test_config();
    save_config(&config).unwrap();
    let mut view = MainView::new().unwrap();

    view.load_command_into_form("test-project", "test-app")
        .unwrap();
    assert_eq!(view.config_mode, ConfigMode::EditCommand);
    assert_eq!(view.config_form.edit_command_name.content(), "start");
}
