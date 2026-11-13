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
        // DEVCLI_CONFIG_DIR, and two different mutexes would not exclude each
        // other, so save_config could still land on the real ~/.devcli.
        let _lock = crate::test_utils::env_guard();
        let original_dir = std::env::var("DEVCLI_CONFIG_DIR").ok();
        let temp_dir = TempDir::new().unwrap();
        // SAFETY: test code; only std reads the environment here, and std serialises its own accesses.
        unsafe { std::env::set_var("DEVCLI_CONFIG_DIR", temp_dir.path()) };
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
            // SAFETY: test code; only std reads the environment here, and std serialises its own accesses.
            Some(dir) => unsafe { std::env::set_var("DEVCLI_CONFIG_DIR", dir) },
            // SAFETY: test code; only std reads the environment here, and std serialises its own accesses.
            None => unsafe { std::env::remove_var("DEVCLI_CONFIG_DIR") },
        }
    }
}

fn setup_test_config_dir() -> TestConfigGuard {
    TestConfigGuard::new()
}

fn create_test_config() -> Config {
    let mut projects = crate::config::models::ProjectMap::new();
    let mut apps = crate::config::models::AppMap::new();

    let mut local_cmds = HashMap::new();
    local_cmds.insert("start".to_string(), "npm start".to_string());

    apps.insert(
        "test-app".to_string(),
        App {
            extra: Default::default(),
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
            port: None,
        },
    );

    projects.insert(
        "test-project".to_string(),
        Project {
            extra: Default::default(),
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

// --- Edit app / commands: the config is updated in place ---------------

use crate::config::loader::load_config;
use crate::test_utils::{AppBuilder, ConfigBuilder};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// An app with much more than the "Edit app" form shows.
fn rich_config() -> Config {
    let mut api = AppBuilder::new("nodejs", "/repo/api")
        .with_local_command("build", "npm run build")
        .with_local_command("dev", "npm run dev")
        .with_local_default("dev")
        .with_docker_command("up", "docker run api")
        .with_docker_default("up")
        .with_orbstack_command("up", "orb run api")
        .with_dependency("p", "db")
        .build();
    api.port = Some(3000);
    api.env_files = Some(
        [(
            "dev".to_string(),
            [("local".to_string(), ".env.dev".to_string())].into(),
        )]
        .into(),
    );
    api.extra
        .insert("future_key".to_string(), serde_json::json!(true));
    let web = AppBuilder::new("nodejs", "/repo/web")
        .with_local_command("start", "npm start")
        .with_dependency("p", "api")
        .build();
    let db = AppBuilder::new("postgres", "/repo/db")
        .with_local_command("start", "pg")
        .build();
    ConfigBuilder::new()
        .with_app("p", "api", api)
        .with_app("p", "db", db)
        .with_app("p", "web", web)
        .build()
}

fn select(state: &Arc<Mutex<AppState>>, project: &str, app: &str) {
    let key = crate::tui::state::AppKey::new(project, app);
    assert!(state.lock().unwrap().select_app(&key));
}

fn open_edit_app(view: &mut MainView, state: &Arc<Mutex<AppState>>) {
    view.active_tab = MainTab::Config;
    view.config_mode = ConfigMode::View;
    assert!(view.handle_input(key(KeyCode::Char('E')), state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::Edit);
}

#[test]
fn edit_app_keeps_what_the_form_does_not_show() {
    let _guard = setup_test_config_dir();
    let config = rich_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    select(&state, "p", "api");
    let mut view = MainView::new().unwrap();
    open_edit_app(&mut view, &state);

    // The default local command is the one in the form, under its name.
    assert_eq!(view.config_form.local_start_cmd.content(), "npm run dev");
    view.config_form
        .local_start_cmd
        .set_content("npm run dev -- --watch".to_string());
    view.config_form.path.set_content("/repo/api2".to_string());
    assert!(view.handle_input(key(KeyCode::Enter), &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::View);

    let saved = load_config().unwrap();
    let api = &saved.projects["p"].apps["api"];
    assert_eq!(api.path, "/repo/api2");
    let local = api.commands.local.as_ref().unwrap();
    assert_eq!(local["dev"], "npm run dev -- --watch");
    assert_eq!(local["build"], "npm run build");
    assert!(!local.contains_key("start"));
    assert_eq!(api.defaults.local.as_deref(), Some("dev"));
    assert_eq!(
        api.commands.docker.as_ref().unwrap()["up"],
        "docker run api"
    );
    assert_eq!(api.commands.orbstack.as_ref().unwrap()["up"], "orb run api");
    assert_eq!(api.dependencies.len(), 1);
    assert_eq!(api.port, Some(3000));
    assert!(api.env_files.is_some());
    assert_eq!(api.extra.get("future_key"), Some(&serde_json::json!(true)));
}

#[test]
fn renaming_an_app_moves_it_and_its_dependents_follow() {
    let _guard = setup_test_config_dir();
    let config = rich_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    select(&state, "p", "api");
    let mut view = MainView::new().unwrap();
    open_edit_app(&mut view, &state);

    view.config_form.app_name.set_content("backend".to_string());
    assert!(view.handle_input(key(KeyCode::Enter), &state).unwrap());

    let saved = load_config().unwrap();
    let apps: Vec<_> = saved.projects["p"].apps.keys().cloned().collect();
    assert_eq!(apps, vec!["backend", "db", "web"], "renamed in place");
    assert_eq!(
        saved.projects["p"].apps["web"].dependencies[0].app,
        "backend"
    );
    let state = state.lock().unwrap();
    assert_eq!(state.selected_app().unwrap().name, "backend");
}

#[test]
fn renaming_onto_an_existing_app_is_refused() {
    let _guard = setup_test_config_dir();
    let config = rich_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    select(&state, "p", "api");
    let mut view = MainView::new().unwrap();
    open_edit_app(&mut view, &state);

    view.config_form.app_name.set_content("web".to_string());
    assert!(view.handle_input(key(KeyCode::Enter), &state).unwrap());

    assert_eq!(view.config_mode, ConfigMode::Edit, "the form stays open");
    let message = state.lock().unwrap().error_message.clone().unwrap();
    assert!(message.contains("p/web already exists"), "{message}");
    let saved = load_config().unwrap();
    assert_eq!(saved.projects["p"].apps["web"].path, "/repo/web");
    assert!(saved.projects["p"].apps.contains_key("api"));
}

#[test]
fn an_incomplete_app_form_says_what_is_missing() {
    let _guard = setup_test_config_dir();
    let config = rich_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    select(&state, "p", "api");
    let mut view = MainView::new().unwrap();
    open_edit_app(&mut view, &state);

    view.config_form.path.set_content(String::new());
    assert!(view.handle_input(key(KeyCode::Enter), &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::Edit);
    let message = state.lock().unwrap().error_message.clone().unwrap();
    assert!(message.contains("Path is required"), "{message}");
}

/// On a worktree row the form edits the configured app with its configured
/// path (the row's worktree path used to be saved over it).
#[test]
fn edit_app_on_a_worktree_row_keeps_the_configured_path() {
    let _guard = setup_test_config_dir();
    let config = rich_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    state
        .lock()
        .unwrap()
        .sync_worktree_projects(&[crate::tui::state::TrackedInstance {
            project: "p@feat".to_string(),
            base_project: "p".to_string(),
            slug: "feat".to_string(),
            app: "api".to_string(),
            path: "/worktrees/feat/api".to_string(),
            repo_apps: None,
        }]);
    select(&state, "p@feat", "api");
    let mut view = MainView::new().unwrap();
    open_edit_app(&mut view, &state);

    assert_eq!(view.config_form.project_name.content(), "p");
    assert_eq!(view.config_form.path.content(), "/repo/api");
    assert!(view.handle_input(key(KeyCode::Enter), &state).unwrap());
    let saved = load_config().unwrap();
    assert_eq!(saved.projects["p"].apps["api"].path, "/repo/api");
    assert!(!saved.projects.contains_key("p@feat"));
}

/// `s` sets the default of the command the panel shows selected, in its
/// environment (an orbstack command used to set the k8s default).
#[test]
fn set_default_uses_the_panel_order() {
    let _guard = setup_test_config_dir();
    let config = rich_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    select(&state, "p", "api");
    let mut view = MainView::new().unwrap();
    view.active_tab = MainTab::Config;
    view.focus = PanelFocus::DetailPanel;
    // local: build, dev - docker: up - orbstack: up
    view.selected_config_command_idx = 3;

    assert!(view.handle_input(key(KeyCode::Char('s')), &state).unwrap());
    let saved = load_config().unwrap();
    let api = &saved.projects["p"].apps["api"];
    assert_eq!(api.defaults.orbstack.as_deref(), Some("up"));
    assert_eq!(api.defaults.k8s, None);
}

/// Renaming a command moves it (and the default) instead of copying it.
#[test]
fn editing_a_command_name_renames_it() {
    let _guard = setup_test_config_dir();
    let config = rich_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    select(&state, "p", "api");
    let mut view = MainView::new().unwrap();
    view.active_tab = MainTab::Config;
    view.focus = PanelFocus::DetailPanel;
    view.selected_config_command_idx = 1; // local "dev", the default

    assert!(view.handle_input(key(KeyCode::Char('e')), &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::EditCommand);
    view.config_form
        .edit_command_name
        .set_content("serve".to_string());
    assert!(view.handle_input(key(KeyCode::Enter), &state).unwrap());

    let saved = load_config().unwrap();
    let api = &saved.projects["p"].apps["api"];
    let local = api.commands.local.as_ref().unwrap();
    assert!(!local.contains_key("dev"));
    assert_eq!(local["serve"], "npm run dev");
    assert_eq!(api.defaults.local.as_deref(), Some("serve"));
}

/// The delete confirmation is modal: `q` does not quit through it, and `y`
/// does not delete an app other than the one it asked about.
#[test]
fn delete_confirmation_is_modal_and_bound_to_its_app() {
    let _guard = setup_test_config_dir();
    let config = rich_config();
    save_config(&config).unwrap();
    let state = create_test_state_arc(&config);
    select(&state, "p", "api");
    let mut view = MainView::new().unwrap();
    view.active_tab = MainTab::Config;
    view.focus = PanelFocus::AppList;

    assert!(view.handle_input(key(KeyCode::Char('d')), &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::ConfirmDelete);
    assert!(
        view.handle_input(key(KeyCode::Char('q')), &state).unwrap(),
        "consumed"
    );
    assert_eq!(view.config_mode, ConfigMode::ConfirmDelete);

    // The selection moves under the open question.
    select(&state, "p", "web");
    assert!(view.handle_input(key(KeyCode::Char('y')), &state).unwrap());
    assert_eq!(view.config_mode, ConfigMode::View);
    let saved = load_config().unwrap();
    assert!(saved.projects["p"].apps.contains_key("web"));
    assert!(saved.projects["p"].apps.contains_key("api"));
    assert!(state.lock().unwrap().error_message.is_some());
}
