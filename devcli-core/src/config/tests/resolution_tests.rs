// Unit tests for app resolution functionality
// Tests resolve_app, get_app_by_project, and list_all_apps functions

#[cfg(test)]
mod tests {
    use crate::config::*;
    use crate::test_utils::{AppBuilder, ConfigBuilder};
    use std::collections::HashMap;

    // Helper: Create config with multiple projects and apps
    fn create_multi_project_config() -> Config {
        // Project 1: infrastructure
        let redis = AppBuilder::new("redis", "/tmp/redis")
            .with_local_command("start", "redis-server")
            .with_local_default("start")
            .build();

        // Project 2: api (depends on redis)
        let api = AppBuilder::new("nodejs", "/tmp/api")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .with_dependency("infrastructure", "redis")
            .build();

        ConfigBuilder::new()
            .with_app("infrastructure", "redis", redis)
            .with_app("api-project", "api", api)
            .build()
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
                        orbstack: None,
                        k8s: None,
                        ..Default::default()
                    },
                    dependencies: Vec::new(),
                    defaults: Defaults {
                        local: Some("start".to_string()),
                        docker: None,
                        orbstack: None,
                        k8s: None,
                        ..Default::default()
                    },
                    dockerfile_path: None,
                    env_files: None,
                    default_stages: None,
                    alternative_name: None,
                    health_check: None,
                    restart_policy: None,
                },
            );
            config.projects.insert(
                project_name.to_string(),
                Project {
                    apps,
                    alternative_name: None,
                },
            );
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
    }

    // Helper: single project with an app that has an alternative_name
    fn config_with_alternate_named_app() -> Config {
        let mut config = Config {
            projects: HashMap::new(),
        };
        let mut projects = HashMap::new();
        let mut apps = HashMap::new();

        let mut api = AppBuilder::new("nodejs", "/tmp/api")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .build();
        api.alternative_name = Some("api-alias".to_string());
        apps.insert("api".to_string(), api);

        projects.insert(
            "p1".to_string(),
            Project {
                apps,
                alternative_name: None,
            },
        );
        config.projects = projects;
        config
    }

    // Test: pick_app_in_project resolves by exact app name
    #[test]
    fn test_pick_app_in_project_exact_name() {
        let config = create_multi_project_config();
        let resolved = pick_app_in_project(&config, "infrastructure", "redis").unwrap();
        assert_eq!(resolved.project, "infrastructure");
        assert_eq!(resolved.app_name, "redis");
        assert_eq!(resolved.app.app_type, "redis");
    }

    // Test: pick_app_in_project resolves via alternative_name and returns the
    // actual app key (not the alias)
    #[test]
    fn test_pick_app_in_project_by_alternative_name() {
        let config = config_with_alternate_named_app();
        let resolved = pick_app_in_project(&config, "p1", "api-alias").unwrap();
        assert_eq!(resolved.project, "p1");
        assert_eq!(resolved.app_name, "api");
        assert_eq!(resolved.app.app_type, "nodejs");
    }

    // Test: pick_app_in_project errors on unknown project or app
    #[test]
    fn test_pick_app_in_project_missing() {
        let config = create_multi_project_config();
        assert!(pick_app_in_project(&config, "nope", "redis").is_err());
        assert!(pick_app_in_project(&config, "infrastructure", "nope").is_err());
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
