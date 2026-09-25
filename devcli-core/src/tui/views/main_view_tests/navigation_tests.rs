// Tests for navigation and scroll management

use crate::tui::state::{AppStateData, AppStatus, HealthStatus};
use crate::tui::views::main_view::MainView;
use std::collections::HashMap;
fn create_test_app_state_data() -> AppStateData {
    let mut commands_map = HashMap::new();
    let mut local_cmds = vec![];
    for i in 0..5 {
        local_cmds.push(crate::tui::state::CommandInfo {
            name: format!("cmd{}", i),
            command: format!("echo {}", i),
        });
    }
    commands_map.insert("local".to_string(), local_cmds);

    AppStateData {
        name: "test-app".to_string(),
        alternative_name: None,
        project: "test-project".to_string(),
        app_type: "nodejs".to_string(),
        status: AppStatus::Stopped,
        commands: commands_map,
        dependencies: vec![],
        path: Some("/test/path".to_string()),
        stage: None,
        active_stage: None,
        env_files: None,
        restart_count: 0,
        last_exit_code: None,
        health_status: HealthStatus::Unknown,
    }
}
// Note: count_total_commands is a private method
// It is tested indirectly through navigation tests
#[test]
fn test_get_command_by_index() {
    let app = create_test_app_state_data();
    let view = MainView::new().unwrap();
    // Get first command
    let result = view.get_command_by_index(&app, 0);
    assert!(result.is_some());
    let (env, cmd) = result.unwrap();
    assert_eq!(env, "local");
    assert_eq!(cmd.name, "cmd0");
    // Get last command
    let result = view.get_command_by_index(&app, 4);
    assert!(result.is_some());
    let (_, cmd) = result.unwrap();
    assert_eq!(cmd.name, "cmd4");
    // Get out of bounds
    let result = view.get_command_by_index(&app, 10);
    assert!(result.is_none());
}

#[test]
fn test_get_command_by_index_multiple_environments() {
    let mut app = create_test_app_state_data();
    let mut docker_cmds = vec![];
    for i in 0..3 {
        docker_cmds.push(crate::tui::state::CommandInfo {
            name: format!("docker-cmd{}", i),
            command: format!("docker {}", i),
        });
    }
    app.commands.insert("docker".to_string(), docker_cmds);
    let view = MainView::new().unwrap();
    // Get command from local environment
    let result = view.get_command_by_index(&app, 2);
    assert!(result.is_some());
    let (_, cmd) = result.unwrap();
    assert_eq!(cmd.name, "cmd2");
    // Get command from docker environment (index 5 = first docker command)
    let result = view.get_command_by_index(&app, 5);
    assert!(result.is_some());
    let (env, cmd) = result.unwrap();
    assert_eq!(env, "docker");
    assert_eq!(cmd.name, "docker-cmd0");
}

#[test]
fn test_get_selected_config_command() {
    let app = create_test_app_state_data();
    let mut view = MainView::new().unwrap();
    view.selected_config_command_idx = 2;
    let result = view.get_selected_config_command(&app);
    assert!(result.is_some());
    let (_, cmd) = result.unwrap();
    assert_eq!(cmd.name, "cmd2");
}

// Note: calculate_scroll_offset and calculate_selected_app_line are private methods
// They are tested indirectly through the rendering and navigation tests

#[test]
fn test_format_duration_seconds() {
    let duration = chrono::Duration::seconds(45);
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "45s");
}

#[test]
fn test_format_duration_minutes() {
    let duration = chrono::Duration::seconds(135); // 2m 15s
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "2m 15s");
}

#[test]
fn test_format_duration_hours() {
    let duration = chrono::Duration::seconds(7395); // 2h 3m 15s
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "2h 3m 15s");
}

#[test]
fn test_format_duration_exact_minute() {
    let duration = chrono::Duration::seconds(120); // 2m 0s
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "2m 0s");
}

#[test]
fn test_format_duration_exact_hour() {
    let duration = chrono::Duration::seconds(3600); // 1h 0m 0s
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "1h 0m 0s");
}
