// Unit tests for environment detection
// Tests Docker, Kubernetes, and local command detection

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

    // Test: Docker detection with Dockerfile
    #[test]
    fn test_detect_docker_simple() {
        let dir = create_temp_dir();

        // Create package.json (nodejs app)
        create_package_json(dir.path(), "docker-app", vec![("start", "node server.js")]);

        // Create simple Dockerfile
        fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:18\nCOPY . .\nCMD [\"npm\", \"start\"]\n",
        )
        .unwrap();

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
        create_package_json(
            dir.path(),
            "multistage-app",
            vec![("start", "node server.js")],
        );

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
"#,
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();

        assert!(detected.docker_commands.is_some());
        let docker_cmds = detected.docker_commands.unwrap();

        // Should have stage-specific commands
        assert!(
            docker_cmds.contains_key("build"),
            "Should have 'build' stage command"
        );

        // Should still have general commands
        assert!(docker_cmds.contains_key("run"));
    }

    // Test: K8s detection with k8s directory
    #[test]
    fn test_detect_k8s_with_directory() {
        let dir = create_temp_dir();

        create_package_json(dir.path(), "k8s-app", vec![("start", "node server.js")]);

        // Create k8s directory with manifests
        fs::create_dir(dir.path().join("k8s")).unwrap();
        fs::write(
            dir.path().join("k8s/deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n",
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();

        assert!(detected.k8s_commands.is_some());
        let k8s_cmds = detected.k8s_commands.unwrap();
        assert!(k8s_cmds.contains_key("apply"));
        assert!(k8s_cmds.contains_key("delete"));
    }

    // Test: K8s detection with .k8s.yaml files
    #[test]
    fn test_detect_k8s_with_suffix() {
        let dir = create_temp_dir();

        create_package_json(dir.path(), "k8s-app2", vec![("start", "node server.js")]);

        // Create .k8s.yaml file
        fs::write(
            dir.path().join("deployment.k8s.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n",
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();

        assert!(detected.k8s_commands.is_some());
    }

    // Test: Docker port mapping for different app types
    #[test]
    fn test_docker_port_mappings() {
        // Test Node.js port (3000)
        let dir_node = create_temp_dir();
        create_package_json(
            dir_node.path(),
            "node-app",
            vec![("start", "node server.js")],
        );
        fs::write(dir_node.path().join("Dockerfile"), "FROM node:18\n").unwrap();

        let detected = detect_app(dir_node.path()).unwrap();
        let docker_cmds = detected.docker_commands.unwrap();
        let run_cmd = docker_cmds.get("run").unwrap();
        assert!(
            run_cmd.contains("3000:3000"),
            "Node.js should use port 3000"
        );

        // Test Python port (8000)
        let dir_python = create_temp_dir();
        fs::write(dir_python.path().join("requirements.txt"), "flask\n").unwrap();
        fs::write(dir_python.path().join("Dockerfile"), "FROM python:3.11\n").unwrap();

        let detected = detect_app(dir_python.path()).unwrap();
        let docker_cmds = detected.docker_commands.unwrap();
        let run_cmd = docker_cmds.get("run").unwrap();
        assert!(run_cmd.contains("8000:8000"), "Python should use port 8000");
    }

    // Test: Dockerfile in subdirectory
    #[test]
    fn test_dockerfile_in_subdirectory() {
        let dir = create_temp_dir();

        // Create package.json in root
        create_package_json(
            dir.path(),
            "subdir-docker",
            vec![("start", "node server.js")],
        );

        // Create Dockerfile in subdirectory (should still be found)
        fs::create_dir(dir.path().join("docker")).unwrap();
        fs::write(dir.path().join("docker/Dockerfile"), "FROM node:18\n").unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();

        assert!(
            detected.docker_commands.is_some(),
            "Should find Dockerfile in subdirectory"
        );
    }

    // Test: Multiple k8s files
    #[test]
    fn test_multiple_k8s_files() {
        let dir = create_temp_dir();

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
"#,
        )
        .unwrap();

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
  web:
    image: nginx
    labels:
      - "traefik.enable=true"
"#,
        )
        .unwrap();

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();
        assert_eq!(detected.app_type, "traefik");
    }

    // Test: Default command suggestions
    #[test]
    fn test_default_command_suggestions() {
        let dir = create_temp_dir();

        // Create Node.js app with common commands
        create_package_json(
            dir.path(),
            "defaults-test",
            vec![
                ("dev", "node dev.js"),
                ("start", "node server.js"),
                ("serve", "node serve.js"),
            ],
        );

        let result = detect_app(dir.path());
        assert!(result.is_ok());
        let detected = result.unwrap();

        assert!(detected.suggested_local_default.is_some());

        // Should prefer "serve" over "start" for Node.js
        let default = detected.suggested_local_default.unwrap();
        assert_eq!(default, "serve");
    }
}
