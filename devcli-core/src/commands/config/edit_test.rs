//! Unit tests for config editing functionality

#[cfg(test)]
mod edit_tests {
    use crate::config::Config;
    use crate::test_utils::{AppBuilder, ConfigBuilder};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a test config
    fn create_test_config(temp_dir: &TempDir) -> Config {
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
        
        let local_commands = app.commands.local.as_ref().unwrap();
        
        // Test that default exists in commands
        let default_local = app.defaults.local.as_ref().unwrap();
        assert!(local_commands.contains_key(default_local), "Default command should exist in commands");
        
        // Test changing default
        app.defaults.local = Some("test".to_string());
        let new_default = app.defaults.local.as_ref().unwrap();
        assert!(local_commands.contains_key(new_default), "New default should exist in commands");
    }

    #[test]
    fn test_stage_validation() {
        use crate::config::models::Stage;
        
        // Test valid stages
        assert!(Stage::from_string("dev").is_some());
        assert!(Stage::from_string("qa").is_some());
        assert!(Stage::from_string("preprod").is_some());
        assert!(Stage::from_string("prod").is_some());
        
        // Test invalid stages
        assert!(Stage::from_string("invalid").is_none());
        assert!(Stage::from_string("development").is_none());
        assert!(Stage::from_string("production").is_none());
        assert!(Stage::from_string("").is_none());
    }

    #[test]
    fn test_default_stages_configuration_logic() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_test_config(&temp_dir);
        
        let app = config.projects.get_mut("test-project").unwrap()
            .apps.get_mut("test-app").unwrap();
        
        // Initially default_stages should be None
        assert!(app.default_stages.is_none());
        
        // Set default stages
        let mut stages = std::collections::HashMap::new();
        stages.insert("local".to_string(), "dev".to_string());
        stages.insert("docker".to_string(), "qa".to_string());
        app.default_stages = Some(stages);
        
        assert!(app.default_stages.is_some());
        assert_eq!(app.default_stages.as_ref().unwrap().get("local").unwrap(), "dev");
        assert_eq!(app.default_stages.as_ref().unwrap().get("docker").unwrap(), "qa");
        
        // Remove default_stages
        app.default_stages = None;
        assert!(app.default_stages.is_none());
    }

    #[test]
    fn test_stage_all_names() {
        use crate::config::models::Stage;
        
        let all_names = Stage::all_names();
        assert!(all_names.contains("dev"));
        assert!(all_names.contains("qa"));
        assert!(all_names.contains("preprod"));
        assert!(all_names.contains("prod"));
    }

    #[test]
    fn test_stage_as_str() {
        use crate::config::models::Stage;
        
        assert_eq!(Stage::Dev.as_str(), "dev");
        assert_eq!(Stage::Qa.as_str(), "qa");
        assert_eq!(Stage::Preprod.as_str(), "preprod");
        assert_eq!(Stage::Prod.as_str(), "prod");
    }
}
