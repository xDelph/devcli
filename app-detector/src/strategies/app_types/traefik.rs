//! Traefik reverse proxy detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use std::collections::HashMap;

/// Detects Traefik reverse proxy via traefik.yml/yaml/toml
#[derive(Default)]
pub struct TraefikStrategy;

impl DetectionStrategy for TraefikStrategy {
    fn id(&self) -> &str {
        "traefik"
    }

    fn name(&self) -> &str {
        "Traefik"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::ReverseProxy)
    }

    fn priority(&self) -> usize {
        150 // Service detection, after languages
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // Check for standard Traefik config files
        for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
            if ctx.file_exists(file) {
                return true;
            }
        }

        // Check for traefik/ subdirectory with configs
        if ctx.file_exists("traefik") {
            let traefik_dir = ctx.root_path.join("traefik");
            for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
                if traefik_dir.join(file).exists() {
                    return true;
                }
            }
        }

        // Check for any traefik config files
        if let Ok(entries) = std::fs::read_dir(&ctx.root_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.starts_with("traefik")
                    && (file_name_str.ends_with(".yml")
                        || file_name_str.ends_with(".yaml")
                        || file_name_str.ends_with(".toml"))
                {
                    return true;
                }
            }
        }

        // Check docker-compose files for traefik services
        for compose_file in &["docker-compose.yml", "docker-compose.yaml"] {
            if ctx.file_exists(compose_file) {
                if let Ok(content) = ctx.read_file(compose_file) {
                    let content_lower = content.to_lowercase();
                    if content_lower.contains("traefik") || content_lower.contains("image: traefik") {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let mut config_files = Vec::new();
        let mut metadata = HashMap::new();

        // Find all Traefik config files
        for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
            if ctx.file_exists(file) {
                config_files.push(std::path::PathBuf::from(*file));
            }
        }

        // Check traefik/ subdirectory
        if ctx.file_exists("traefik") {
            let traefik_dir = ctx.root_path.join("traefik");
            for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
                if traefik_dir.join(file).exists() {
                    config_files.push(std::path::PathBuf::from(format!("traefik/{}", file)));
                }
            }
        }

        // Check for pattern-matched config files
        if let Ok(entries) = std::fs::read_dir(&ctx.root_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.starts_with("traefik")
                    && (file_name_str.ends_with(".yml")
                        || file_name_str.ends_with(".yaml")
                        || file_name_str.ends_with(".toml"))
                    && !config_files.iter().any(|p| p.to_string_lossy() == file_name_str)
                {
                    config_files.push(std::path::PathBuf::from(file_name_str.to_string()));
                }
            }
        }

        // Check if defined in docker-compose
        for compose_file in &["docker-compose.yml", "docker-compose.yaml"] {
            if ctx.file_exists(compose_file) {
                if let Ok(content) = ctx.read_file(compose_file) {
                    let content_lower = content.to_lowercase();
                    if content_lower.contains("traefik") || content_lower.contains("image: traefik") {
                        metadata.insert("in_docker_compose".to_string(), serde_json::json!(true));
                        break;
                    }
                }
            }
        }

        metadata.insert("config_files_count".to_string(), serde_json::json!(config_files.len()));

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::Service(ServiceInfo {
                name: "Traefik".to_string(),
                version: None, // Would need to parse from config or docker image
                config_files,
                metadata,
            }),
            suggested_strategies: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_traefik_detection_with_config() {
        let temp_dir = TempDir::new().unwrap();

        // Create traefik.yml
        let traefik_yml = temp_dir.path().join("traefik.yml");
        let mut file = fs::File::create(&traefik_yml).unwrap();
        file.write_all(
            b"api:\n  dashboard: true\nentryPoints:\n  web:\n    address: :80\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = TraefikStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "traefik");
        assert_eq!(result.confidence, 1.0);

        // Check that it's Service data
        match result.data {
            DetectionData::Service(info) => {
                assert_eq!(info.name, "Traefik");
                assert!(info.config_files.contains(&std::path::PathBuf::from("traefik.yml")));
            }
            _ => panic!("Expected Service data"),
        }
    }

    #[test]
    fn test_traefik_detection_docker_compose() {
        let temp_dir = TempDir::new().unwrap();

        // Create docker-compose.yml with Traefik
        let compose = temp_dir.path().join("docker-compose.yml");
        let mut file = fs::File::create(&compose).unwrap();
        file.write_all(
            b"services:\n  traefik:\n    image: traefik:v2.10\n    ports:\n      - 80:80\n      - 8080:8080\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = TraefikStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "traefik");

        // Check metadata
        match result.data {
            DetectionData::Service(info) => {
                assert!(info.metadata.contains_key("in_docker_compose"));
            }
            _ => panic!("Expected Service data"),
        }
    }

    #[test]
    fn test_traefik_no_detection() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = TraefikStrategy;

        // Should not apply without Traefik markers
        assert!(!strategy.can_apply(&ctx));
    }
}
