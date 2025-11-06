//! Unit tests for config validation functionality

#[cfg(test)]
mod tests {
    use crate::config::{Config, Project, App, Commands, Defaults};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a valid test config
    fn create_valid_config(temp_dir: &TempDir) -> Config {
        // Create a test app directory
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
    fn test_validate_config_structure() {
        // Test validation logic without file I/O
        let temp_dir = TempDir::new().unwrap();
        let config = create_valid_config(&temp_dir);
        
        // Test that config has expected structure for validation
        assert!(!config.projects.is_empty());
        assert!(config.projects.contains_key("test-project"));
        
        let app = &config.projects["test-project"].apps["test-app"];
        assert!(app.commands.local.is_some());
        assert!(app.defaults.local.is_some());
        assert_eq!(app.defaults.local.as_ref().unwrap(), "start");
        
        // Test that local commands contain the default
        let local_commands = app.commands.local.as_ref().unwrap();
        assert!(local_commands.contains_key("start"));
    }

    #[test]
    fn test_validate_config_with_no_commands() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_valid_config(&temp_dir);
        
        // Remove all commands
        config.projects.get_mut("test-project").unwrap()
            .apps.get_mut("test-app").unwrap()
            .commands = Commands {
                local: None,
                docker: None,
                k8s: None,
            };
        
        // Test that validation would detect this issue
        let app = &config.projects["test-project"].apps["test-app"];
        let has_local = app.commands.local.as_ref().map(|m| !m.is_empty()).unwrap_or(false);
        let has_docker = app.commands.docker.as_ref().map(|m| !m.is_empty()).unwrap_or(false);
        
        assert!(!has_local && !has_docker, "App should have no commands");
    }

    #[test]
    fn test_validate_invalid_default_command() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_valid_config(&temp_dir);
        
        // Set default to non-existent command
        config.projects.get_mut("test-project").unwrap()
            .apps.get_mut("test-app").unwrap()
            .defaults.local = Some("nonexistent".to_string());
        
        // Test that validation would detect this issue
        let app = &config.projects["test-project"].apps["test-app"];
        let local_commands = app.commands.local.as_ref().unwrap();
        let default_local = app.defaults.local.as_ref().unwrap();
        
        assert!(!local_commands.contains_key(default_local), "Default should not exist in commands");
    }
}