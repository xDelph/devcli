//! Redis service detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use std::collections::HashMap;

/// Detects Redis service via redis.conf or docker-compose
#[derive(Default)]
pub struct RedisStrategy;

impl DetectionStrategy for RedisStrategy {
    fn id(&self) -> &str {
        "redis"
    }

    fn name(&self) -> &str {
        "Redis"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::Service)
    }

    fn priority(&self) -> usize {
        150 // Service detection, after languages
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // Check for Redis config files
        if ctx.file_exists("redis.conf") {
            return true;
        }

        // Check for redis/ subdirectory with configs
        if ctx.file_exists("redis") {
            let redis_dir = ctx.root_path.join("redis");
            if redis_dir.join("redis.conf").exists() || redis_dir.join("redis.config").exists() {
                return true;
            }
        }

        // Check for any redis config files
        if let Ok(entries) = std::fs::read_dir(&ctx.root_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.starts_with("redis")
                    && (file_name_str.ends_with(".conf") || file_name_str.ends_with(".config"))
                {
                    return true;
                }
            }
        }

        // Check docker-compose files for redis services
        for compose_file in &["docker-compose.yml", "docker-compose.yaml"] {
            if ctx.file_exists(compose_file) {
                if let Ok(content) = ctx.read_file(compose_file) {
                    let content_lower = content.to_lowercase();
                    if content_lower.contains("redis:") || content_lower.contains("image: redis") {
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

        // Find all Redis config files
        if ctx.file_exists("redis.conf") {
            config_files.push(std::path::PathBuf::from("redis.conf"));
        }

        // Check redis/ subdirectory
        if ctx.file_exists("redis") {
            let redis_dir = ctx.root_path.join("redis");
            if redis_dir.join("redis.conf").exists() {
                config_files.push(std::path::PathBuf::from("redis/redis.conf"));
            }
            if redis_dir.join("redis.config").exists() {
                config_files.push(std::path::PathBuf::from("redis/redis.config"));
            }
        }

        // Check for pattern-matched config files
        if let Ok(entries) = std::fs::read_dir(&ctx.root_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.starts_with("redis")
                    && (file_name_str.ends_with(".conf") || file_name_str.ends_with(".config"))
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
                    if content_lower.contains("redis:") || content_lower.contains("image: redis") {
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
                name: "Redis".to_string(),
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
    fn test_redis_detection_with_config() {
        let temp_dir = TempDir::new().unwrap();

        // Create redis.conf
        let redis_conf = temp_dir.path().join("redis.conf");
        let mut file = fs::File::create(&redis_conf).unwrap();
        file.write_all(b"port 6379\nbind 127.0.0.1\n").unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = RedisStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "redis");
        assert_eq!(result.confidence, 1.0);

        // Check that it's Service data
        match result.data {
            DetectionData::Service(info) => {
                assert_eq!(info.name, "Redis");
                assert!(info.config_files.contains(&std::path::PathBuf::from("redis.conf")));
            }
            _ => panic!("Expected Service data"),
        }
    }

    #[test]
    fn test_redis_detection_docker_compose() {
        let temp_dir = TempDir::new().unwrap();

        // Create docker-compose.yml with Redis
        let compose = temp_dir.path().join("docker-compose.yml");
        let mut file = fs::File::create(&compose).unwrap();
        file.write_all(
            b"services:\n  redis:\n    image: redis:7-alpine\n    ports:\n      - 6379:6379\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = RedisStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "redis");

        // Check metadata
        match result.data {
            DetectionData::Service(info) => {
                assert!(info.metadata.contains_key("in_docker_compose"));
            }
            _ => panic!("Expected Service data"),
        }
    }

    #[test]
    fn test_redis_no_detection() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = RedisStrategy;

        // Should not apply without Redis markers
        assert!(!strategy.can_apply(&ctx));
    }
}
