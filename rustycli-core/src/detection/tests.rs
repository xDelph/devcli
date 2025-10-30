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
}

