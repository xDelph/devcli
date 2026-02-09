// Unit tests for app type detection
// Tests detection of different application types (Node.js, Python, Redis, Traefik, NX)

#[cfg(test)]
mod tests {
    use crate::detection::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper: Create a temporary directory for testing
    fn create_temp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    // Helper: Create a package.json file
    fn create_package_json(dir: &std::path::Path, name: &str, scripts: Vec<(&str, &str)>) {
        let mut pkg = serde_json::json!({
            "name": name,
            "version": "1.0.0",
            "scripts": {}
        });

        for (key, value) in scripts {
            pkg["scripts"][key] = serde_json::json!(value);
        }

        let content = serde_json::to_string_pretty(&pkg).unwrap();
        fs::write(dir.join("package.json"), content).unwrap();
    }

    // Test: Detect Node.js app
    #[test]
    fn test_detect_nodejs_app() {
        let dir = create_temp_dir();
        create_package_json(
            dir.path(),
            "test-app",
            vec![("start", "node server.js"), ("test", "jest")],
        );

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "nodejs");
        assert_eq!(detected.app_name, "test-app");
        assert!(detected.local_commands.is_some());

        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"));
        assert!(local_cmds.contains_key("test"));
    }

    // Test: Detect NX monorepo
    #[test]
    fn test_detect_nx_monorepo() {
        let dir = create_temp_dir();

        // Create nx.json
        fs::write(dir.path().join("nx.json"), r#"{"version": 2}"#).unwrap();

        // Create package.json
        create_package_json(dir.path(), "nx-workspace", vec![("start", "nx serve")]);

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "nx");
    }

    // Test: Detect Python app
    #[test]
    fn test_detect_python_app() {
        let dir = create_temp_dir();

        // Create requirements.txt
        fs::write(dir.path().join("requirements.txt"), "flask\npytest").unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "python");

        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("install"));
    }

    // Test: Detect Python app with pyproject.toml
    #[test]
    fn test_detect_python_with_pyproject() {
        let dir = create_temp_dir();

        // Create pyproject.toml
        fs::write(
            dir.path().join("pyproject.toml"),
            r#"[project]
name = "my-python-app"
version = "1.0.0"
"#,
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "python");
        assert_eq!(detected.app_name, "my-python-app");
    }

    // Test: Detect Redis app
    #[test]
    fn test_detect_redis_app() {
        let dir = create_temp_dir();

        // Create redis.conf
        fs::write(dir.path().join("redis.conf"), "port 6379\n").unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "redis");
    }

    // Test: Detect Traefik app
    #[test]
    fn test_detect_traefik_app() {
        let dir = create_temp_dir();

        // Create traefik.yml
        fs::write(dir.path().join("traefik.yml"), "api:\n  dashboard: true\n").unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "traefik");
    }

    // Test: Scoped package name handling
    #[test]
    fn test_scoped_package_name() {
        let dir = create_temp_dir();

        // Create package.json with scoped name
        let pkg = serde_json::json!({
            "name": "@myorg/my-package",
            "scripts": {
                "start": "node server.js"
            }
        });

        fs::write(
            dir.path().join("package.json"),
            serde_json::to_string_pretty(&pkg).unwrap(),
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();

        // Should extract just the package name without scope
        assert_eq!(detected.app_name, "my-package");
    }

    // Test: Explicit Redis conf file detection
    #[test]
    fn test_redis_conf_file_detection() {
        let dir = create_temp_dir();

        // Create redis.conf file
        fs::write(
            dir.path().join("redis.conf"),
            "port 6379\nbind 127.0.0.1\nsave 900 1\n",
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "redis");

        let local_cmds = detected.local_commands.unwrap();
        assert!(
            local_cmds.contains_key("start"),
            "Should have start command"
        );

        let start_cmd = local_cmds.get("start").unwrap();

        // Verify the start command uses redis.conf (since config file exists)
        assert!(
            start_cmd.contains("redis.conf"),
            "Start command should reference redis.conf file"
        );
        assert!(
            start_cmd.starts_with("redis-server"),
            "Should use redis-server command"
        );
    }

    // Test: Explicit Traefik TOML file detection
    #[test]
    fn test_traefik_toml_file_detection() {
        let dir = create_temp_dir();

        // Create traefik.toml file
        fs::write(
            dir.path().join("traefik.toml"),
            r#"[api]
dashboard = true
debug = true

[entryPoints]
  [entryPoints.web]
  address = ":80"
  
  [entryPoints.websecure]
  address = ":443"
"#,
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "traefik");

        let local_cmds = detected.local_commands.unwrap();
        let start_cmd = local_cmds.get("start").unwrap();

        // Verify the start command uses traefik.toml
        assert!(
            start_cmd.contains("traefik.toml"),
            "Start command should reference traefik.toml file"
        );
        assert!(
            start_cmd.contains("--configFile="),
            "Should use --configFile flag"
        );
    }

    // Test: Redis local config file detection (redis.local.conf)
    #[test]
    fn test_redis_local_conf_detection() {
        let dir = create_temp_dir();

        // Create redis.local.conf file
        fs::write(
            dir.path().join("redis.local.conf"),
            "port 6379\nbind 127.0.0.1\nsave 900 1\nlogfile redis.local.log\n",
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "redis");

        let local_cmds = detected.local_commands.unwrap();
        let start_cmd = local_cmds.get("start").unwrap();

        // Verify the start command uses redis.local.conf (since config file exists)
        assert!(
            start_cmd.contains("redis.local.conf"),
            "Start command should reference redis.local.conf file"
        );
    }

    // Test: Traefik local TOML file detection (traefik.local.toml)
    #[test]
    fn test_traefik_local_toml_detection() {
        let dir = create_temp_dir();

        // Create traefik.local.toml file
        fs::write(
            dir.path().join("traefik.local.toml"),
            r#"[log]
level = "DEBUG"

[providers]
  [providers.file]
  filename = "traefik-dynamic.local.toml"
  watch = true
"#,
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "traefik");

        let local_cmds = detected.local_commands.unwrap();
        let start_cmd = local_cmds.get("start").unwrap();

        // Verify the start command uses traefik.local.toml
        assert!(
            start_cmd.contains("traefik.local.toml"),
            "Start command should reference traefik.local.toml file"
        );
    }

    // Test: Multiple local config patterns
    #[test]
    fn test_multiple_local_config_patterns() {
        // Test redis.dev.conf
        let dir_redis_dev = create_temp_dir();
        fs::write(dir_redis_dev.path().join("redis.dev.conf"), "port 6380\n").unwrap();
        let result = detect_app(dir_redis_dev.path());
        assert_eq!(result.unwrap().app_type, "redis");

        // Test traefik.staging.yml
        let dir_traefik_staging = create_temp_dir();
        fs::write(
            dir_traefik_staging.path().join("traefik.staging.yml"),
            "api:\n  dashboard: true\n",
        )
        .unwrap();
        let result = detect_app(dir_traefik_staging.path());
        assert_eq!(result.unwrap().app_type, "traefik");

        // Test redis.production.config
        let dir_redis_prod = create_temp_dir();
        fs::write(
            dir_redis_prod.path().join("redis.production.config"),
            "port 6379\n",
        )
        .unwrap();
        let result = detect_app(dir_redis_prod.path());
        assert_eq!(result.unwrap().app_type, "redis");
    }

    // Test: Multiple Redis config files (should pick first found)
    #[test]
    fn test_redis_multiple_configs() {
        let dir = create_temp_dir();

        // Create multiple redis config files
        fs::write(dir.path().join("redis.conf"), "port 6379\n").unwrap();
        fs::write(dir.path().join("redis-custom.conf"), "port 6380\n").unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "redis");

        let local_cmds = detected.local_commands.unwrap();
        let start_cmd = local_cmds.get("start").unwrap();

        // Should have a start command with config file (exact file depends on filesystem order)
        assert!(start_cmd.contains("redis") && start_cmd.contains(".conf"));
    }
}
