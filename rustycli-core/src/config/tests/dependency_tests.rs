// Unit tests for dependency chain resolution
// Tests dependencies::resolve_dependency_chain functionality

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
}