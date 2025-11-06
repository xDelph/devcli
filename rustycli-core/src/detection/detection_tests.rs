// Unit tests for detection module
// Tests app type detection, command generation, and environment detection

#[cfg(test)]
mod tests {
    use super::super::*;
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
        create_package_json(dir.path(), "test-app", vec![
            ("start", "node server.js"),
            ("test", "jest"),
        ]);
        
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
        create_package_json(dir.path(), "nx-workspace", vec![
            ("start", "nx serve"),
        ]);
        
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
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"));
        assert!(local_cmds.contains_key("install"));
    }

    // Test: Detect Python app with pyproject.toml
    #[test]
    fn test_detect_python_with_pyproject() {
        let dir = create_temp_dir();
        
        // Create pyproject.toml
        fs::write(
            dir.path().join("pyproject.toml"),
            r#"
[project]
name = "my-python-app"
version = "1.0.0"
"#
        ).unwrap();
        
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
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"));
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
        assert!(detected.local_commands.is_some());
    }

    // Test: Docker detection with Dockerfile
    #[test]
    fn test_detect_docker_simple() {
        let dir = create_temp_dir();
        
        // Create package.json (nodejs app)
        create_package_json(dir.path(), "docker-app", vec![("start", "node server.js")]);
        
        // Create simple Dockerfile
        fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:18\nCOPY . .\nCMD [\"npm\", \"start\"]\n"
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert!(detected.docker_commands.is_some());
        
        let docker_cmds = detected.docker_commands.unwrap();
        assert!(docker_cmds.contains_key("build"));
        assert!(docker_cmds.contains_key("run"));
        assert!(docker_cmds.contains_key("stop"));
    }

    // Test: Docker multi-stage detection
    #[test]
    fn test_detect_docker_multistage() {
        let dir = create_temp_dir();
        
        // Create package.json
        create_package_json(dir.path(), "multistage-app", vec![("start", "node server.js")]);
        
        // Create multi-stage Dockerfile
        fs::write(
            dir.path().join("Dockerfile"),
            r#"FROM node:18 AS build
RUN npm install
RUN npm run build

FROM node:18 AS test
RUN npm test

FROM node:18-slim AS production
COPY --from=build /app/dist .
CMD ["node", "server.js"]
"#
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert!(detected.docker_commands.is_some());
        
        let docker_cmds = detected.docker_commands.unwrap();
        
        // Should have stage-specific commands
        assert!(docker_cmds.contains_key("build"), "Should have 'build' stage command");
        assert!(docker_cmds.contains_key("test"), "Should have 'test' stage command");
        assert!(docker_cmds.contains_key("production"), "Should have 'production' stage command");
        
        // Test stage should have a run command
        assert!(docker_cmds.contains_key("test-run"), "Should have 'test-run' command");
        
        // Should still have general commands
        assert!(docker_cmds.contains_key("run"));
    }

    // Test: K8s detection with k8s directory
    #[test]
    fn test_detect_k8s_with_directory() {
        let dir = create_temp_dir();
        
        // Create package.json
        create_package_json(dir.path(), "k8s-app", vec![("start", "node server.js")]);
        
        // Create k8s directory with manifests
        fs::create_dir(dir.path().join("k8s")).unwrap();
        fs::write(
            dir.path().join("k8s/deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n"
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert!(detected.k8s_commands.is_some());
        
        let k8s_cmds = detected.k8s_commands.unwrap();
        assert!(k8s_cmds.contains_key("apply"));
        assert!(k8s_cmds.contains_key("delete"));
        assert!(k8s_cmds.contains_key("restart"));
    }

    // Test: K8s detection with .k8s.yaml files
    #[test]
    fn test_detect_k8s_with_suffix() {
        let dir = create_temp_dir();
        
        // Create package.json
        create_package_json(dir.path(), "k8s-app2", vec![("start", "node server.js")]);
        
        // Create .k8s.yaml file
        fs::write(
            dir.path().join("deployment.k8s.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n"
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert!(detected.k8s_commands.is_some());
    }

    // Test: App with all three environments
    #[test]
    fn test_detect_all_environments() {
        let dir = create_temp_dir();
        
        // Create package.json
        create_package_json(dir.path(), "full-app", vec![
            ("start", "node server.js"),
            ("test", "jest"),
        ]);
        
        // Create Dockerfile
        fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:18\nCMD [\"npm\", \"start\"]\n"
        ).unwrap();
        
        // Create k8s directory
        fs::create_dir(dir.path().join("k8s")).unwrap();
        fs::write(
            dir.path().join("k8s/deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n"
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert!(detected.local_commands.is_some(), "Should have local commands");
        assert!(detected.docker_commands.is_some(), "Should have docker commands");
        assert!(detected.k8s_commands.is_some(), "Should have k8s commands");
    }

    // Test: Default command suggestions
    #[test]
    fn test_default_command_suggestions() {
        let dir = create_temp_dir();
        
        // Create Node.js app with common commands
        create_package_json(dir.path(), "defaults-test", vec![
            ("dev", "node dev.js"),
            ("start", "node server.js"),
            ("serve", "node serve.js"),
        ]);
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert!(detected.suggested_local_default.is_some());
        
        // Should prefer "serve" over "start" for Node.js
        let default = detected.suggested_local_default.unwrap();
        assert_eq!(default, "serve");
    }

    // Test: Scoped package name handling
    #[test]
    fn test_scoped_package_name() {
        let dir = create_temp_dir();
        
        // Create package.json with scoped name
        let pkg = serde_json::json!({
            "name": "@myorg/my-package",
            "version": "1.0.0",
            "scripts": {
                "start": "node server.js"
            }
        });
        fs::write(
            dir.path().join("package.json"),
            serde_json::to_string_pretty(&pkg).unwrap()
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        // Should extract just the package name without scope
        assert_eq!(detected.app_name, "my-package");
    }

    // Test: App with no commands detected
    #[test]
    fn test_app_no_commands() {
        let dir = create_temp_dir();
        
        // Create package.json with no scripts
        let pkg = serde_json::json!({
            "name": "no-scripts-app",
            "version": "1.0.0"
        });
        fs::write(
            dir.path().join("package.json"),
            serde_json::to_string_pretty(&pkg).unwrap()
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        // Should have app type but no local commands
        assert_eq!(detected.app_type, "nodejs");
        assert!(detected.local_commands.is_none() || detected.local_commands.as_ref().unwrap().is_empty());
    }

    // Test: Invalid directory
    #[test]
    fn test_detect_invalid_directory() {
        let result = detect_app(std::path::Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    // Test: Empty directory (no app detected)
    #[test]
    fn test_detect_empty_directory() {
        let dir = create_temp_dir();
        
        let result = detect_app(dir.path());
        assert!(result.is_err());
    }

    // Test: Docker port mapping for different app types
    #[test]
    fn test_docker_port_mappings() {
        // Test Node.js port (3000)
        let dir_node = create_temp_dir();
        create_package_json(dir_node.path(), "node-app", vec![("start", "node server.js")]);
        fs::write(dir_node.path().join("Dockerfile"), "FROM node:18\n").unwrap();
        
        let detected = detect_app(dir_node.path()).unwrap();
        let docker_cmds = detected.docker_commands.unwrap();
        let run_cmd = docker_cmds.get("run").unwrap();
        assert!(run_cmd.contains("3000:3000"), "Node.js should use port 3000");
        
        // Test Python port (8000)
        let dir_python = create_temp_dir();
        fs::write(dir_python.path().join("requirements.txt"), "flask\n").unwrap();
        fs::write(dir_python.path().join("Dockerfile"), "FROM python:3.11\n").unwrap();
        
        let detected = detect_app(dir_python.path()).unwrap();
        let docker_cmds = detected.docker_commands.unwrap();
        let run_cmd = docker_cmds.get("run").unwrap();
        assert!(run_cmd.contains("8000:8000"), "Python should use port 8000");
    }

    // Test: Directory name fallback for app name
    #[test]
    fn test_app_name_from_directory() {
        let dir = create_temp_dir();
        let dir_path = dir.path().to_path_buf();
        
        // Create redis.conf (no name in config file)
        fs::write(dir_path.join("redis.conf"), "port 6379\n").unwrap();
        
        let result = detect_app(&dir_path);
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        // App name should be derived from directory name
        assert!(!detected.app_name.is_empty());
    }

    // Test: Dockerfile in subdirectory
    #[test]
    fn test_dockerfile_in_subdirectory() {
        let dir = create_temp_dir();
        
        // Create package.json in root
        create_package_json(dir.path(), "subdir-docker", vec![("start", "node server.js")]);
        
        // Create Dockerfile in subdirectory (should still be found)
        fs::create_dir(dir.path().join("docker")).unwrap();
        fs::write(
            dir.path().join("docker/Dockerfile"),
            "FROM node:18\n"
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert!(detected.docker_commands.is_some(), "Should find Dockerfile in subdirectory");
    }

    // Test: Multiple k8s files
    #[test]
    fn test_multiple_k8s_files() {
        let dir = create_temp_dir();
        
        // Create package.json
        create_package_json(dir.path(), "multi-k8s", vec![("start", "node server.js")]);
        
        // Create k8s directory with multiple files
        fs::create_dir(dir.path().join("k8s")).unwrap();
        fs::write(dir.path().join("k8s/deployment.yaml"), "kind: Deployment\n").unwrap();
        fs::write(dir.path().join("k8s/service.yaml"), "kind: Service\n").unwrap();
        fs::write(dir.path().join("k8s/ingress.yaml"), "kind: Ingress\n").unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert!(detected.k8s_commands.is_some());
        
        // Commands should reference the k8s directory (not individual files)
        let k8s_cmds = detected.k8s_commands.unwrap();
        let apply_cmd = k8s_cmds.get("apply").unwrap();
        assert!(apply_cmd.contains("k8s/"));
    }

    // Test: Redis detection in subdirectory
    #[test]
    fn test_detect_redis_in_subdirectory() {
        let dir = create_temp_dir();
        
        // Create redis subdirectory with config
        fs::create_dir(dir.path().join("redis")).unwrap();
        fs::write(dir.path().join("redis/redis.conf"), "port 6379\nbind 127.0.0.1\n").unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "redis");
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"));
        
        // Should reference the subdirectory config in the start command
        let start_cmd = local_cmds.get("start").unwrap();
        assert!(start_cmd.contains("redis/redis.conf"));
    }

    // Test: Traefik detection in subdirectory
    #[test]
    fn test_detect_traefik_in_subdirectory() {
        let dir = create_temp_dir();
        
        // Create traefik subdirectory with config
        fs::create_dir(dir.path().join("traefik")).unwrap();
        fs::write(
            dir.path().join("traefik/traefik.yml"),
            "api:\n  dashboard: true\nentryPoints:\n  web:\n    address: \":80\"\n"
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "traefik");
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"));
        
        // Should reference the subdirectory config
        let start_cmd = local_cmds.get("start").unwrap();
        assert!(start_cmd.contains("traefik/traefik.yml"));
    }

    // Test: Redis detection with custom filename patterns
    #[test]
    fn test_detect_redis_custom_patterns() {
        let dir = create_temp_dir();
        
        // Create redis config with custom name
        fs::write(dir.path().join("redis-production.conf"), "port 6379\n").unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "redis");
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"));
        
        // Should use the custom config file in the start command
        let start_cmd = local_cmds.get("start").unwrap();
        assert!(start_cmd.contains("redis-production.conf"));
    }

    // Test: Traefik detection with custom filename patterns
    #[test]
    fn test_detect_traefik_custom_patterns() {
        let dir = create_temp_dir();
        
        // Create traefik config with custom name
        fs::write(
            dir.path().join("traefik-dynamic.yml"),
            "http:\n  routers:\n    api:\n      rule: \"Host(`traefik.localhost`)\"\n"
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "traefik");
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"));
        
        // Should use the custom config file
        let start_cmd = local_cmds.get("start").unwrap();
        assert!(start_cmd.contains("traefik-dynamic.yml"));
    }

    // Test: Docker-compose detection for Redis
    #[test]
    fn test_detect_redis_via_docker_compose() {
        let dir = create_temp_dir();
        
        // Create docker-compose.yml with redis service
        fs::write(
            dir.path().join("docker-compose.yml"),
            r#"version: '3.8'
services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
  app:
    image: node:18
    depends_on:
      - redis
"#
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "redis");
    }

    // Test: Docker-compose detection for Traefik
    #[test]
    fn test_detect_traefik_via_docker_compose() {
        let dir = create_temp_dir();
        
        // Create docker-compose.yml with traefik service
        fs::write(
            dir.path().join("docker-compose.yml"),
            r#"version: '3.8'
services:
  traefik:
    image: traefik:v3.0
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock
  app:
    image: nginx
    labels:
      - "traefik.enable=true"
"#
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "traefik");
    }

    // Test: Tilde path contraction
    #[test]
    fn test_tilde_path_contraction() {
        let dir = create_temp_dir();
        
        // Create a simple redis app
        fs::write(dir.path().join("redis.conf"), "port 6379\n").unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        
        // Path should be contracted to use ~ if under home directory
        // Note: In tests, temp dirs might not be under home, so we check the logic
        if detected.path.starts_with('/') && std::env::var("HOME").is_ok() {
            let home = std::env::var("HOME").unwrap();
            if dir.path().starts_with(&home) {
                assert!(detected.path.starts_with("~/"), "Path should use tilde notation: {}", detected.path);
            }
        }
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
        assert!(local_cmds.contains_key("start"));
        
        // Should have a start command with config file (exact file depends on filesystem order)
        let start_cmd = local_cmds.get("start").unwrap();
        assert!(start_cmd.contains("redis") && start_cmd.contains(".conf"));
    }

    // Test: Traefik with different config formats
    #[test]
    fn test_traefik_different_formats() {
        // Test YAML format
        let dir_yaml = create_temp_dir();
        fs::write(
            dir_yaml.path().join("traefik.yaml"),
            "api:\n  dashboard: true\n"
        ).unwrap();
        
        let result = detect_app(dir_yaml.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().app_type, "traefik");
        
        // Test TOML format
        let dir_toml = create_temp_dir();
        fs::write(
            dir_toml.path().join("traefik.toml"),
            "[api]\ndashboard = true\n"
        ).unwrap();
        
        let result = detect_app(dir_toml.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().app_type, "traefik");
    }

    // Test: Explicit Redis conf file detection
    #[test]
    fn test_redis_conf_file_detection() {
        let dir = create_temp_dir();
        
        // Create redis.conf file
        fs::write(dir.path().join("redis.conf"), "port 6379\nbind 127.0.0.1\nsave 900 1\n").unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "redis");
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"), "Should have start command");
        
        // Verify the start command uses redis.conf (since config file exists)
        let start_cmd = local_cmds.get("start").unwrap();
        assert!(start_cmd.contains("redis.conf"), "Start command should reference redis.conf file");
        assert!(start_cmd.starts_with("redis-server"), "Should use redis-server command");
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
"#
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "traefik");
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"), "Should have start command");
        
        // Verify the start command uses traefik.toml
        let start_cmd = local_cmds.get("start").unwrap();
        assert!(start_cmd.contains("traefik.toml"), "Start command should reference traefik.toml file");
        assert!(start_cmd.contains("--configFile="), "Should use --configFile flag");
    }

    // Test: Redis local config file detection (redis.local.conf)
    #[test]
    fn test_redis_local_conf_detection() {
        let dir = create_temp_dir();
        
        // Create redis.local.conf file
        fs::write(
            dir.path().join("redis.local.conf"), 
            "port 6379\nbind 127.0.0.1\nsave 900 1\nlogfile redis.local.log\n"
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "redis");
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"), "Should have start command");
        
        // Verify the start command uses redis.local.conf (since config file exists)
        let start_cmd = local_cmds.get("start").unwrap();
        assert!(start_cmd.contains("redis.local.conf"), "Start command should reference redis.local.conf file");
        assert!(start_cmd.starts_with("redis-server"), "Should use redis-server command");
    }

    // Test: Traefik local TOML file detection (traefik.local.toml)
    #[test]
    fn test_traefik_local_toml_detection() {
        let dir = create_temp_dir();
        
        // Create traefik.local.toml file
        fs::write(
            dir.path().join("traefik.local.toml"),
            r#"[api]
dashboard = true
debug = true

[log]
level = "DEBUG"

[entryPoints]
  [entryPoints.web]
  address = ":80"
  
  [entryPoints.websecure]
  address = ":443"

[providers]
  [providers.file]
  filename = "traefik-dynamic.local.toml"
  watch = true
"#
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "traefik");
        assert!(detected.local_commands.is_some());
        
        let local_cmds = detected.local_commands.unwrap();
        assert!(local_cmds.contains_key("start"), "Should have start command");
        
        // Verify the start command uses traefik.local.toml
        let start_cmd = local_cmds.get("start").unwrap();
        assert!(start_cmd.contains("traefik.local.toml"), "Start command should reference traefik.local.toml file");
        assert!(start_cmd.contains("--configFile="), "Should use --configFile flag");
    }

    // Test: Multiple local config patterns
    #[test]
    fn test_multiple_local_config_patterns() {
        // Test redis.dev.conf
        let dir_redis_dev = create_temp_dir();
        fs::write(dir_redis_dev.path().join("redis.dev.conf"), "port 6380\n").unwrap();
        
        let result = detect_app(dir_redis_dev.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().app_type, "redis");
        
        // Test traefik.staging.yml
        let dir_traefik_staging = create_temp_dir();
        fs::write(
            dir_traefik_staging.path().join("traefik.staging.yml"),
            "api:\n  dashboard: true\n"
        ).unwrap();
        
        let result = detect_app(dir_traefik_staging.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().app_type, "traefik");
        
        // Test redis.production.config
        let dir_redis_prod = create_temp_dir();
        fs::write(dir_redis_prod.path().join("redis.production.config"), "port 6379\n").unwrap();
        
        let result = detect_app(dir_redis_prod.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().app_type, "redis");
    }
}



