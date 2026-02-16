//! Docker container detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use std::collections::HashMap;

/// Detects Docker via Dockerfile
#[derive(Default)]
pub struct DockerStrategy;

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

        // Find Dockerfiles
        if ctx.file_exists("Dockerfile") {
            dockerfiles.push(std::path::PathBuf::from("Dockerfile"));
        }

        // Find in subdirectories
        for path in ctx.glob("**/Dockerfile*") {
            if !dockerfiles.contains(&path) {
                dockerfiles.push(path);
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
            // Plain Docker commands
            let dockerfile_path = dockerfiles.first().unwrap().to_string_lossy();
            if dockerfile_path == "Dockerfile" {
                commands.insert("build".to_string(), "docker build -t app .".to_string());
            } else {
                commands.insert("build".to_string(), format!("docker build -f {} -t app .", dockerfile_path));
            }
            commands.insert("run".to_string(), "docker run app".to_string());
            commands.insert("run-it".to_string(), "docker run -it app".to_string());
            if !exposed_ports.is_empty() {
                let port = exposed_ports[0];
                commands.insert("run-port".to_string(), format!("docker run -p {}:{} app", port, port));
            }
            metadata.insert("has_dockerfile".to_string(), serde_json::json!(true));
            suggested_default = Some("build".to_string());
        } else {
            suggested_default = None;
        }

        metadata.insert("dockerfile_count".to_string(), serde_json::json!(dockerfiles.len()));
        metadata.insert("compose_file_count".to_string(), serde_json::json!(compose_files.len()));

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
        assert!(info.base_images.contains(&"debian:bookworm-slim".to_string()));

        assert_eq!(info.exposed_ports.len(), 2);
        assert!(info.exposed_ports.contains(&8080));
        assert!(info.exposed_ports.contains(&9090));
    }
}
