//! OrbStack environment capability detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use std::collections::HashMap;

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
        // OrbStack is available if Docker is available
        // Check for Dockerfile or docker-compose
        ctx.file_exists("Dockerfile")
            || ctx.file_exists("docker-compose.yml")
            || ctx.file_exists("docker-compose.yaml")
            || !ctx.glob("**/Dockerfile*").is_empty()
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let mut commands = HashMap::new();
        let mut metadata = HashMap::new();

        // OrbStack uses standard Docker commands
        // Check for docker-compose files
        let has_compose = ctx.file_exists("docker-compose.yml")
            || ctx.file_exists("docker-compose.yaml");

        if has_compose {
            commands.insert(
                "up".to_string(),
                "docker compose up".to_string(),
            );
            commands.insert(
                "down".to_string(),
                "docker compose down".to_string(),
            );
            commands.insert(
                "build".to_string(),
                "docker compose build".to_string(),
            );
            commands.insert(
                "logs".to_string(),
                "docker compose logs -f".to_string(),
            );
            metadata.insert("has_compose".to_string(), serde_json::json!(true));
        }

        // Check for Dockerfile
        if ctx.file_exists("Dockerfile") || !ctx.glob("**/Dockerfile*").is_empty() {
            if !has_compose {
                // Add basic docker commands if no compose file
                commands.insert(
                    "build".to_string(),
                    "docker build -t app .".to_string(),
                );
                commands.insert(
                    "run".to_string(),
                    "docker run app".to_string(),
                );
            }
            metadata.insert("has_dockerfile".to_string(), serde_json::json!(true));
        }

        // Suggest default command
        let suggested_default = if has_compose {
            Some("up".to_string())
        } else if commands.contains_key("run") {
            Some("run".to_string())
        } else {
            None
        };

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::OrbStackEnv(OrbStackEnvInfo {
                commands,
                suggested_default,
                metadata,
            }),
            suggested_strategies: vec![],
        })
    }

    fn depends_on(&self) -> Vec<&str> {
        // Can run alongside docker-env
        vec![]
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
        let strategy = OrbStackEnvStrategy;

        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "orbstack-env");

        match result.data {
            DetectionData::OrbStackEnv(info) => {
                assert!(info.commands.contains_key("up"));
                assert!(info.commands.contains_key("down"));
                assert_eq!(info.suggested_default, Some("up".to_string()));
                assert_eq!(
                    info.metadata.get("has_compose"),
                    Some(&serde_json::json!(true))
                );
            }
            _ => panic!("Expected OrbStackEnv data"),
        }
    }

    #[test]
    fn test_orbstack_with_dockerfile_only() {
        let temp_dir = TempDir::new().unwrap();

        // Create Dockerfile
        let dockerfile = temp_dir.path().join("Dockerfile");
        let mut file = fs::File::create(&dockerfile).unwrap();
        file.write_all(b"FROM node:18\nWORKDIR /app\nCOPY . .\nCMD [\"node\", \"index.js\"]\n")
            .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = OrbStackEnvStrategy;

        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::OrbStackEnv(info) => {
                assert!(info.commands.contains_key("build"));
                assert!(info.commands.contains_key("run"));
                assert_eq!(
                    info.metadata.get("has_dockerfile"),
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

        // Should not apply without Docker files
        assert!(!strategy.can_apply(&ctx));
    }
}
