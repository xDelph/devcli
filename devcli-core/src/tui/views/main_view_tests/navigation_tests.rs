// Tests for navigation and scroll management

use crate::tui::state::{AppStateData, AppStatus};
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
        project: "test-project".to_string(),
        app_type: "nodejs".to_string(),
        status: AppStatus::Stopped,
        commands: commands_map,
        dependencies: vec![],
        path: Some("/test/path".to_string()),
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
    assert_eq!(cmd.name, "cmd4");
    // Get out of bounds
    let result = view.get_command_by_index(&app, 10);
    assert!(result.is_none());
fn test_get_command_by_index_multiple_environments() {
    let mut app = create_test_app_state_data();
    let mut docker_cmds = vec![];
    for i in 0..3 {
        docker_cmds.push(crate::tui::state::CommandInfo {
            name: format!("docker-cmd{}", i),
            command: format!("docker {}", i),
    app.commands.insert("docker".to_string(), docker_cmds);
    // Get command from local environment
    let result = view.get_command_by_index(&app, 2);
    assert_eq!(cmd.name, "cmd2");
    // Get command from docker environment (index 5 = first docker command)
    let result = view.get_command_by_index(&app, 5);
    assert_eq!(env, "docker");
    assert_eq!(cmd.name, "docker-cmd0");
fn test_get_selected_command() {
    let mut view = MainView::new().unwrap();
    view.selected_command_idx = 2;
    let result = view.get_selected_command(&app);
// Note: calculate_scroll_offset and calculate_selected_app_line are private methods
// They are tested indirectly through the rendering and navigation tests
fn test_format_duration_seconds() {
    let duration = chrono::Duration::seconds(45);
    let formatted = MainView::format_duration(&duration);
    assert_eq!(formatted, "45s");
fn test_format_duration_minutes() {
    let duration = chrono::Duration::seconds(135); // 2m 15s
    assert_eq!(formatted, "2m 15s");
fn test_format_duration_hours() {
    let duration = chrono::Duration::seconds(7395); // 2h 3m 15s
    assert_eq!(formatted, "2h 3m 15s");
fn test_format_duration_exact_minute() {
    let duration = chrono::Duration::seconds(120); // 2m 0s
    assert_eq!(formatted, "2m 0s");
fn test_format_duration_exact_hour() {
    let duration = chrono::Duration::seconds(3600); // 1h 0m 0s
    assert_eq!(formatted, "1h 0m 0s");
