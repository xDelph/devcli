//! Docker container detection strategy

use crate::{context::DetectionContext, strategy::DetectionStrategy, types::*, Result};
use std::collections::HashMap;

/// Detects Docker via Dockerfile
#[derive(Default)]
pub struct DockerStrategy;

fn detected_app_type(ctx: &DetectionContext) -> Option<&'static str> {
    if ctx.get_result("nx").is_some() {
        return Some("nx");
    }
    if ctx.get_result("nodejs").is_some() {
        return Some("nodejs");
    }
    if ctx.get_result("python").is_some() {
        return Some("python");
    }
    if ctx.get_result("rust").is_some() {
        return Some("rust");
    }
    if ctx.get_result("redis").is_some() {
        return Some("redis");
    }
    if ctx.get_result("traefik").is_some() {
        return Some("traefik");
    }
    None
}

fn app_name(ctx: &DetectionContext) -> String {
    ctx.root_path
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("app")
        .to_string()
}

fn run_port_mapping(app_type: Option<&str>) -> Option<String> {
    match app_type {
        Some("nodejs") | Some("nx") => Some("-p 3000:3000".to_string()),
        Some("python") => Some("-p 8000:8000".to_string()),
        Some("redis") => Some("-p 6379:6379".to_string()),
        Some("traefik") => Some("-p 80:80 -p 443:443".to_string()),
        _ => None,
    }
}

impl DetectionStrategy for DockerStrategy {
    fn id(&self) -> &str {
        "docker"
    }

    fn name(&self) -> &str {
        "Docker"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::EnvCapability(crate::types::EnvCapabilityCategory::Docker)
    }

    fn priority(&self) -> usize {
        400 // Containers have lower priority than languages/frameworks
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // At monorepo root, only detect root-level files (not recursive into workspaces).
        // Each workspace will be detected independently via hierarchical detection.
        if ctx.get_result("nx").is_some() && !ctx.is_workspace() {
            return ctx.file_exists("Dockerfile")
                || ctx.file_exists("docker-compose.yml")
                || ctx.file_exists("docker-compose.yaml");
        }

        ctx.file_exists("Dockerfile")
            || ctx.file_exists("docker-compose.yml")
            || ctx.file_exists("docker-compose.yaml")
            || !ctx.glob("**/Dockerfile*").is_empty()
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let mut dockerfiles = Vec::new();
        let mut compose_files = Vec::new();
        let mut stages = Vec::new();
        let mut base_images = std::collections::HashSet::new();
        let mut exposed_ports = Vec::new();

        // At monorepo root, only scan root-level files.
        // Each workspace will be detected independently via hierarchical detection.
        let monorepo_root = ctx.get_result("nx").is_some() && !ctx.is_workspace();

        // Find Dockerfiles
        if ctx.file_exists("Dockerfile") {
            dockerfiles.push(std::path::PathBuf::from("Dockerfile"));
        }

        // Find in subdirectories (skip at monorepo root)
        if !monorepo_root {
            for path in ctx.glob("**/Dockerfile*") {
                if !dockerfiles.contains(&path) {
                    dockerfiles.push(path);
                }
            }
        }

        // Find compose files
        for compose_name in &["docker-compose.yml", "docker-compose.yaml", "compose.yml"] {
            if ctx.file_exists(compose_name) {
                compose_files.push(std::path::PathBuf::from(compose_name));
            }
        }

        // Parse Dockerfiles
        for dockerfile_path in &dockerfiles {
            if let Ok(content) = ctx.read_file(dockerfile_path) {
                let parsed = parse_dockerfile(&content);
                stages.extend(parsed.stages);
                base_images.extend(parsed.base_images);
                exposed_ports.extend(parsed.exposed_ports);
            }
        }

        let mut metadata = HashMap::new();
        let mut commands = HashMap::new();
        let suggested_default;

        // Generate commands based on what's available
        if !compose_files.is_empty() {
            // Docker Compose commands
            commands.insert("up".to_string(), "docker compose up".to_string());
            commands.insert("down".to_string(), "docker compose down".to_string());
            commands.insert("build".to_string(), "docker compose build".to_string());
            commands.insert("logs".to_string(), "docker compose logs -f".to_string());
            commands.insert("ps".to_string(), "docker compose ps".to_string());
            commands.insert("restart".to_string(), "docker compose restart".to_string());
            metadata.insert("has_compose".to_string(), serde_json::json!(true));
            suggested_default = Some("up".to_string());
        } else if !dockerfiles.is_empty() {
            // Plain Docker commands (devcli-style: stable keys + app-name tags)
            let dockerfile_path = dockerfiles.first().unwrap().to_string_lossy();
            let dockerfile_flag = if dockerfile_path == "Dockerfile" {
                String::new()
            } else {
                format!("-f {} ", dockerfile_path)
            };

            let app = app_name(ctx);
            let app_type = detected_app_type(ctx);

            // Generic build command
            commands.insert(
                "build".to_string(),
                format!("docker build {dockerfile_flag}-t {app} ."),
            );

            // Stage-specific build commands (use stage name as key)
            if !stages.is_empty() {
                for stage in &stages {
                    commands.insert(
                        stage.clone(),
                        format!(
                            "docker build {dockerfile_flag}--target {stage} -t {app}:{stage} ."
                        ),
                    );
                    if stage.to_lowercase().contains("test") {
                        commands.insert(
                            format!("{stage}-run"),
                            format!("docker run --rm {app}:{stage}"),
                        );
                    }
                }
                metadata.insert("has_stages".to_string(), serde_json::json!(true));
                metadata.insert("stages".to_string(), serde_json::json!(stages));
            }

            // Run / stop commands
            let port = run_port_mapping(app_type);
            let run = match port {
                Some(p) => format!("docker run --name {app} --rm {p} {app}"),
                None => format!("docker run --name {app} --rm {app}"),
            };
            commands.insert("run".to_string(), run);
            commands.insert("stop".to_string(), format!("docker stop {app}"));

            metadata.insert("has_dockerfile".to_string(), serde_json::json!(true));
            suggested_default = Some("build".to_string());
        } else {
            suggested_default = None;
        }

        metadata.insert(
            "dockerfile_count".to_string(),
            serde_json::json!(dockerfiles.len()),
        );
        metadata.insert(
            "compose_file_count".to_string(),
            serde_json::json!(compose_files.len()),
        );

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::DockerEnv(DockerEnvInfo {
                dockerfiles,
                compose_files,
                stages,
                base_images: base_images.into_iter().collect(),
                exposed_ports,
                commands,
                suggested_default,
                metadata,
            }),
            suggested_strategies: vec![],
        })
    }
}

struct DockerfileInfo {
    stages: Vec<String>,
    base_images: Vec<String>,
    exposed_ports: Vec<u16>,
}

fn parse_dockerfile(content: &str) -> DockerfileInfo {
    let mut stages = Vec::new();
    let mut base_images = Vec::new();
    let mut exposed_ports = Vec::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let line_upper = line.to_uppercase();

        // Parse FROM statements
        if line_upper.starts_with("FROM ") {
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Extract base image (second element)
            if parts.len() > 1 {
                base_images.push(parts[1].to_string());
            }

            // Check for stage name (AS keyword)
            if let Some(as_pos) = parts.iter().position(|&p| p.to_uppercase() == "AS") {
                if as_pos + 1 < parts.len() {
                    stages.push(parts[as_pos + 1].to_string());
                }
            }
        }

        // Parse EXPOSE statements
        if line_upper.starts_with("EXPOSE ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in parts.iter().skip(1) {
                // Remove /tcp or /udp suffix
                let port_str = part.split('/').next().unwrap_or(part);
                if let Ok(port) = port_str.parse::<u16>() {
                    if !exposed_ports.contains(&port) {
                        exposed_ports.push(port);
                    }
                }
            }
        }
    }

    DockerfileInfo {
        stages,
        base_images,
        exposed_ports,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_docker_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create Dockerfile
        let dockerfile = temp_dir.path().join("Dockerfile");
        let mut file = fs::File::create(&dockerfile).unwrap();
        file.write_all(
            b"FROM node:18 AS builder\nWORKDIR /app\nCOPY . .\nRUN npm install\n\nFROM nginx:alpine AS production\nEXPOSE 80\nCOPY --from=builder /app/dist /usr/share/nginx/html\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = DockerStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "docker");
        assert_eq!(result.confidence, 1.0);

        // Check that it's DockerEnv data
        match result.data {
            DetectionData::DockerEnv(info) => {
                assert_eq!(info.dockerfiles.len(), 1);
                assert_eq!(info.stages.len(), 2);
                assert!(info.stages.contains(&"builder".to_string()));
                assert!(info.stages.contains(&"production".to_string()));
                assert!(info.base_images.contains(&"node:18".to_string()));
                assert!(info.base_images.contains(&"nginx:alpine".to_string()));
                assert!(info.exposed_ports.contains(&80));
            }
            _ => panic!("Expected DockerEnv data"),
        }
    }

    #[test]
    fn test_dockerfile_parsing() {
        let content = r#"
FROM rust:1.75 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

FROM debian:bookworm-slim
EXPOSE 8080
EXPOSE 9090/tcp
COPY --from=builder /app/target/release/app /usr/local/bin/
        "#;

        let info = parse_dockerfile(content);

        assert_eq!(info.stages.len(), 1);
        assert_eq!(info.stages[0], "builder");

        assert_eq!(info.base_images.len(), 2);
        assert!(info.base_images.contains(&"rust:1.75".to_string()));
        assert!(info
            .base_images
            .contains(&"debian:bookworm-slim".to_string()));

        assert_eq!(info.exposed_ports.len(), 2);
        assert!(info.exposed_ports.contains(&8080));
        assert!(info.exposed_ports.contains(&9090));
    }

    #[test]
    fn test_dockerfile_in_subdirectory() {
        let temp_dir = TempDir::new().unwrap();

        // Create docker directory with Dockerfile
        fs::create_dir(temp_dir.path().join("docker")).unwrap();
        let dockerfile = temp_dir.path().join("docker/Dockerfile");
        let mut file = fs::File::create(&dockerfile).unwrap();
        file.write_all(b"FROM node:18\nEXPOSE 3000\n").unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = DockerStrategy;

        // Should detect Dockerfile in subdirectory
        assert!(
            strategy.can_apply(&ctx),
            "Should detect Dockerfile in docker/ subdirectory"
        );

        let result = strategy.detect(&ctx).unwrap();
        match result.data {
            DetectionData::DockerEnv(info) => {
                assert_eq!(info.dockerfiles.len(), 1);
                assert!(info.dockerfiles[0]
                    .to_string_lossy()
                    .contains("docker/Dockerfile"));
                assert!(info.exposed_ports.contains(&3000));
            }
            _ => panic!("Expected DockerEnv data"),
        }
    }

    #[test]
    fn test_multistage_dockerfile_commands() {
        let temp_dir = TempDir::new().unwrap();

        // Create multi-stage Dockerfile
        let dockerfile = temp_dir.path().join("Dockerfile");
        let mut file = fs::File::create(&dockerfile).unwrap();
        file.write_all(
            b"FROM node:18 AS builder\nWORKDIR /app\nCOPY . .\nRUN npm install\n\nFROM nginx:alpine AS production\nEXPOSE 80\nCOPY --from=builder /app/dist /usr/share/nginx/html\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = DockerStrategy;

        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "docker");

        match result.data {
            DetectionData::DockerEnv(info) => {
                // Should have generic build command
                assert!(info.commands.contains_key("build"));
                assert!(info
                    .commands
                    .get("build")
                    .is_some_and(|cmd| cmd.starts_with("docker build -t ") && cmd.ends_with(" .")));

                // Should have stage-specific build commands
                assert!(info.commands.contains_key("builder"));
                assert!(info.commands.contains_key("production"));

                assert!(info
                    .commands
                    .get("builder")
                    .is_some_and(|cmd| cmd.contains("--target builder")
                        && cmd.contains(" -t ")
                        && cmd.ends_with(" .")));
                assert!(info
                    .commands
                    .get("production")
                    .is_some_and(|cmd| cmd.contains("--target production")
                        && cmd.contains(" -t ")
                        && cmd.ends_with(" .")));

                // Should have stage metadata
                assert_eq!(
                    info.metadata.get("has_stages"),
                    Some(&serde_json::json!(true))
                );
                assert_eq!(info.stages.len(), 2);
                assert!(info.stages.contains(&"builder".to_string()));
                assert!(info.stages.contains(&"production".to_string()));

                // Should have base images
                assert!(info.base_images.contains(&"node:18".to_string()));
                assert!(info.base_images.contains(&"nginx:alpine".to_string()));

                // Should have exposed port
                assert!(info.exposed_ports.contains(&80));
            }
            _ => panic!("Expected DockerEnv data"),
        }
    }
}
