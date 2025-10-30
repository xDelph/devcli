// Unit tests for resolver module
// Tests app resolution, dependency chains, and circular dependency detection

#[cfg(test)]
mod tests {
    use crate::config::*;
    use std::collections::HashMap;

    // Helper: Create config with multiple projects and apps
    fn create_multi_project_config() -> Config {
        let mut projects = HashMap::new();
        
        // Project 1: infrastructure
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
                    docker: None,
                    k8s: None,
                },
                dependencies: Vec::new(),
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        projects.insert("infrastructure".to_string(), Project { apps: infra_apps });
        
        // Project 2: api (depends on redis)
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
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "infrastructure".to_string(),
                    app: "redis".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        projects.insert("api-project".to_string(), Project { apps: api_apps });
        
        Config { projects }
    }

    // Test: Resolve app with project specified
    #[test]
    fn test_resolve_app_with_project() {
        let config = create_multi_project_config();
        
        let result = get_app_by_project(&config, "infrastructure", "redis");
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.project, "infrastructure");
        assert_eq!(resolved.app_name, "redis");
        assert_eq!(resolved.app.app_type, "redis");
    }

    // Test: Resolve app without project (unique name)
    #[test]
    fn test_resolve_app_unique_name() {
        let config = create_multi_project_config();
        
        let result = resolve_app(&config, "redis", None);
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.app_name, "redis");
    }

    // Test: Resolve non-existent app fails
    #[test]
    fn test_resolve_nonexistent_app() {
        let config = create_multi_project_config();
        
        let result = resolve_app(&config, "nonexistent", None);
        assert!(result.is_err());
        
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("not found in config"));
    }

    // Test: Resolve ambiguous app name (same name in multiple projects)
    #[test]
    fn test_resolve_ambiguous_app() {
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        // Create "api" in two different projects
        for project_name in &["project1", "project2"] {
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
                        k8s: None,
                    },
                    dependencies: Vec::new(),
                    defaults: Defaults {
                        local: Some("start".to_string()),
                        docker: None,
                        k8s: None,
                    },
                },
            );
            config.projects.insert(project_name.to_string(), Project { apps });
        }
        
        // Should fail without project specified
        let result = resolve_app(&config, "api", None);
        assert!(result.is_err());
        
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("ambiguous"));
        
        // Should work with project specified
        let result = resolve_app(&config, "api", Some("project1"));
        assert!(result.is_ok());
    }

    // Test: List all apps
    #[test]
    fn test_list_all_apps() {
        let config = create_multi_project_config();
        
        let apps = list_all_apps(&config);
        
        // Should have 2 apps total
        assert_eq!(apps.len(), 2);
        
        // Should contain both apps (tuple is: project_name, app_name, app)
        let app_names: Vec<&str> = apps.iter().map(|(_, name, _)| name.as_str()).collect();
        assert!(app_names.contains(&"redis"));
        assert!(app_names.contains(&"api"));
    }

    // Test: Simple dependency chain
    #[test]
    fn test_simple_dependency_chain() {
        let config = create_multi_project_config();
        
        let api_app = resolve_app(&config, "api", None).unwrap();
        let deps = dependencies::resolve_dependency_chain(&config, &api_app).unwrap();
        
        // Should have 1 dependency (redis)
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].app_name, "redis");
    }

    // Test: Complex dependency chain (A->B->C)
    #[test]
    fn test_complex_dependency_chain() {
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        let mut apps = HashMap::new();
        
        // C: no dependencies
        apps.insert(
            "c".to_string(),
            App {
                app_type: "redis".to_string(),
                path: "/tmp/c".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "redis-server".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: Vec::new(),
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        // B: depends on C
        apps.insert(
            "b".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/b".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "c".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        // A: depends on B
        apps.insert(
            "a".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/a".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "b".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        config.projects.insert("test".to_string(), Project { apps });
        
        // Resolve dependency chain for A
        let a_app = resolve_app(&config, "a", None).unwrap();
        let deps = dependencies::resolve_dependency_chain(&config, &a_app).unwrap();
        
        // Should have 2 dependencies (B and C)
        assert_eq!(deps.len(), 2);
        
        // Verify both B and C are in the chain
        let dep_names: Vec<&str> = deps.iter().map(|d| d.app_name.as_str()).collect();
        assert!(dep_names.contains(&"c"));
        assert!(dep_names.contains(&"b"));
        
        // In BFS, order isn't strictly guaranteed for this case, but C should generally come first
        // since B depends on C, but we'll just verify both are present
    }

    // Test: Circular dependency handling (A->B->A)
    // Note: Current implementation uses BFS with visited set, which prevents infinite loops
    // but doesn't explicitly detect/report circular dependencies
    #[test]
    fn test_circular_dependency_simple() {
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        let mut apps = HashMap::new();
        
        // A depends on B
        apps.insert(
            "a".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/a".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "b".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        // B depends on A (circular!)
        apps.insert(
            "b".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/b".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "a".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        config.projects.insert("test".to_string(), Project { apps });
        
        // Current implementation handles this gracefully (no infinite loop)
        // It returns both A and B in the dependency chain
        let a_app = resolve_app(&config, "a", None).unwrap();
        let result = dependencies::resolve_dependency_chain(&config, &a_app);
        
        // Should succeed (visited set prevents infinite loop)
        assert!(result.is_ok());
        let deps = result.unwrap();
        // Should have B in the chain
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].app_name, "b");
    }

    // Test: Complex circular dependency handling (A->B->C->A)
    // Current implementation handles this without explicit cycle detection
    #[test]
    fn test_circular_dependency_complex() {
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        let mut apps = HashMap::new();
        
        // A depends on B
        apps.insert(
            "a".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/a".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "b".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        // B depends on C
        apps.insert(
            "b".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/b".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "c".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        // C depends on A (circular!)
        apps.insert(
            "c".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/c".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "a".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        config.projects.insert("test".to_string(), Project { apps });
        
        // Should handle gracefully (visited set prevents infinite loop)
        let a_app = resolve_app(&config, "a", None).unwrap();
        let result = dependencies::resolve_dependency_chain(&config, &a_app);
        
        assert!(result.is_ok());
        let deps = result.unwrap();
        // Should have B and C in the chain
        assert_eq!(deps.len(), 2);
    }

    // Test: Diamond dependency (A depends on B and C, both depend on D)
    #[test]
    fn test_diamond_dependency() {
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        let mut apps = HashMap::new();
        
        // D: no dependencies (base)
        apps.insert(
            "d".to_string(),
            App {
                app_type: "redis".to_string(),
                path: "/tmp/d".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "redis-server".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: Vec::new(),
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        // B depends on D
        apps.insert(
            "b".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/b".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "d".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        // C depends on D
        apps.insert(
            "c".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/c".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "d".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        // A depends on B and C
        apps.insert(
            "a".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/a".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![
                    Dependency {
                        project: "test".to_string(),
                        app: "b".to_string(),
                    },
                    Dependency {
                        project: "test".to_string(),
                        app: "c".to_string(),
                    },
                ],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        config.projects.insert("test".to_string(), Project { apps });
        
        // Resolve dependency chain for A
        let a_app = resolve_app(&config, "a", None).unwrap();
        let deps = dependencies::resolve_dependency_chain(&config, &a_app).unwrap();
        
        // Should have D, B, C (D appears only once despite being referenced twice)
        assert_eq!(deps.len(), 3);
        
        let dep_names: Vec<&str> = deps.iter().map(|d| d.app_name.as_str()).collect();
        assert!(dep_names.contains(&"d"));
        assert!(dep_names.contains(&"b"));
        assert!(dep_names.contains(&"c"));
        
        // Verify D is only included once (not duplicated)
        assert_eq!(dep_names.iter().filter(|&&n| n == "d").count(), 1);
    }

    // Test: Missing dependency app
    #[test]
    fn test_missing_dependency() {
        let mut config = Config {
            projects: HashMap::new(),
        };
        
        let mut apps = HashMap::new();
        
        // App depends on non-existent dependency
        apps.insert(
            "app".to_string(),
            App {
                app_type: "nodejs".to_string(),
                path: "/tmp/app".to_string(),
                commands: Commands {
                    local: Some({
                        let mut cmds = HashMap::new();
                        cmds.insert("start".to_string(), "npm start".to_string());
                        cmds
                    }),
                    docker: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "nonexistent".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    k8s: None,
                },
            },
        );
        
        config.projects.insert("test".to_string(), Project { apps });
        
        // Should fail when resolving dependency chain
        let app = resolve_app(&config, "app", None).unwrap();
        let result = dependencies::resolve_dependency_chain(&config, &app);
        
        assert!(result.is_err());
    }
}

