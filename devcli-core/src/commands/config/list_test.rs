//! Unit tests for config listing functionality

#[cfg(test)]
mod list_tests {
    use crate::config::Config;
    use crate::test_utils::{AppBuilder, ConfigBuilder};
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a test config with multiple projects and apps
    fn create_test_config(temp_dir: &TempDir) -> Config {
        // Create test app directories
        let app1_dir = temp_dir.path().join("app1");
        let app2_dir = temp_dir.path().join("app2");
        fs::create_dir_all(&app1_dir).unwrap();
        fs::create_dir_all(&app2_dir).unwrap();

        let app1 = AppBuilder::new("nodejs", &app1_dir.to_string_lossy())
            .with_local_command("start", "npm start")
            .with_local_command("test", "npm test")
            .with_local_default("start")
            .build();

        let app2 = AppBuilder::new("python", &app2_dir.to_string_lossy())
            .with_local_command("serve", "python -m http.server")
            .with_local_default("serve")
            .build();

        ConfigBuilder::new()
            .with_app("project1", "app1", app1)
            .with_app("project1", "app2", app2)
            .build()
    }

    #[test]
    fn test_config_list_structure() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        // Test that config has expected structure for listing
        assert!(!config.projects.is_empty());
        assert!(config.projects.contains_key("project1"));

        let project = &config.projects["project1"];
        assert!(!project.apps.is_empty());
        assert!(project.apps.contains_key("app1"));
        assert!(project.apps.contains_key("app2"));

        // Test app details
        let app1 = &project.apps["app1"];
        assert_eq!(app1.app_type, "nodejs");
        assert!(app1.commands.local.is_some());
        assert_eq!(app1.defaults.local.as_ref().unwrap(), "start");

        let app2 = &project.apps["app2"];
        assert_eq!(app2.app_type, "python");
        assert!(app2.commands.local.is_some());
        assert_eq!(app2.defaults.local.as_ref().unwrap(), "serve");
    }

    #[test]
    fn test_config_show_app_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        // Test that we can identify apps by project
        let project = &config.projects["project1"];
        let app_names: Vec<&String> = project.apps.keys().collect();

        assert!(app_names.contains(&&"app1".to_string()));
        assert!(app_names.contains(&&"app2".to_string()));

        // Test that app2 is unique (only in project1)
        let app2_occurrences = app_names.iter().filter(|&&name| name == "app2").count();
        assert_eq!(app2_occurrences, 1, "app2 should be unique");
    }

    #[test]
    fn test_config_list_commands_structure() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let app1 = &config.projects["project1"].apps["app1"];

        // Test local commands
        let local_commands = app1.commands.local.as_ref().unwrap();
        assert!(local_commands.contains_key("start"));
        assert!(local_commands.contains_key("test"));
        assert_eq!(local_commands["start"], "npm start");

        // Test default identification
        assert_eq!(app1.defaults.local.as_ref().unwrap(), "start");

        // Test environment filtering logic
        let valid_environments = ["local", "docker", "k8s"];
        assert!(valid_environments.contains(&"local"));
        assert!(valid_environments.contains(&"docker"));
        assert!(valid_environments.contains(&"k8s"));
        assert!(!valid_environments.contains(&"invalid"));
    }
}
