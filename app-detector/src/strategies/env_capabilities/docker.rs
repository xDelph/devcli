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

/// Compose file names Docker Compose looks for (`compose.yaml` is the
/// canonical one since Compose v2).
const COMPOSE_FILES: &[&str] = &[
    "compose.yaml",
    "compose.yml",
    "docker-compose.yaml",
    "docker-compose.yml",
];

/// Whether `ctx` has Docker files this strategy (and OrbStack) builds on.
///
/// At an Nx monorepo root only root-level files count: each workspace is
/// detected on its own by hierarchical detection.
pub(crate) fn has_docker_files(ctx: &DetectionContext) -> bool {
    let root_files =
        ctx.file_exists("Dockerfile") || COMPOSE_FILES.iter().any(|name| ctx.file_exists(name));
    if ctx.get_result("nx").is_some() && !ctx.is_workspace() {
        return root_files;
    }
    root_files || !nested_dockerfiles(ctx).is_empty()
}

/// Dockerfiles anywhere under the root, sorted.
///
/// `Dockerfile.dockerignore` (BuildKit's per-Dockerfile ignore file) matches
/// `Dockerfile*` but is not a Dockerfile.
fn nested_dockerfiles(ctx: &DetectionContext) -> Vec<std::path::PathBuf> {
    let mut found: Vec<_> = ctx
        .glob("**/Dockerfile*")
        .into_iter()
        .filter(|path| {
            !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(".dockerignore"))
        })
        .collect();
    found.sort();
    found
}

/// Image / container name derived from the folder name.
///
/// Docker image names must be lowercase `[a-z0-9._-]` and start with a letter
/// or digit: a folder called `MyApp` or `my app` produced `-t MyApp`, which
/// docker rejects ("repository name must be lowercase"), and an unquoted space
/// split the command.
fn app_name(ctx: &DetectionContext) -> String {
    let raw = ctx
        .root_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("app");
    docker_name(raw)
}

fn docker_name(raw: &str) -> String {
    let mut name: String = raw
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    name = name
        .trim_start_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_string();
    if name.is_empty() {
        "app".to_string()
    } else {
        name
    }
}

/// Quote a path for a shell-split command when it needs it.
fn shell_arg(arg: &str) -> String {
    if arg
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
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
        has_docker_files(ctx)
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

        // Find in subdirectories (skip at monorepo root). Sorted, so which
        // Dockerfile is "the" one (the first) no longer depends on directory
        // listing order.
        if !monorepo_root {
            for path in nested_dockerfiles(ctx) {
                if !dockerfiles.contains(&path) {
                    dockerfiles.push(path);
                }
            }
        }

        // Find compose files
        for compose_name in COMPOSE_FILES {
            if ctx.file_exists(compose_name) {
                compose_files.push(std::path::PathBuf::from(compose_name));
            }
        }

        // Parse Dockerfiles. Build stages come from the primary Dockerfile only:
        // the generated `--target` commands build that file, and a stage that
        // only exists in another Dockerfile made them fail.
        for (index, dockerfile_path) in dockerfiles.iter().enumerate() {
            if let Ok(content) = ctx.read_file(dockerfile_path) {
                let parsed = parse_dockerfile(&content);
                if index == 0 {
                    stages.extend(parsed.stages);
                }
                base_images.extend(parsed.base_images);
                exposed_ports.extend(parsed.exposed_ports);
            }
        }

        let mut metadata = HashMap::new();
        let mut commands = HashMap::new();

        // Generate commands based on what's available
        let suggested_default = if !compose_files.is_empty() {
            // Docker Compose commands
            commands.insert("up".to_string(), "docker compose up".to_string());
            commands.insert("down".to_string(), "docker compose down".to_string());
            commands.insert("build".to_string(), "docker compose build".to_string());
            commands.insert("logs".to_string(), "docker compose logs -f".to_string());
            commands.insert("ps".to_string(), "docker compose ps".to_string());
            commands.insert("restart".to_string(), "docker compose restart".to_string());
            metadata.insert("has_compose".to_string(), serde_json::json!(true));
            Some("up".to_string())
        } else if !dockerfiles.is_empty() {
            // Plain Docker commands (devcli-style: stable keys + app-name tags)
            let dockerfile_path = dockerfiles.first().unwrap().to_string_lossy();
            let dockerfile_flag = if dockerfile_path == "Dockerfile" {
                String::new()
            } else {
                format!("-f {} ", shell_arg(&dockerfile_path))
            };

            let app = app_name(ctx);
            let app_type = detected_app_type(ctx);

            // Generic build command
            commands.insert(
                "build".to_string(),
                format!("docker build {dockerfile_flag}-t {app} ."),
            );

            // Stage-specific build commands, namespaced as `build:<stage>`
            // (the `devcli run <app> build:production` form). Keyed by the
            // bare stage name, a stage called `build`, `run` or `stop`
            // replaced the generic command: `build` then tagged `app:build`
            // and `run` started an `app` image nothing had built.
            if !stages.is_empty() {
                for stage in &stages {
                    commands.insert(
                        format!("build:{stage}"),
                        format!(
                            "docker build {dockerfile_flag}--target {stage} -t {app}:{stage} ."
                        ),
                    );
                    if stage.to_lowercase().contains("test") {
                        commands.insert(
                            format!("run:{stage}"),
                            format!("docker run --rm --pull never {app}:{stage}"),
                        );
                    }
                }
                metadata.insert("has_stages".to_string(), serde_json::json!(true));
                metadata.insert("stages".to_string(), serde_json::json!(stages));
            }

            // Run / stop commands. `run` starts the image `build` tags, and
            // never pulls: an image that was not built yet must fail, not
            // fetch docker.io/library/<folder> (an unrelated official image
            // for folders called redis, nginx, postgres...).
            let port = run_port_mapping(app_type);
            let run = match port {
                Some(p) => format!("docker run --name {app} --rm --pull never {p} {app}"),
                None => format!("docker run --name {app} --rm --pull never {app}"),
            };
            commands.insert("run".to_string(), run);
            commands.insert("stop".to_string(), format!("docker stop {app}"));

            metadata.insert("has_dockerfile".to_string(), serde_json::json!(true));
            Some("build".to_string())
        } else {
            None
        };

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
            if let Some(as_pos) = parts.iter().position(|&p| p.to_uppercase() == "AS")
                && as_pos + 1 < parts.len()
            {
                stages.push(parts[as_pos + 1].to_string());
            }
        }

        // Parse EXPOSE statements
        if line_upper.starts_with("EXPOSE ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in parts.iter().skip(1) {
                // Remove /tcp or /udp suffix
                let port_str = part.split('/').next().unwrap_or(part);
                if let Ok(port) = port_str.parse::<u16>()
                    && !exposed_ports.contains(&port)
                {
                    exposed_ports.push(port);
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
                assert!(info.commands.contains_key("build:builder"));
                assert!(info.commands.contains_key("build:production"));

                assert!(info
                    .commands
                    .get("build:builder")
                    .is_some_and(|cmd| cmd.contains("--target builder")
                        && cmd.contains(" -t ")
                        && cmd.ends_with(" .")));
                assert!(info
                    .commands
                    .get("build:production")
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

    fn docker_info(dir: &std::path::Path) -> DockerEnvInfo {
        let ctx = DetectionContext::new(dir).unwrap();
        match DockerStrategy.detect(&ctx).unwrap().data {
            DetectionData::DockerEnv(info) => info,
            _ => panic!("Expected DockerEnv data"),
        }
    }

    #[test]
    fn stages_named_like_generic_commands_do_not_replace_them() {
        let temp_dir = TempDir::new().unwrap();
        let project = temp_dir.path().join("web");
        fs::create_dir(&project).unwrap();
        fs::write(
            project.join("Dockerfile"),
            "FROM node:20 AS build\nRUN npm ci\n\nFROM node:20 AS run\n\nFROM node:20 AS stop\n\nFROM node:20 AS test\n",
        )
        .unwrap();

        let info = docker_info(&project);
        let cmd = |key: &str| info.commands.get(key).map(String::as_str);
        assert_eq!(info.suggested_default.as_deref(), Some("build"));
        assert_eq!(cmd("build"), Some("docker build -t web ."));
        assert_eq!(
            cmd("run"),
            Some("docker run --name web --rm --pull never web")
        );
        assert_eq!(cmd("stop"), Some("docker stop web"));
        assert_eq!(
            cmd("build:build"),
            Some("docker build --target build -t web:build .")
        );
        assert!(info.commands.contains_key("build:run"));
        assert!(info.commands.contains_key("build:stop"));
        assert_eq!(
            cmd("run:test"),
            Some("docker run --rm --pull never web:test")
        );
    }

    #[test]
    fn run_starts_the_image_build_tags_without_pulling() {
        let temp_dir = TempDir::new().unwrap();
        let project = temp_dir.path().join("redis");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("Dockerfile"), "FROM alpine\n").unwrap();

        let info = docker_info(&project);
        let cmd = |key: &str| info.commands.get(key).map(String::as_str);
        assert_eq!(cmd("build"), Some("docker build -t redis ."));
        assert_eq!(
            cmd("run"),
            Some("docker run --name redis --rm --pull never redis")
        );
    }

    #[test]
    fn dockerignore_files_and_dockerfile_directories_are_not_dockerfiles() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir(temp_dir.path().join("Dockerfiles")).unwrap();
        fs::write(temp_dir.path().join("Dockerfiles/README.md"), "# docs\n").unwrap();
        fs::write(
            temp_dir.path().join("Dockerfile.dockerignore"),
            "node_modules\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        assert!(!DockerStrategy.can_apply(&ctx));

        fs::write(
            temp_dir.path().join("Dockerfiles/Dockerfile.api"),
            "FROM alpine\n",
        )
        .unwrap();
        let info = docker_info(temp_dir.path());
        assert_eq!(
            info.dockerfiles,
            vec![std::path::PathBuf::from("Dockerfiles/Dockerfile.api")]
        );
    }

    #[test]
    fn docker_names_are_valid_image_names() {
        assert_eq!(docker_name("MyApp"), "myapp");
        assert_eq!(docker_name("my app"), "my-app");
        assert_eq!(docker_name("_internal"), "internal");
        assert_eq!(docker_name("@@@"), "app");
    }

    #[test]
    fn compose_yaml_is_recognised() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("compose.yaml"), "services: {}\n").unwrap();
        let ctx = DetectionContext::new(temp.path()).unwrap();
        assert!(DockerStrategy.can_apply(&ctx));
        let result = DockerStrategy.detect(&ctx).unwrap();
        let DetectionData::DockerEnv(info) = result.data else {
            panic!("expected docker env");
        };
        assert_eq!(
            info.compose_files,
            vec![std::path::PathBuf::from("compose.yaml")]
        );
        assert!(info.commands.contains_key("up"));
    }
}
