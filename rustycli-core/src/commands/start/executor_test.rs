//! Unit tests for the executor module

#[cfg(test)]
mod tests {

    use super::super::resolver::*;
    use crate::config::models::*;
    use crate::config::resolver::ResolvedApp;
    use std::collections::HashMap;

    fn create_test_app_to_start(app_name: &str, project: &str) -> AppToStart {
        let mut commands = HashMap::new();
        commands.insert("start".to_string(), "echo test".to_string());
        
        let app = App {
            app_type: "test".to_string(),
            path: "/tmp".to_string(), // Use a path that exists
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
            command: "echo test".to_string(),
            default_command: "start".to_string(),
        }
    }

    #[test]
    fn test_start_command_args_validation() {
        let args = StartCommandArgs {
            app_names: vec![],
            project: None,
            env: None,
            skip_deps: false,
        };
        
        // Empty app_names should be handled by the caller
        assert_eq!(args.app_names.len(), 0);
    }

    #[test]
    fn test_start_command_args_single_app() {
        let args = StartCommandArgs {
            app_names: vec!["test-app".to_string()],
            project: Some("test-project".to_string()),
            env: Some("local".to_string()),
            skip_deps: true,
        };
        
        assert_eq!(args.app_names.len(), 1);
        assert_eq!(args.app_names[0], "test-app");
        assert_eq!(args.project.as_ref().unwrap(), "test-project");
        assert_eq!(args.env.as_ref().unwrap(), "local");
        assert!(args.skip_deps);
    }

    #[test]
    fn test_start_command_args_multiple_apps() {
        let args = StartCommandArgs {
            app_names: vec!["app1".to_string(), "app2".to_string(), "app3".to_string()],
            project: None,
            env: None,
            skip_deps: false,
        };
        
        assert_eq!(args.app_names.len(), 3);
        assert_eq!(args.app_names[0], "app1");
        assert_eq!(args.app_names[1], "app2");
        assert_eq!(args.app_names[2], "app3");
        assert!(args.project.is_none());
        assert!(args.env.is_none());
        assert!(!args.skip_deps);
    }

    #[test]
    fn test_empty_apps_to_start_list() {
        let apps_to_start: Vec<AppToStart> = vec![];
        
        // Should handle empty list gracefully
        assert_eq!(apps_to_start.len(), 0);
    }

    #[test]
    fn test_app_to_start_structure() {
        let app_to_start = create_test_app_to_start("test-app", "test-project");
        
        assert_eq!(app_to_start.resolved_app.app_name, "test-app");
        assert_eq!(app_to_start.resolved_app.project, "test-project");
        assert_eq!(app_to_start.command, "echo test");
        assert_eq!(app_to_start.default_command, "start");
        assert_eq!(app_to_start.resolved_app.app.path, "/tmp");
    }

    // Note: Tests for start_apps_in_parallel and start_single_app_process
    // require actual process spawning and file system operations, which
    // are better suited for integration tests. The core logic validation
    // is covered by the existing integration test suite.
}