// Unit tests for app resolution functionality
// Tests resolve_app, get_app_by_project, and list_all_apps functions

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

    // Test: Resolve ambiguous app name with project filter
    // When the same app name exists in multiple projects, specifying the project should work
    #[test]
    fn test_resolve_ambiguous_app_with_project_filter() {
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
        
        // Should work with project specified for project1
        let result = resolve_app(&config, "api", Some("project1"));
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.project, "project1");
        assert_eq!(resolved.app_name, "api");
        
        // Should work with project specified for project2
        let result = resolve_app(&config, "api", Some("project2"));
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert_eq!(resolved.project, "project2");
        assert_eq!(resolved.app_name, "api");
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
}