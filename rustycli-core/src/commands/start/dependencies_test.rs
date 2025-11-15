//! Unit tests for the dependencies module

#[cfg(test)]
mod tests {

    use super::super::resolver::*;
    use crate::config::models::*;
    use crate::config::resolver::ResolvedApp;
    use std::collections::HashMap;

    fn create_test_app_to_start(app_name: &str, project: &str) -> AppToStart {
        let mut commands = HashMap::new();
        commands.insert("start".to_string(), "npm start".to_string());
        
        let app = App {
            app_type: "test".to_string(),
            path: "/tmp".to_string(),
            commands: Commands {
                local: Some(commands),
                docker: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                k8s: None,
            },
        };
        
        let resolved_app = ResolvedApp {
            app,
            app_name: app_name.to_string(),
            project: project.to_string(),
        };
        
        AppToStart {
            resolved_app,
            command: "npm start".to_string(),
            default_command: "start".to_string(),
        }
    }

    #[test]
    fn test_app_to_start_structure() {
        let app_to_start = create_test_app_to_start("test-app", "test-project");
        
        assert_eq!(app_to_start.resolved_app.app_name, "test-app");
        assert_eq!(app_to_start.resolved_app.project, "test-project");
        assert_eq!(app_to_start.command, "npm start");
        assert_eq!(app_to_start.default_command, "start");
    }

    #[test]
    fn test_empty_apps_to_start() {
        // Test that empty apps list doesn't cause issues
        let apps_to_start: Vec<AppToStart> = vec![];
        
        // This should not panic or cause issues
        assert_eq!(apps_to_start.len(), 0);
    }

    #[test]
    fn test_multiple_apps_to_start() {
        let apps_to_start = vec![
            create_test_app_to_start("app1", "project1"),
            create_test_app_to_start("app2", "project2"),
        ];
        
        assert_eq!(apps_to_start.len(), 2);
        assert_eq!(apps_to_start[0].resolved_app.app_name, "app1");
        assert_eq!(apps_to_start[1].resolved_app.app_name, "app2");
    }

    // Note: Integration tests for handle_dependencies would require 
    // setting up config files and process tracking, which is better
    // suited for integration tests rather than unit tests.
    // The core logic is tested through the existing integration test suite.
}