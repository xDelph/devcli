//! Unit tests for the dependencies module

#[cfg(test)]
mod tests {
    use super::super::resolver::*;
    use crate::config::resolver::ResolvedApp;
    use crate::test_utils::AppBuilder;

    fn create_test_app_to_start(app_name: &str, project: &str) -> AppToStart {
        let app = AppBuilder::new("test", "/tmp")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .build();

        let resolved_app = ResolvedApp {
            app,
            app_name: app_name.to_string(),
            project: project.to_string(),
            worktree: None,
        };

        AppToStart {
            resolved_app,
            command: "npm start".to_string(),
            default_command: "start".to_string(),
        }
    }

    #[test]
    fn test_app_to_start_structure() {
        let app_to_start = create_test_app_to_start("test-app", "test-project");
        assert_eq!(app_to_start.resolved_app.app_name, "test-app");
        assert_eq!(app_to_start.resolved_app.project, "test-project");
        assert_eq!(app_to_start.command, "npm start");
        assert_eq!(app_to_start.default_command, "start");
    }

    #[test]
    fn test_empty_apps_to_start() {
        // Test that empty apps list doesn't cause issues
        let apps_to_start: Vec<AppToStart> = vec![];
        // This should not panic or cause issues
        assert_eq!(apps_to_start.len(), 0);
    }

    #[test]
    fn test_multiple_apps_to_start() {
        let apps_to_start = [
            create_test_app_to_start("app1", "project1"),
            create_test_app_to_start("app2", "project2"),
        ];

        assert_eq!(apps_to_start.len(), 2);
        assert_eq!(apps_to_start[0].resolved_app.app_name, "app1");
        assert_eq!(apps_to_start[1].resolved_app.app_name, "app2");
    }

    // Note: Integration tests for handle_dependencies would require
    // setting up config files and process tracking, which is better
    // suited for integration tests rather than unit tests.
    // The core logic is tested through the existing integration test suite.

    fn chain_config() -> crate::config::Config {
        use crate::test_utils::ConfigBuilder;
        let leaf = AppBuilder::new("redis", "/tmp")
            .with_local_command("start", "redis-server")
            .build();
        let api = AppBuilder::new("nodejs", "/tmp")
            .with_local_command("start", "npm start")
            .with_dependency("proj", "redis")
            .build();
        let worker = AppBuilder::new("nodejs", "/tmp")
            .with_local_command("start", "npm run worker")
            .with_dependency("proj", "api")
            .build();
        let other = AppBuilder::new("nodejs", "/tmp")
            .with_local_command("start", "npm start")
            .build();
        ConfigBuilder::new()
            .with_app("proj", "redis", leaf)
            .with_app("proj", "api", api)
            .with_app("proj", "worker", worker)
            .with_app("proj", "other", other)
            .build()
    }

    #[test]
    fn test_start_waves_respect_dependency_order() {
        let config = chain_config();
        let missing = vec![
            "proj/worker".to_string(),
            "proj/api".to_string(),
            "proj/other".to_string(),
            "proj/redis".to_string(),
        ];
        let waves = super::super::dependencies::start_waves(&config, &missing).unwrap();

        let position = |key: &str| waves.iter().position(|w| w.iter().any(|k| k == key));
        assert!(position("proj/redis") < position("proj/api"));
        assert!(position("proj/api") < position("proj/worker"));
        // Independent apps start in the first wave.
        assert_eq!(position("proj/other"), Some(0));
        assert_eq!(waves.iter().map(Vec::len).sum::<usize>(), 4);
    }

    #[test]
    fn test_start_waves_only_order_missing_apps() {
        // redis is already running (not missing): api is unblocked.
        let config = chain_config();
        let missing = vec!["proj/api".to_string(), "proj/worker".to_string()];
        let waves = super::super::dependencies::start_waves(&config, &missing).unwrap();
        assert_eq!(
            waves,
            vec![
                vec!["proj/api".to_string()],
                vec!["proj/worker".to_string()]
            ]
        );
    }
}
