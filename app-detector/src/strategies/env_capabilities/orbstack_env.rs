//! OrbStack environment capability detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};

/// Detects OrbStack environment (Docker/Kubernetes on macOS)
#[derive(Default)]
pub struct OrbStackEnvStrategy;

impl DetectionStrategy for OrbStackEnvStrategy {
    fn id(&self) -> &str {
        "orbstack-env"
    }

    fn name(&self) -> &str {
        "OrbStack"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::EnvCapability(EnvCapabilityCategory::OrbStack)
    }

    fn priority(&self) -> usize {
        410 // Just after Docker (400)
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // OrbStack is available if Docker files exist.
        // Mirror Docker's scope awareness: at monorepo root, only check root-level files.
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
        // OrbStack is a superset of Docker - reuse Docker's detection
        let docker_result = ctx.get_result("docker")
            .ok_or_else(|| anyhow::anyhow!("Docker strategy must run first"))?;

        // Extract Docker's data
        let (docker_commands, docker_metadata, docker_suggested_default) = match &docker_result.data {
            DetectionData::DockerEnv(info) => {
                (info.commands.clone(), info.metadata.clone(), info.suggested_default.clone())
            }
            _ => return Err(anyhow::anyhow!("Expected DockerEnv data from docker strategy")),
        };

        // OrbStack uses Docker commands with `--context orbstack`.
        let docker_commands = docker_commands
            .into_iter()
            .map(|(k, v)| {
                let updated = if let Some(rest) = v.strip_prefix("docker ") {
                    format!("docker --context orbstack {rest}")
                } else {
                    v
                };
                (k, updated)
            })
            .collect();

        let mut metadata = docker_metadata;
        metadata.insert("orbstack_compatible".to_string(), serde_json::json!(true));

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::OrbStackEnv(OrbStackEnvInfo {
                commands: docker_commands,
                suggested_default: docker_suggested_default,
                metadata,
            }),
            suggested_strategies: vec![],
        })
    }

    fn depends_on(&self) -> Vec<&str> {
        // OrbStack depends on Docker detection
        vec!["docker"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_orbstack_with_compose() {
        let temp_dir = TempDir::new().unwrap();

        // Create docker-compose.yml
        let compose = temp_dir.path().join("docker-compose.yml");
        let mut file = fs::File::create(&compose).unwrap();
        file.write_all(
            b"services:\n  app:\n    build: .\n    ports:\n      - 3000:3000\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        // First run Docker strategy
        let docker_strategy = crate::strategies::DockerStrategy;
        let docker_result = docker_strategy.detect(&ctx).unwrap();
        ctx.store_result(docker_result);

        // Then run OrbStack strategy
        let strategy = OrbStackEnvStrategy;
        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "orbstack-env");

        match result.data {
            DetectionData::OrbStackEnv(info) => {
                // Should have same commands as Docker
                assert!(info.commands.contains_key("up"));
                assert!(info.commands.contains_key("down"));
                assert!(info.commands.contains_key("build"));
                assert!(info.commands.contains_key("logs"));
                assert_eq!(info.suggested_default, Some("up".to_string()));
                assert_eq!(
                    info.metadata.get("has_compose"),
                    Some(&serde_json::json!(true))
                );
                assert_eq!(
                    info.metadata.get("orbstack_compatible"),
                    Some(&serde_json::json!(true))
                );
            }
            _ => panic!("Expected OrbStackEnv data"),
        }
    }

    #[test]
    fn test_orbstack_with_multistage_dockerfile() {
        let temp_dir = TempDir::new().unwrap();

        // Create multi-stage Dockerfile
        let dockerfile = temp_dir.path().join("Dockerfile");
        let mut file = fs::File::create(&dockerfile).unwrap();
        file.write_all(
            b"FROM node:18 AS builder\nWORKDIR /app\nCOPY . .\nRUN npm install\n\nFROM nginx:alpine AS production\nEXPOSE 80\nCOPY --from=builder /app/dist /usr/share/nginx/html\n"
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        // First run Docker strategy
        let docker_strategy = crate::strategies::DockerStrategy;
        let docker_result = docker_strategy.detect(&ctx).unwrap();
        ctx.store_result(docker_result);

        // Then run OrbStack strategy
        let strategy = OrbStackEnvStrategy;
        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::OrbStackEnv(info) => {
                // Should have stage-specific build commands from Docker
                assert!(info.commands.contains_key("build"));
                assert!(info.commands.contains_key("builder"));
                assert!(info.commands.contains_key("production"));
                assert!(info.commands.contains_key("run"));
                assert_eq!(
                    info.metadata.get("has_stages"),
                    Some(&serde_json::json!(true))
                );
            }
            _ => panic!("Expected OrbStackEnv data"),
        }
    }

    #[test]
    fn test_orbstack_no_detection() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = OrbStackEnvStrategy;

        // Should not apply without Docker detection
        assert!(!strategy.can_apply(&ctx));
    }
}
