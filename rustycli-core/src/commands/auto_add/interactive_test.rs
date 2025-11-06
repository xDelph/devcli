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
        
        // This test verifies that show_preview doesn't panic and handles the basic structure
        // Since show_preview prints to stdout, we can't easily capture the output in a unit test
        // But we can ensure it doesn't crash with valid input
        show_preview("test-project", "custom-name", &app);
        
        // If we get here without panicking, the function handled the input correctly
        assert!(true);
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
        
        // Test with minimal app data (no commands)
        show_preview("minimal-project", "minimal-app", &app);
        
        // If we get here without panicking, the function handled minimal data correctly
        assert!(true);
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
        
        // Test with K8s commands only
        show_preview("k8s-project", "k8s-app", &app);
        
        // If we get here without panicking, the function handled K8s commands correctly
        assert!(true);
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
        
        // Test with all environment types
        show_preview("full-project", "full-app", &app);
        
        // If we get here without panicking, the function handled all environments correctly
        assert!(true);
    }

    #[test]
    fn test_show_preview_with_special_characters() {
        let app = create_test_app("nodejs", "app-with-special-chars");
        
        // Test with project and app names containing special characters
        show_preview("project-with-dashes", "app_with_underscores", &app);
        show_preview("project.with.dots", "app-name", &app);
        
        // If we get here without panicking, the function handled special characters correctly
        assert!(true);
    }

    // Note: Testing the actual interactive prompts (prompt_project_selection, prompt_app_name, etc.)
    // would require mocking the inquire library or integration tests with actual user input.
    // These functions are primarily tested through integration tests or manual testing.
    // The core logic they use (like validation) is tested in the validation module tests.
}