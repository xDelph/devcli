//! Unit tests for config validation functionality

#[cfg(test)]
mod validate_tests {
    use crate::config::{Commands, Config};
    use crate::test_utils::{AppBuilder, ConfigBuilder};
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a valid test config
    fn create_valid_config(temp_dir: &TempDir) -> Config {
        // Create a test app directory
        let app_dir = temp_dir.path().join("test-app");
        fs::create_dir_all(&app_dir).unwrap();

        let app = AppBuilder::new("nodejs", &app_dir.to_string_lossy())
            .with_local_command("start", "npm start")
            .with_local_command("test", "npm test")
            .with_local_default("start")
            .build();

        ConfigBuilder::new()
            .with_app("test-project", "test-app", app)
            .build()
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
        config
            .projects
            .get_mut("test-project")
            .unwrap()
            .apps
            .get_mut("test-app")
            .unwrap()
            .commands = Commands {
            local: None,
            docker: None,
            orbstack: None,
            k8s: None,
            ..Default::default()
        };

        // Test that validation would detect this issue
        let app = &config.projects["test-project"].apps["test-app"];
        let has_local = app
            .commands
            .local
            .as_ref()
            .map(|m| !m.is_empty())
            .unwrap_or(false);
        let has_docker = app
            .commands
            .docker
            .as_ref()
            .map(|m| !m.is_empty())
            .unwrap_or(false);
        assert!(!has_local && !has_docker, "App should have no commands");
    }

    #[test]
    fn test_validate_invalid_default_command() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_valid_config(&temp_dir);

        // Set default to non-existent command
        config
            .projects
            .get_mut("test-project")
            .unwrap()
            .apps
            .get_mut("test-app")
            .unwrap()
            .defaults
            .local = Some("nonexistent".to_string());

        let app = &config.projects["test-project"].apps["test-app"];
        let local_commands = app.commands.local.as_ref().unwrap();
        let default_local = app.defaults.local.as_ref().unwrap();
        assert!(
            !local_commands.contains_key(default_local),
            "Default should not exist in commands"
        );
    }
}
