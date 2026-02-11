// Unit tests for config module
// Tests config loading, saving, validation, and resolution

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::test_utils::{AppBuilder, ConfigBuilder};
    use std::collections::HashMap;

    // Helper: Create a test config with known structure
    fn create_test_config() -> Config {
        // Create infrastructure project with redis
        let redis = AppBuilder::new("redis", "/tmp/redis")
            .with_local_command("start", "redis-server")
            .with_local_default("start")
            .with_docker_command("run", "docker run redis")
            .with_docker_default("run")
            .build();

        // Create api project that depends on redis
        let api = AppBuilder::new("nodejs", "/tmp/api")
            .with_local_command("start", "npm start")
            .with_local_command("test", "npm test")
            .with_local_default("start")
            .with_dependency("infrastructure", "redis")
            .build();

        ConfigBuilder::new()
            .with_app("infrastructure", "redis", redis)
            .with_app("api-project", "api", api)
            .build()
    }

    // Test: Config serialization and deserialization
    #[test]
    fn test_config_serialization() {
        let config = create_test_config();

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&config).unwrap();

        // Should contain expected keys
        assert!(json.contains("projects"));
        assert!(json.contains("infrastructure"));
        assert!(json.contains("redis"));

        // Deserialize back
        let deserialized: Config = serde_json::from_str(&json).unwrap();

        // Should have same structure
        assert_eq!(deserialized.projects.len(), 2);
        assert!(deserialized.projects.contains_key("infrastructure"));
        assert!(deserialized.projects.contains_key("api-project"));
    }

    // Test: Config with all three environments
    #[test]
    fn test_config_with_k8s_environment() {
        let web = AppBuilder::new("nodejs", "/tmp/web")
            .with_docker_command("run", "docker run web")
            .with_docker_default("run")
            .with_k8s_command("apply", "kubectl apply -f k8s/")
            .with_k8s_command("delete", "kubectl delete -f k8s/")
            .with_k8s_default("apply")
            .build();

        let config = ConfigBuilder::new()
            .with_app("web-project", "web", web)
            .build();

        // Serialize and check
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("k8s"));
        assert!(json.contains("kubectl apply"));

        // Deserialize and verify
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        let web_app = &deserialized.projects["web-project"].apps["web"];
        assert!(web_app.commands.k8s.is_some());
        assert_eq!(web_app.defaults.k8s, Some("apply".to_string()));
    }

    // Test: Empty config is valid
    #[test]
    fn test_empty_config() {
        let config = Config {
            projects: HashMap::new(),
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.projects.len(), 0);
    }

    // Test: App with no commands fails validation
    #[test]
    fn test_app_with_no_commands() {
        let app = AppBuilder::new("nodejs", "/tmp/app").build();

        // App should serialize/deserialize fine
        let json = serde_json::to_string(&app).unwrap();
        let _deserialized: App = serde_json::from_str(&json).unwrap();
        // But validation should catch this (when we implement validation)
    }

    // Test: Preferences defaults
    #[test]
    fn test_preferences_defaults() {
        let prefs = Preferences::default();
        assert_eq!(prefs.default_env, "local");
        assert!(!prefs.detached_mode); // Show output by default
        assert!(prefs.auto_start_deps);
    }

    // Test: Preferences serialization
    #[test]
    fn test_preferences_serialization() {
        let prefs = Preferences {
            default_env: "docker".to_string(),
            detached_mode: true,
            auto_start_deps: false,
            docker_platform: "linux/arm64".to_string(),
            default_stage: Some("qa".to_string()),
        };

        let json = serde_json::to_string_pretty(&prefs).unwrap();
        let deserialized: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.default_env, "docker");
        assert!(deserialized.detached_mode);
        assert_eq!(deserialized.docker_platform, "linux/arm64");
        assert!(!deserialized.auto_start_deps);
        assert_eq!(deserialized.default_stage, Some("qa".to_string()));
    }

    // Test: Commands with only one environment
    #[test]
    fn test_commands_single_environment() {
        let commands = Commands {
            local: Some({
                let mut cmds = HashMap::new();
                cmds.insert("start".to_string(), "npm start".to_string());
                cmds
            }),
            docker: None,
            orbstack: None,
            k8s: None,
        };

        let json = serde_json::to_string(&commands).unwrap();
        // Should only contain local, not docker/k8s (skip_serializing_if)
        assert!(json.contains("local"));
        assert!(!json.contains("docker"));
        assert!(!json.contains("k8s"));
    }

    // Test: Dependency structure
    #[test]
    fn test_dependency_structure() {
        let dep = Dependency {
            project: "infra".to_string(),
            app: "database".to_string(),
        };

        let json = serde_json::to_string(&dep).unwrap();
        let deserialized: Dependency = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.project, "infra");
        assert_eq!(deserialized.app, "database");
    }

    // Test: Multiple dependencies
    #[test]
    fn test_multiple_dependencies() {
        let deps = vec![
            Dependency {
                project: "infra".to_string(),
                app: "redis".to_string(),
            },
            Dependency {
                project: "infra".to_string(),
                app: "postgres".to_string(),
            },
        ];

        let json = serde_json::to_vec(&deps).unwrap();
        let deserialized: Vec<Dependency> = serde_json::from_slice(&json).unwrap();
        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized[0].app, "redis");
        assert_eq!(deserialized[1].app, "postgres");
    }

    // Test: Defaults with missing environments
    #[test]
    fn test_defaults_partial() {
        let defaults = Defaults {
            local: Some("start".to_string()),
            docker: None,
            orbstack: None,
            k8s: None,
        };

        let json = serde_json::to_string(&defaults).unwrap();
        // Should skip None values
        assert!(json.contains("local"));
    }

    // Test: Config with special characters in paths
    #[test]
    fn test_config_with_special_paths() {
        let mut config = Config {
            projects: HashMap::new(),
        };

        let app = AppBuilder::new("nodejs", "~/Projects/my app/with spaces")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .build();

        config.projects.insert(
            "test".to_string(),
            Project {
                apps: {
                    let mut apps = HashMap::new();
                    apps.insert("my-app".to_string(), app);
                    apps
                },
                alternative_name: None,
            },
        );

        // Should serialize/deserialize with special chars
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        let app = &deserialized.projects["test"].apps["my-app"];
        assert_eq!(app.path, "~/Projects/my app/with spaces");
    }

    // Test: Large config with many projects and apps
    #[test]
    fn test_large_config() {
        let mut builder = ConfigBuilder::new();

        // Create 10 projects, each with 5 apps
        for i in 0..10 {
            for j in 0..5 {
                let app = AppBuilder::new("nodejs", &format!("/tmp/project{}/app{}", i, j))
                    .with_local_command("start", "npm start")
                    .with_local_default("start")
                    .build();
                builder = builder.with_app(&format!("project-{}", i), &format!("app-{}", j), app);
            }
        }

        let config = builder.build();

        // Should handle large configs
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config.projects.len(), 10);
        assert_eq!(deserialized.projects.len(), 10);
    }

    // Test: Backward compatibility - loading config without stage field
    #[test]
    fn test_backward_compatibility_no_stage_field() {
        // Simulate an old config JSON without stage field
        let old_config_json = r#"{
            "projects": {
                "my-project": {
                    "apps": {
                        "api": {
                            "type": "nodejs",
                            "path": "/tmp/api",
                            "commands": {
                                "local": {
                                    "start": "npm start"
                                },
                                "docker": {
                                    "run": "docker run api"
                                }
                            },
                            "defaults": {
                                "local": "start",
                                "docker": "run"
                            },
                            "dependencies": []
                        }
                    }
                }
            }
        }"#;

        // Should deserialize successfully
        let config: Config = serde_json::from_str(old_config_json).unwrap();

        // Default stages should be None
        let app = &config.projects["my-project"].apps["api"];
        assert!(app.default_stages.is_none());

        // Re-serialize and verify stage field is omitted
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("\"stage\""));
    }
}
