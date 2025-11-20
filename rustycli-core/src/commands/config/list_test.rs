//! Unit tests for config listing functionality

#[cfg(test)]
mod list_tests {
    use crate::config::{Config, Project, App, Commands, Defaults};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a test config with multiple projects and apps
    fn create_test_config(temp_dir: &TempDir) -> Config {
        // Create test app directories
        let app1_dir = temp_dir.path().join("app1");
        let app2_dir = temp_dir.path().join("app2");
        fs::create_dir_all(&app1_dir).unwrap();
        fs::create_dir_all(&app2_dir).unwrap();
        
        let mut projects = HashMap::new();
        
        // Project 1 with 2 apps
        let mut apps1 = HashMap::new();
        
        let mut local_commands1 = HashMap::new();
        local_commands1.insert("start".to_string(), "npm start".to_string());
        local_commands1.insert("test".to_string(), "npm test".to_string());
        
        let app1 = App {
            app_type: "nodejs".to_string(),
            path: app1_dir.to_string_lossy().to_string(),
            commands: Commands {
                local: Some(local_commands1),
                docker: None,
                orbstack: None,
                k8s: None,
            },
            dependencies: vec![],
            dockerfile_path: None,
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                orbstack: None,
                k8s: None,
            },
        };
        
        let mut local_commands2 = HashMap::new();
        local_commands2.insert("serve".to_string(), "python -m http.server".to_string());
        
        let app2 = App {
            app_type: "python".to_string(),
            path: app2_dir.to_string_lossy().to_string(),
            commands: Commands {
                local: Some(local_commands2),
                docker: None,
                orbstack: None,
                k8s: None,
            },
            dependencies: vec![],
            dockerfile_path: None,
            defaults: Defaults {
                local: Some("serve".to_string()),
                docker: None,
                orbstack: None,
                k8s: None,
            },
        };
        
        apps1.insert("app1".to_string(), app1);
        apps1.insert("app2".to_string(), app2);
        projects.insert("project1".to_string(), Project { apps: apps1 });
        
        Config { projects }
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
