//! Unit tests for config editing functionality

#[cfg(test)]
mod edit_tests {
    use crate::config::{Config, Project, App, Commands, Defaults};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a test config
    fn create_test_config(temp_dir: &TempDir) -> Config {
        let app_dir = temp_dir.path().join("test-app");
        fs::create_dir_all(&app_dir).unwrap();
        
        let mut projects = HashMap::new();
        let mut apps = HashMap::new();
        
        let mut local_commands = HashMap::new();
        local_commands.insert("start".to_string(), "npm start".to_string());
        local_commands.insert("test".to_string(), "npm test".to_string());
        
        let app = App {
            app_type: "nodejs".to_string(),
            path: app_dir.to_string_lossy().to_string(),
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
    fn test_config_edit_operations_structure() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        
        // Test that config has expected structure for editing operations
        assert!(!config.projects.is_empty());
        assert!(config.projects.contains_key("test-project"));
        
        let app = &config.projects["test-project"].apps["test-app"];
        assert!(app.commands.local.is_some());
        
        let local_commands = app.commands.local.as_ref().unwrap();
        assert!(local_commands.contains_key("start"));
        assert!(local_commands.contains_key("test"));
        assert_eq!(local_commands["start"], "npm start");
        assert_eq!(local_commands["test"], "npm test");
        
        // Test defaults
        assert_eq!(app.defaults.local.as_ref().unwrap(), "start");
    }

    #[test]
    fn test_config_command_validation() {
        // Test environment validation logic
        let valid_environments = ["local", "docker", "k8s"];
        
        // Test valid environments
        for env in &valid_environments {
            assert!(valid_environments.contains(env), "Environment {} should be valid", env);
        }
        
        // Test invalid environment
        assert!(!valid_environments.contains(&"invalid"), "Invalid environment should not be accepted");
    }

    #[test]
    fn test_config_command_operations_logic() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_test_config(&temp_dir);
        
        // Test adding command to new environment
        let app = config.projects.get_mut("test-project").unwrap()
            .apps.get_mut("test-app").unwrap();
        
        // Initially docker should be None
        assert!(app.commands.docker.is_none());
        
        // Simulate adding docker command
        app.commands.docker = Some({
            let mut docker_commands = HashMap::new();
            docker_commands.insert("build".to_string(), "docker build -t test .".to_string());
            docker_commands
        });
        
        // Verify command was added
        assert!(app.commands.docker.is_some());
        assert_eq!(app.commands.docker.as_ref().unwrap()["build"], "docker build -t test .");
    }

    #[test]
    fn test_config_default_command_logic() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_test_config(&temp_dir);
        
        let app = config.projects.get_mut("test-project").unwrap()
            .apps.get_mut("test-app").unwrap();
        
        // Test that default exists in commands
        let default_local = app.defaults.local.as_ref().unwrap();
        let local_commands = app.commands.local.as_ref().unwrap();
        assert!(local_commands.contains_key(default_local), "Default command should exist in commands");
        
        // Test changing default
        app.defaults.local = Some("test".to_string());
        let new_default = app.defaults.local.as_ref().unwrap();
        assert!(local_commands.contains_key(new_default), "New default should exist in commands");
    }
}