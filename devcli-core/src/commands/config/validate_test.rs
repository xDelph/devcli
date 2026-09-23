//! Unit tests for config validation functionality

#[cfg(test)]
mod validate_tests {
    use crate::config::{Commands, Config};
    use crate::config_manager_support::DevCliConfigValidator;
    use crate::test_utils::{AppBuilder, ConfigBuilder};
    use config_manager::validation::Validator;
    use std::fs;
    use tempfile::TempDir;

    fn create_valid_config(temp_dir: &TempDir) -> Config {
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
        let temp_dir = TempDir::new().unwrap();
        let config = create_valid_config(&temp_dir);

        let result = DevCliConfigValidator::new().validate(&config);
        assert!(result.is_valid(), "{result}");
    }

    #[test]
    fn test_validate_config_with_no_commands() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_valid_config(&temp_dir);

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

        let result = DevCliConfigValidator::new().validate(&config);
        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.message().contains("No commands defined")));
    }

    #[test]
    fn test_validate_invalid_default_command() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_valid_config(&temp_dir);

        config
            .projects
            .get_mut("test-project")
            .unwrap()
            .apps
            .get_mut("test-app")
            .unwrap()
            .defaults
            .local = Some("nonexistent".to_string());

        let result = DevCliConfigValidator::new().validate(&config);
        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.message().contains("Default local command")));
    }
}
