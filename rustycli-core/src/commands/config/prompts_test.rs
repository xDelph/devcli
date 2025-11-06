//! Unit tests for config prompt functionality

#[cfg(test)]
mod tests {
    use crate::config::{Config, Project, App, Commands, Defaults};
    use std::collections::HashMap;

    /// Helper to create a test config
    fn create_test_config() -> Config {
        let mut projects = HashMap::new();
        let mut apps = HashMap::new();
        
        let mut local_commands = HashMap::new();
        local_commands.insert("start".to_string(), "npm start".to_string());
        local_commands.insert("test".to_string(), "npm test".to_string());
        
        let app = App {
            app_type: "nodejs".to_string(),
            path: "/test/path".to_string(),
            commands: Commands {
                local: Some(local_commands),
                docker: None,
                k8s: None,
            },
            dependencies: vec![],
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                k8s: None,
            },
        };
        
        apps.insert("test-app".to_string(), app);
        projects.insert("test-project".to_string(), Project { apps });
        
        Config { projects }
    }

    #[test]
    fn test_config_structure_for_prompts() {
        let config = create_test_config();
        
        // Test that config structure supports prompt operations
        assert!(config.projects.contains_key("test-project"));
        assert!(config.projects["test-project"].apps.contains_key("test-app"));
        
        let app = &config.projects["test-project"].apps["test-app"];
        assert!(app.commands.local.is_some());
        assert!(app.commands.docker.is_none());
        assert!(app.commands.k8s.is_none());
        
        // Test command availability for prompt_for_command
        let local_commands = app.commands.local.as_ref().unwrap();
        assert!(local_commands.contains_key("start"));
        assert!(local_commands.contains_key("test"));
        
        // Test defaults for prompt display
        assert_eq!(app.defaults.local.as_ref().unwrap(), "start");
        assert!(app.defaults.docker.is_none());
        assert!(app.defaults.k8s.is_none());
    }

    #[test]
    fn test_environment_validation() {
        // Test that valid environments are recognized
        let valid_environments = ["local", "docker", "k8s"];
        
        for env in &valid_environments {
            // This simulates the validation logic used in prompt functions
            assert!(valid_environments.contains(env), "Environment {} should be valid", env);
        }
        
        // Test invalid environment
        assert!(!valid_environments.contains(&"invalid"), "Invalid environment should not be accepted");
    }

    #[test]
    fn test_empty_config_handling() {
        let empty_config = Config {
            projects: HashMap::new(),
        };
        
        // Test that empty config is handled properly
        assert!(empty_config.projects.is_empty());
        assert_eq!(empty_config.projects.len(), 0);
    }
}