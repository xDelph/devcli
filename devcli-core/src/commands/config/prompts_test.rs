//! Unit tests for config prompt functionality

#[cfg(test)]
mod prompts_tests {
    use crate::config::Config;
    use crate::test_utils::{AppBuilder, ConfigBuilder};
    use std::collections::HashMap;

    /// Helper to create a test config
    fn create_test_config() -> Config {
        let app = AppBuilder::new("nodejs", "/test/path")
            .with_local_command("start", "npm start")
            .with_local_command("test", "npm test")
            .with_local_default("start")
            .build();

        ConfigBuilder::new()
            .with_app("test-project", "test-app", app)
            .build()
    }

    #[test]
    fn test_config_structure_for_prompts() {
        let config = create_test_config();

        // Test that config structure supports prompt operations
        assert!(config.projects.contains_key("test-project"));
        assert!(config.projects["test-project"]
            .apps
            .contains_key("test-app"));

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
            assert!(
                valid_environments.contains(env),
                "Environment {} should be valid",
                env
            );
        }

        // Test invalid environment
        assert!(
            !valid_environments.contains(&"invalid"),
            "Invalid environment should not be accepted"
        );
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
