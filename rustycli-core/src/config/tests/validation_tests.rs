// Unit tests for validation and error handling
// Tests circular dependency detection, missing dependencies, and error cases

#[cfg(test)]
mod tests {
    use crate::config::*;
    use std::collections::HashMap;

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
                    orbstack: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "b".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
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
                    orbstack: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "a".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
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
                    orbstack: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "b".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
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
                    orbstack: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "c".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
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
                    orbstack: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "a".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
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
                    orbstack: None,
                    k8s: None,
                },
                dependencies: vec![Dependency {
                    project: "test".to_string(),
                    app: "nonexistent".to_string(),
                }],
                defaults: Defaults {
                    local: Some("start".to_string()),
                    docker: None,
                    orbstack: None,
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
