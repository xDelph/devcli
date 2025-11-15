// Unit tests for interactive module
// Tests user interaction logic and display formatting

#[cfg(test)]
mod tests {
    use crate::commands::auto_add::interactive::show_preview;
    use crate::detection::DetectedApp;
    use std::collections::HashMap;

    // Helper function to create a test DetectedApp
    fn create_test_app(app_type: &str, app_name: &str) -> DetectedApp {
        let mut local_commands = HashMap::new();
        local_commands.insert("start".to_string(), "test command".to_string());
        local_commands.insert("dev".to_string(), "test dev command".to_string());

        let mut docker_commands = HashMap::new();
        docker_commands.insert("up".to_string(), "docker-compose up".to_string());

        DetectedApp {
            app_type: app_type.to_string(),
            app_name: app_name.to_string(),
            path: "/test/path".to_string(),
            local_commands: Some(local_commands),
            docker_commands: Some(docker_commands),
            k8s_commands: None,
            suggested_local_default: Some("start".to_string()),
            suggested_docker_default: Some("up".to_string()),
        }
    }

    #[test]
    fn test_show_preview_displays_basic_info() {
        let app = create_test_app("nodejs", "test-app");
        
        // Smoke test: verify show_preview doesn't panic with valid input
        // Since show_preview prints to stdout, we can't capture output in unit tests
        let result = std::panic::catch_unwind(|| {
            show_preview("test-project", "custom-name", &app);
        });
        
        assert!(result.is_ok(), "show_preview should not panic with valid input");
    }

    #[test]
    fn test_show_preview_with_minimal_app() {
        let app = DetectedApp {
            app_type: "redis".to_string(),
            app_name: "redis-app".to_string(),
            path: "/minimal/path".to_string(),
            local_commands: None,
            docker_commands: None,
            k8s_commands: None,
            suggested_local_default: None,
            suggested_docker_default: None,
        };
        
        // Smoke test: verify show_preview handles minimal app data without panicking
        let result = std::panic::catch_unwind(|| {
            show_preview("minimal-project", "minimal-app", &app);
        });
        
        assert!(result.is_ok(), "show_preview should handle minimal app data without panicking");
    }

    #[test]
    fn test_show_preview_with_k8s_commands() {
        let mut k8s_commands = HashMap::new();
        k8s_commands.insert("apply".to_string(), "kubectl apply -f deployment.yaml".to_string());
        k8s_commands.insert("delete".to_string(), "kubectl delete -f deployment.yaml".to_string());

        let app = DetectedApp {
            app_type: "nodejs".to_string(),
            app_name: "k8s-app".to_string(),
            path: "/k8s/path".to_string(),
            local_commands: None,
            docker_commands: None,
            k8s_commands: Some(k8s_commands),
            suggested_local_default: None,
            suggested_docker_default: None,
        };
        
        // Smoke test: verify show_preview handles K8s commands without panicking
        let result = std::panic::catch_unwind(|| {
            show_preview("k8s-project", "k8s-app", &app);
        });
        
        assert!(result.is_ok(), "show_preview should handle K8s commands without panicking");
    }

    #[test]
    fn test_show_preview_with_all_environments() {
        let mut local_commands = HashMap::new();
        local_commands.insert("start".to_string(), "npm start".to_string());
        
        let mut docker_commands = HashMap::new();
        docker_commands.insert("up".to_string(), "docker-compose up".to_string());
        
        let mut k8s_commands = HashMap::new();
        k8s_commands.insert("apply".to_string(), "kubectl apply -f .".to_string());

        let app = DetectedApp {
            app_type: "nodejs".to_string(),
            app_name: "full-app".to_string(),
            path: "/full/path".to_string(),
            local_commands: Some(local_commands),
            docker_commands: Some(docker_commands),
            k8s_commands: Some(k8s_commands),
            suggested_local_default: Some("start".to_string()),
            suggested_docker_default: Some("up".to_string()),
        };
        
        // Smoke test: verify show_preview handles all environment types without panicking
        let result = std::panic::catch_unwind(|| {
            show_preview("full-project", "full-app", &app);
        });
        
        assert!(result.is_ok(), "show_preview should handle all environment types without panicking");
    }

    #[test]
    fn test_show_preview_with_special_characters() {
        let app = create_test_app("nodejs", "app-with-special-chars");
        
        // Smoke test: verify show_preview handles special characters without panicking
        let result1 = std::panic::catch_unwind(|| {
            show_preview("project-with-dashes", "app_with_underscores", &app);
        });
        let result2 = std::panic::catch_unwind(|| {
            show_preview("project.with.dots", "app-name", &app);
        });
        
        assert!(result1.is_ok(), "show_preview should handle dashes and underscores");
        assert!(result2.is_ok(), "show_preview should handle dots in names");
    }

    // Note: Testing the actual interactive prompts (prompt_project_selection, prompt_app_name, etc.)
    // would require mocking the inquire library or integration tests with actual user input.
    // These functions are primarily tested through integration tests or manual testing.
    // The core logic they use (like validation) is tested in the validation module tests.
}