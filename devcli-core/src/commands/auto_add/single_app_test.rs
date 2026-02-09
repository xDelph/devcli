// Unit tests for single_app module
// Tests single app discovery, Redis/Traefik config detection, and app creation logic

#[cfg(test)]
mod tests {
    use crate::commands::auto_add::single_app::discover_all_apps;
    use std::fs;
    use tempfile::TempDir;

    // Helper: Create a temporary directory for testing
    fn create_temp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_discover_multiple_apps() {
        let root_dir = create_temp_dir();

        // Create a Node.js app in root
        let pkg = serde_json::json!({
            "name": "root-app",
            "version": "1.0.0",
            "scripts": {
                "start": "node server.js"
            }
        });
        fs::write(
            root_dir.path().join("package.json"),
            serde_json::to_string_pretty(&pkg).unwrap(),
        )
        .unwrap();

        // Create a Redis app in subdirectory
        fs::create_dir(root_dir.path().join("redis-service")).unwrap();
        fs::write(
            root_dir.path().join("redis-service/redis.conf"),
            "port 6379\n",
        )
        .unwrap();

        // Create a Traefik app in another subdirectory
        fs::create_dir(root_dir.path().join("proxy")).unwrap();
        fs::write(
            root_dir.path().join("proxy/traefik.yml"),
            "api:\n  dashboard: true\n",
        )
        .unwrap();

        // Test discovery
        let discovered = discover_all_apps(root_dir.path());
        assert!(discovered.is_ok());

        let apps = discovered.unwrap();
        assert_eq!(apps.len(), 3, "Should discover 3 apps");

        // Verify app types
        let app_types: Vec<&str> = apps.iter().map(|app| app.app_type.as_str()).collect();
        assert!(app_types.contains(&"nodejs"));
        assert!(app_types.contains(&"redis"));
        assert!(app_types.contains(&"traefik"));
    }

    #[test]
    fn test_discovery_skips_common_directories() {
        let root_dir = create_temp_dir();

        // Create a valid app in root
        fs::write(root_dir.path().join("redis.conf"), "port 6379\n").unwrap();

        // Create directories that should be skipped
        for skip_dir in &[
            "node_modules",
            ".git",
            "target",
            "dist",
            "build",
            ".next",
            "coverage",
        ] {
            fs::create_dir(root_dir.path().join(skip_dir)).unwrap();
            // Add a fake config file that would normally be detected
            fs::write(
                root_dir.path().join(skip_dir).join("package.json"),
                r#"{"name": "fake-app", "scripts": {"start": "node index.js"}}"#,
            )
            .unwrap();
        }

        // Test discovery
        let discovered = discover_all_apps(root_dir.path());
        assert!(discovered.is_ok());

        let apps = discovered.unwrap();
        assert_eq!(
            apps.len(),
            1,
            "Should only discover the root redis app, skipping common directories"
        );
        assert_eq!(apps[0].app_type, "redis");
    }

    #[test]
    fn test_discovery_multiple_app_types_same_directory() {
        let root_dir = create_temp_dir();

        // Create redis subdirectory with multiple configs
        fs::create_dir(root_dir.path().join("redis")).unwrap();
        fs::write(root_dir.path().join("redis/redis.conf"), "port 6379\n").unwrap();
        fs::write(
            root_dir.path().join("redis/redis.local.conf"),
            "port 6380\n",
        )
        .unwrap();

        // Create traefik configs in root
        fs::write(
            root_dir.path().join("traefik-dynamic.local.toml"),
            "[api]\ndashboard = true\n",
        )
        .unwrap();
        fs::write(
            root_dir.path().join("traefik.local.toml"),
            "[api]\ndashboard = true\n",
        )
        .unwrap();

        // Test discovery
        let discovered = discover_all_apps(root_dir.path());
        assert!(discovered.is_ok());

        let apps = discovered.unwrap();

        // Should find both redis (from subdirectory) and traefik (from root)
        assert!(
            apps.len() >= 2,
            "Should discover at least 2 apps, found: {}",
            apps.len()
        );

        let app_types: Vec<&str> = apps.iter().map(|app| app.app_type.as_str()).collect();
        assert!(app_types.contains(&"redis"), "Should find redis app");
        assert!(app_types.contains(&"traefik"), "Should find traefik app");
    }

    #[test]
    fn test_redis_config_file_detection() {
        let root_dir = create_temp_dir();

        // Create multiple Redis config files
        fs::write(root_dir.path().join("redis.conf"), "port 6379\n").unwrap();
        fs::write(root_dir.path().join("redis.local.conf"), "port 6380\n").unwrap();
        fs::write(root_dir.path().join("redis-prod.config"), "port 6381\n").unwrap();

        let discovered = discover_all_apps(root_dir.path());
        assert!(discovered.is_ok());

        let apps = discovered.unwrap();

        // Should find individual Redis apps for each config file
        let redis_apps: Vec<_> = apps.iter().filter(|app| app.app_type == "redis").collect();
        assert!(redis_apps.len() >= 2, "Should find multiple Redis apps");

        // Verify app names are derived from config filenames
        let app_names: Vec<&str> = redis_apps.iter().map(|app| app.app_name.as_str()).collect();
        assert!(app_names.contains(&"redis"));
        assert!(app_names.contains(&"redis.local"));
    }

    #[test]
    fn test_traefik_config_file_detection() {
        let root_dir = create_temp_dir();

        // Create multiple Traefik config files
        fs::write(
            root_dir.path().join("traefik.yml"),
            "api:\n  dashboard: true\n",
        )
        .unwrap();
        fs::write(
            root_dir.path().join("traefik-dynamic.yaml"),
            "api:\n  dashboard: true\n",
        )
        .unwrap();
        fs::write(
            root_dir.path().join("traefik.local.toml"),
            "[api]\ndashboard = true\n",
        )
        .unwrap();

        let discovered = discover_all_apps(root_dir.path());
        assert!(discovered.is_ok());

        let apps = discovered.unwrap();

        // Should find individual Traefik apps for each config file
        let traefik_apps: Vec<_> = apps
            .iter()
            .filter(|app| app.app_type == "traefik")
            .collect();
        assert!(traefik_apps.len() >= 2, "Should find multiple Traefik apps");

        // Verify app names are derived from config filenames
        let app_names: Vec<&str> = traefik_apps
            .iter()
            .map(|app| app.app_name.as_str())
            .collect();
        assert!(app_names.contains(&"traefik"));
        assert!(app_names.contains(&"traefik-dynamic"));
    }

    #[test]
    fn test_discover_empty_directory() {
        let root_dir = create_temp_dir();

        // Empty directory should return empty results
        let discovered = discover_all_apps(root_dir.path());
        assert!(discovered.is_ok());

        let apps = discovered.unwrap();
        assert_eq!(apps.len(), 0, "Empty directory should discover no apps");
    }

    #[test]
    fn test_discover_mixed_standard_and_config_apps() {
        let root_dir = create_temp_dir();

        // Create a standard Node.js app
        let pkg = serde_json::json!({
            "name": "web-app",
            "scripts": {
                "start": "npm start",
                "dev": "npm run dev"
            }
        });
        fs::write(
            root_dir.path().join("package.json"),
            serde_json::to_string_pretty(&pkg).unwrap(),
        )
        .unwrap();

        // Create individual config files
        fs::write(root_dir.path().join("redis.local.conf"), "port 6379\n").unwrap();
        fs::write(
            root_dir.path().join("traefik.yml"),
            "api:\n  dashboard: true\n",
        )
        .unwrap();

        let discovered = discover_all_apps(root_dir.path());
        assert!(discovered.is_ok());

        let apps = discovered.unwrap();
        assert_eq!(
            apps.len(),
            3,
            "Should discover 3 apps: nodejs, redis, traefik"
        );

        let app_types: Vec<&str> = apps.iter().map(|app| app.app_type.as_str()).collect();
        assert!(app_types.contains(&"nodejs"));
        assert!(app_types.contains(&"redis"));
        assert!(app_types.contains(&"traefik"));
    }
}
