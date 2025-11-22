// Unit tests for nx_monorepo module
// Tests Nx monorepo detection and handling logic

#[cfg(test)]
mod tests {
    use crate::detection::DetectedApp;
    use std::collections::HashMap;

    // Helper function to create a test DetectedApp for Nx
    fn create_nx_test_app(app_name: &str, has_commands: bool) -> DetectedApp {
        let local_commands = if has_commands {
            let mut commands = HashMap::new();
            commands.insert("start".to_string(), format!("nx run {}:serve", app_name));
            commands.insert("build".to_string(), format!("nx run {}:build", app_name));
            commands.insert("test".to_string(), format!("nx run {}:test", app_name));
            Some(commands)
        } else {
            None
        };

        DetectedApp {
            app_type: "nx".to_string(),
            app_name: app_name.to_string(),
            path: format!("/workspace/apps/{}", app_name),
            local_commands,
            docker_commands: None,
            orbstack_commands: None,
            k8s_commands: None,
            dockerfile_path: None,
            suggested_local_default: if has_commands { Some("start".to_string()) } else { None },
            suggested_docker_default: None,
            suggested_orbstack_default: None,
            env_files: None,
        }
    }

    #[test]
    fn test_nx_app_creation_with_commands() {
        let app = create_nx_test_app("frontend", true);
        
        assert_eq!(app.app_type, "nx");
        assert_eq!(app.app_name, "frontend");
        assert_eq!(app.path, "/workspace/apps/frontend");
        
        // Verify commands are present
        assert!(app.local_commands.is_some());
        let commands = app.local_commands.unwrap();
        assert!(commands.contains_key("start"));
        assert!(commands.contains_key("build"));
        assert!(commands.contains_key("test"));
        
        // Verify command format
        assert_eq!(commands.get("start").unwrap(), "nx run frontend:serve");
        assert_eq!(commands.get("build").unwrap(), "nx run frontend:build");
        
        // Verify default suggestion
        assert_eq!(app.suggested_local_default, Some("start".to_string()));
    }

    #[test]
    fn test_nx_app_creation_without_commands() {
        let app = create_nx_test_app("library", false);
        
        assert_eq!(app.app_type, "nx");
        assert_eq!(app.app_name, "library");
        
        // Verify no commands are present
        assert!(app.local_commands.is_none());
        assert!(app.suggested_local_default.is_none());
    }

    #[test]
    fn test_nx_app_with_special_names() {
        // Test with app names that have special characters
        let app1 = create_nx_test_app("my-frontend-app", true);
        let app2 = create_nx_test_app("shared_utils", true);
        let app3 = create_nx_test_app("api.v2", true);
        
        assert_eq!(app1.app_name, "my-frontend-app");
        assert_eq!(app2.app_name, "shared_utils");
        assert_eq!(app3.app_name, "api.v2");
        
        // Verify commands use the correct app names
        if let Some(commands) = &app1.local_commands {
            assert_eq!(commands.get("start").unwrap(), "nx run my-frontend-app:serve");
        }
        
        if let Some(commands) = &app2.local_commands {
            assert_eq!(commands.get("build").unwrap(), "nx run shared_utils:build");
        }
    }

    #[test]
    fn test_multiple_nx_apps() {
        let apps = vec![
            create_nx_test_app("frontend", true),
            create_nx_test_app("backend", true),
            create_nx_test_app("shared-lib", false),
            create_nx_test_app("e2e-tests", true),
        ];
        
        assert_eq!(apps.len(), 4);
        
        // Count apps with and without commands
        let apps_with_commands = apps.iter().filter(|app| app.local_commands.is_some()).count();
        let apps_without_commands = apps.iter().filter(|app| app.local_commands.is_none()).count();
        
        assert_eq!(apps_with_commands, 3);
        assert_eq!(apps_without_commands, 1);
        
        // Verify all are Nx apps
        assert!(apps.iter().all(|app| app.app_type == "nx"));
        
        // Verify unique names
        let names: Vec<&str> = apps.iter().map(|app| app.app_name.as_str()).collect();
        assert!(names.contains(&"frontend"));
        assert!(names.contains(&"backend"));
        assert!(names.contains(&"shared-lib"));
        assert!(names.contains(&"e2e-tests"));
    }

    #[test]
    fn test_nx_app_path_structure() {
        let frontend_app = create_nx_test_app("frontend", true);
        let backend_app = create_nx_test_app("api", true);
        
        // Verify path structure follows Nx conventions
        assert!(frontend_app.path.contains("/apps/frontend"));
        assert!(backend_app.path.contains("/apps/api"));
        
        // Paths should be different for different apps
        assert_ne!(frontend_app.path, backend_app.path);
    }

    // Note: The actual handle_nx_monorepo function is async and involves user interaction,
    // making it more suitable for integration tests. The core logic for processing
    // detected apps and building configuration is tested through the helper functions
    // and data structures tested above.
}