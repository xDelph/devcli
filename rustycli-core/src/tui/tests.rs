// Unit tests for TUI components
// Tests state management, theme, and core functionality

use super::state::{AppStatus, ViewType};
use super::theme::Theme;
use chrono::{Duration, Utc};

#[test]
fn test_theme_default_colors() {
    let theme = Theme::default();
    
    // Verify key colors are set correctly
    assert_eq!(theme.running, ratatui::style::Color::LightGreen);
    assert_eq!(theme.stopped, ratatui::style::Color::DarkGray);
    assert_eq!(theme.error, ratatui::style::Color::Red);
    assert_eq!(theme.success, ratatui::style::Color::Green);
    assert_eq!(theme.primary, ratatui::style::Color::Cyan);
}

#[test]
fn test_theme_from_terminal() {
    let theme = Theme::from_terminal();
    
    // Should return a valid theme (currently same as default)
    assert_eq!(theme.primary, ratatui::style::Color::Cyan);
    assert_eq!(theme.running, ratatui::style::Color::LightGreen);
}

#[test]
fn test_app_status_is_running() {
    let running = AppStatus::Running {
        pid: 1234,
        uptime: Duration::seconds(60),
        start_time: Utc::now(),
    };
    assert!(running.is_running());
    assert_eq!(running.as_str(), "Running");

    let stopped = AppStatus::Stopped;
    assert!(!stopped.is_running());
    assert_eq!(stopped.as_str(), "Stopped");

    let unknown = AppStatus::Unknown;
    assert!(!unknown.is_running());
    assert_eq!(unknown.as_str(), "Unknown");
}

#[test]
fn test_view_type_equality() {
    let main1 = ViewType::Main;
    let main2 = ViewType::Main;
    assert_eq!(main1, main2);

    let cmd1 = ViewType::CommandList {
        project: "test".to_string(),
        app: "app1".to_string(),
    };
    let cmd2 = ViewType::CommandList {
        project: "test".to_string(),
        app: "app1".to_string(),
    };
    assert_eq!(cmd1, cmd2);

    // Different apps should not be equal
    let cmd3 = ViewType::CommandList {
        project: "test".to_string(),
        app: "app2".to_string(),
    };
    assert_ne!(cmd1, cmd3);
}

#[test]
fn test_app_status_uptime_formatting() {
    let start_time = Utc::now();
    let uptime = Duration::seconds(3665); // 1 hour, 1 minute, 5 seconds
    
    let status = AppStatus::Running {
        pid: 1234,
        uptime,
        start_time,
    };
    
    assert!(status.is_running());
    
    // Verify we can extract the uptime
    if let AppStatus::Running { uptime: u, .. } = status {
        assert_eq!(u.num_seconds(), 3665);
    }
}
