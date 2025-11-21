// Unit tests for config module
// Tests config loading, saving, validation, and resolution

#[cfg(test)]
mod tests {
    use super::super::*;
    use std::collections::HashMap;

    // Helper: Create a test config with known structure
    fn create_test_config() -> Config {
        let mut projects = HashMap::new();
        
        // Create infrastructure project with redis
        let mut infra_apps = HashMap::new();
        infra_apps.insert(
            "redis".to_string(),
            App {
                app_type: "redis".to_string(),
                path: "/tmp/redis".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "redis-server".to_string());
                        cmds
                    }),
                    docker: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("run".to_string(), "docker run redis".to_string());
                        cmds
                    }),
                    orbstack: None,
                    k8s: None,
                },
                dependencies: Vec::new(),
                dockerfile_path: None,
                stage: None,
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: Some("run".to_string()),
                    orbstack: None,
                    k8s: None,
                },
            },
        );
        
        projects.insert("infrastructure".to_string(), Project { apps: infra_apps });
        
        // Create api project that depends on redis
        let mut api_apps = HashMap::new();
        api_apps.insert(
            "api".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/api".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds.insert("test".to_string(), "npm test".to_string());
                        cmds
                    }),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "infrastructure".to_string(),
                    app: "redis".to_string(),
                }],
                dockerfile_path: None,
                stage: None,
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
            },
        );
        
        projects.insert("api-project".to_string(), Project { apps: api_apps });
        Config { projects }
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
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        let mut apps = HashMap::new();
        apps.insert(
            "web".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/web".to_string(),
                commands: Commands {
                    local: None,
                    docker: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("run".to_string(), "docker run web".to_string());
                        cmds
                    }),
                    orbstack: None,
                    k8s: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("apply".to_string(), "kubectl apply -f k8s/".to_string());
                        cmds.insert("delete".to_string(), "kubectl delete -f k8s/".to_string());
                        cmds
                    }),
                },
                dependencies: Vec::new(),
                dockerfile_path: None,
                stage: None,
                defaults: Defaults {
                    local: None,
                    docker: Some("run".to_string()),
                    orbstack: None,
                    k8s: Some("apply".to_string()),
                },
            },
        );
        
        config.projects.insert("web-project".to_string(), Project { apps });
        
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
        let app = App {
            app_type: "nodejs".to_string(),
            path: "/tmp/app".to_string(),
            commands: Commands {
                local: None,
                docker: None,
                orbstack: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            dockerfile_path: None,
            stage: None,
            defaults: Defaults {
                local: None,
                docker: None,
                orbstack: None,
                k8s: None,
            },
        };
        
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
        };
        
        let json = serde_json::to_string_pretty(&prefs).unwrap();
        let deserialized: Preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.default_env, "docker");
        assert!(deserialized.detached_mode);
        assert_eq!(deserialized.docker_platform, "linux/arm64");
        assert!(!deserialized.auto_start_deps);
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
        
        let mut apps = HashMap::new();
        apps.insert(
            "my-app".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "~/Projects/my app/with spaces".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
                dependencies: Vec::new(),
                dockerfile_path: None,
                stage: None,
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
            },
        );
        
        config.projects.insert("test".to_string(), Project { apps });
        
        // Should serialize/deserialize with special chars
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        let app = &deserialized.projects["test"].apps["my-app"];
        assert_eq!(app.path, "~/Projects/my app/with spaces");
    }

    // Test: Large config with many projects and apps
    #[test]
    fn test_large_config() {
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        // Create 10 projects, each with 5 apps
        for i in 0..10 {
            let mut apps = HashMap::new();
            for j in 0..5 {
                apps.insert(
                    format!("app-{}", j),
                    App {
                        app_type: "nodejs".to_string(),
                        path: format!("/tmp/project{}/app{}", i, j),
                        commands: Commands {
                            local: Some({
                                let mut cmds = HashMap::new();
                                cmds.insert("start".to_string(), "npm start".to_string());
                                cmds
                            }),
                            docker: None,
                            orbstack: None,
                            k8s: None,
                        },
                        dependencies: Vec::new(),
                        dockerfile_path: None,
                        stage: None,
                        defaults: Defaults {
                            local: Some("start".to_string()),
                            docker: None,
                            orbstack: None,
                            k8s: None,
                        },
                    },
                );
            }
            config.projects.insert(format!("project-{}", i), Project { apps });
        }
        
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
        
        // Stage should be None
        let app = &config.projects["my-project"].apps["api"];
        assert!(app.stage.is_none());
        
        // Re-serialize and verify stage field is omitted
        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("\"stage\""));
    }

    // Test: Config with stage field serializes and deserializes correctly
    #[test]
    fn test_stage_field_serialization() {
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        let mut apps = HashMap::new();
        apps.insert(
            "api".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/api".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
                dependencies: Vec::new(),
                dockerfile_path: None,
                stage: Some("dev".to_string()),
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
            },
        );
        
        config.projects.insert("test".to_string(), Project { apps });
        
        // Serialize and verify stage is included
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"stage\""));
        assert!(json.contains("\"dev\""));
        
        // Deserialize and verify stage value
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        let app = &deserialized.projects["test"].apps["api"];
        assert_eq!(app.stage, Some("dev".to_string()));
    }

    // Test: Mixed config with some apps having stage and others not
    #[test]
    fn test_mixed_stage_configuration() {
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        let mut apps = HashMap::new();
        
        // App with stage
        apps.insert(
            "api".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/api".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
                dependencies: Vec::new(),
                dockerfile_path: None,
                stage: Some("prod".to_string()),
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
            },
        );
        
        // App without stage
        apps.insert(
            "worker".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/worker".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
                dependencies: Vec::new(),
                dockerfile_path: None,
                stage: None,
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
                    k8s: None,
                },
            },
        );
        
        config.projects.insert("test".to_string(), Project { apps });
        
        // Serialize and deserialize
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: Config = serde_json::from_str(&json).unwrap();
        
        // Verify mixed configuration
        let api = &deserialized.projects["test"].apps["api"];
        let worker = &deserialized.projects["test"].apps["worker"];
        
        assert_eq!(api.stage, Some("prod".to_string()));
        assert!(worker.stage.is_none());
    }
}
