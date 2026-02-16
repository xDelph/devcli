//! Environment files detection strategy
//!
//! Detects .env files and parses their stage/context metadata based on naming conventions

use crate::{context::DetectionContext, strategy::DetectionStrategy, types::*, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Detects environment files (.env, .env.*, .*.env)
#[derive(Default)]
pub struct EnvFilesStrategy;

impl DetectionStrategy for EnvFilesStrategy {
    fn id(&self) -> &str {
        "env-files"
    }

    fn name(&self) -> &str {
        "Environment Files"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::Custom("env-files".to_string()))
    }

    fn priority(&self) -> usize {
        50 // Higher priority - env files are fundamental
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // Check if any files in the project are env files
        ctx.list_files().iter().any(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(is_env_file)
                .unwrap_or(false)
        })
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let mut env_files = Vec::new();

        // List all files and filter for env files
        for path in ctx.list_files() {
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if is_env_file(filename) {
                    let full_path = ctx.root_path.join(&path);
                    let (stage, context) = parse_env_filename(filename, &full_path, &ctx.root_path);
                    env_files.push(EnvFileInfo {
                        path: full_path,
                        stage,
                        context,
                    });
                }
            }
        }

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("file_count".to_string(), serde_json::json!(env_files.len()));

        // Count by stage
        let mut stages_count: HashMap<String, usize> = HashMap::new();
        for file in &env_files {
            if let Some(ref stage) = file.stage {
                *stages_count.entry(stage.clone()).or_insert(0) += 1;
            }
        }
        metadata.insert("stages".to_string(), serde_json::json!(stages_count));

        // Count by context
        let mut contexts_count: HashMap<String, usize> = HashMap::new();
        for file in &env_files {
            *contexts_count.entry(file.context.clone()).or_insert(0) += 1;
        }
        metadata.insert("contexts".to_string(), serde_json::json!(contexts_count));

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::Custom(serde_json::json!({
                "env_files": env_files.iter().map(|f| {
                    serde_json::json!({
                        "path": f.path.strip_prefix(&ctx.root_path).unwrap_or(&f.path).to_string_lossy(),
                        "stage": f.stage,
                        "context": f.context,
                    })
                }).collect::<Vec<_>>(),
                "metadata": metadata,
            })),
            suggested_strategies: vec![],
        })
    }
}

/// Information about a detected environment file
#[derive(Debug, Clone)]
struct EnvFileInfo {
    /// Full path to the file
    path: PathBuf,
    /// Detected stage (dev, qa, preprod, prod, staging, production, test, etc.)
    stage: Option<String>,
    /// Detected context (local, docker, orbstack, k8s, unspecified)
    context: String,
}

/// Check if a filename matches env file patterns
/// Patterns: .env, .env.*, .*.env
fn is_env_file(filename: &str) -> bool {
    filename == ".env"
        || filename.starts_with(".env.")
        || (filename.starts_with('.') && filename.ends_with(".env"))
}

/// Parse stage and context from env filename
///
/// # Naming Conventions:
/// - `.env` → stage: None, context: "unspecified"
/// - `.env.dev` → stage: "dev", context: "unspecified"
/// - `.env.local` → stage: None, context: "local"
/// - `.env.dev.local` → stage: "dev", context: "local"
/// - `docker/.env.qa` → stage: "qa", context: "docker"
/// - `.local.env` → stage: None, context: "local"
/// - `.dev.env` → stage: "dev", context: "unspecified"
///
/// # Arguments
/// * `filename` - Name of the env file
/// * `full_path` - Full path to the file
/// * `app_root` - App root directory
///
/// # Returns
/// Tuple of (stage, context) where context is "unspecified" if not explicitly set
fn parse_env_filename(
    filename: &str,
    full_path: &std::path::Path,
    app_root: &std::path::Path,
) -> (Option<String>, String) {
    // Determine context from directory
    let dir_context = if let Some(parent) = full_path.parent() {
        if parent == app_root {
            None
        } else {
            parent
                .file_name()
                .and_then(|n| n.to_str())
                .map(|dir_name| dir_name.to_lowercase())
        }
    } else {
        None
    };

    // Known contexts
    let known_contexts = ["local", "docker", "orbstack", "k8s"];

    // Parse filename
    if filename == ".env" {
        // Base .env file
        return (
            None,
            dir_context.unwrap_or_else(|| "unspecified".to_string()),
        );
    }

    // Handle .env.* pattern
    if let Some(suffix) = filename.strip_prefix(".env.") {
        let parts: Vec<&str> = suffix.split('.').collect();

        match parts.len() {
            1 => {
                // .env.X - could be stage or context
                let part = parts[0];
                if known_contexts.contains(&part) {
                    // It's a context
                    (None, part.to_string())
                } else {
                    // Assume it's a stage
                    (
                        Some(part.to_string()),
                        dir_context.unwrap_or_else(|| "unspecified".to_string()),
                    )
                }
            }
            2 => {
                // .env.X.Y - first is stage, second is context
                let stage = parts[0];
                let context = parts[1];
                (Some(stage.to_string()), context.to_string())
            }
            _ => {
                // More complex pattern - use first as stage, last as context
                let stage = parts[0];
                let context = parts[parts.len() - 1];
                (Some(stage.to_string()), context.to_string())
            }
        }
    }
    // Handle .*.env pattern
    else if filename.ends_with(".env") {
        if let Some(prefix) = filename
            .strip_prefix('.')
            .and_then(|s| s.strip_suffix(".env"))
        {
            // .X.env - X could be stage or context
            if known_contexts.contains(&prefix) {
                (None, prefix.to_string())
            } else {
                (
                    Some(prefix.to_string()),
                    dir_context.unwrap_or_else(|| "unspecified".to_string()),
                )
            }
        } else {
            (None, "unspecified".to_string())
        }
    } else {
        // Shouldn't reach here, but handle gracefully
        (None, "unspecified".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_env_file() {
        assert!(is_env_file(".env"));
        assert!(is_env_file(".env.dev"));
        assert!(is_env_file(".env.local"));
        assert!(is_env_file(".local.env"));
        assert!(is_env_file(".dev.env"));
        assert!(!is_env_file("env"));
        assert!(!is_env_file("config.json"));
        assert!(!is_env_file(".envrc"));
    }

    #[test]
    fn test_parse_env_filename_base() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".env");

        let (stage, context) = parse_env_filename(".env", &path, temp_dir.path());
        assert_eq!(stage, None);
        assert_eq!(context, "unspecified");
    }

    #[test]
    fn test_parse_env_filename_stage() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".env.dev");

        let (stage, context) = parse_env_filename(".env.dev", &path, temp_dir.path());
        assert_eq!(stage, Some("dev".to_string()));
        assert_eq!(context, "unspecified");
    }

    #[test]
    fn test_parse_env_filename_context() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".env.local");

        let (stage, context) = parse_env_filename(".env.local", &path, temp_dir.path());
        assert_eq!(stage, None);
        assert_eq!(context, "local");
    }

    #[test]
    fn test_parse_env_filename_stage_and_context() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".env.dev.local");

        let (stage, context) = parse_env_filename(".env.dev.local", &path, temp_dir.path());
        assert_eq!(stage, Some("dev".to_string()));
        assert_eq!(context, "local");
    }

    #[test]
    fn test_parse_env_filename_in_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let docker_dir = temp_dir.path().join("docker");
        fs::create_dir(&docker_dir).unwrap();
        let path = docker_dir.join(".env.qa");

        let (stage, context) = parse_env_filename(".env.qa", &path, temp_dir.path());
        assert_eq!(stage, Some("qa".to_string()));
        assert_eq!(context, "docker");
    }

    #[test]
    fn test_parse_env_filename_reverse_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".local.env");

        let (stage, context) = parse_env_filename(".local.env", &path, temp_dir.path());
        assert_eq!(stage, None);
        assert_eq!(context, "local");
    }

    #[test]
    fn test_env_files_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create various env files
        fs::write(temp_dir.path().join(".env"), "BASE=1").unwrap();
        fs::write(temp_dir.path().join(".env.dev"), "DEV=1").unwrap();
        fs::write(temp_dir.path().join(".env.qa"), "QA=1").unwrap();
        fs::write(temp_dir.path().join(".env.local"), "LOCAL=1").unwrap();
        fs::write(temp_dir.path().join(".env.dev.local"), "DEV_LOCAL=1").unwrap();

        // Create docker directory with env files
        let docker_dir = temp_dir.path().join("docker");
        fs::create_dir(&docker_dir).unwrap();
        fs::write(docker_dir.join(".env"), "DOCKER_BASE=1").unwrap();
        fs::write(docker_dir.join(".env.prod"), "DOCKER_PROD=1").unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = EnvFilesStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "env-files");
        assert_eq!(result.confidence, 1.0);

        // Check custom data
        match result.data {
            DetectionData::Custom(value) => {
                let env_files = value["env_files"].as_array().unwrap();
                assert_eq!(env_files.len(), 7);

                // Verify some specific files
                let base_file = env_files
                    .iter()
                    .find(|f| f["path"].as_str() == Some(".env"))
                    .unwrap();
                assert_eq!(base_file["stage"], serde_json::json!(null));
                assert_eq!(base_file["context"], "unspecified");

                let dev_file = env_files
                    .iter()
                    .find(|f| f["path"].as_str() == Some(".env.dev"))
                    .unwrap();
                assert_eq!(dev_file["stage"], "dev");
                assert_eq!(dev_file["context"], "unspecified");

                let docker_prod = env_files
                    .iter()
                    .find(|f| f["path"].as_str() == Some("docker/.env.prod"))
                    .unwrap();
                assert_eq!(docker_prod["stage"], "prod");
                assert_eq!(docker_prod["context"], "docker");

                // Check metadata
                let metadata = &value["metadata"];
                assert_eq!(metadata["file_count"], 7);
            }
            _ => panic!("Expected Custom data"),
        }
    }
}
