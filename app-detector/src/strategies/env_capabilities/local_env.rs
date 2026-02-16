//! Local environment capability detection strategy
//! Extracts local commands based on detected app type

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use std::collections::HashMap;

/// Detects local environment commands based on app type
#[derive(Default)]
pub struct LocalEnvStrategy;

impl DetectionStrategy for LocalEnvStrategy {
    fn id(&self) -> &str {
        "local-env"
    }

    fn name(&self) -> &str {
        "Local Environment"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::EnvCapability(EnvCapabilityCategory::Local)
    }

    fn priority(&self) -> usize {
        500 // Environment capabilities run after app types
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // Check if we have any supported app type detected
        // This strategy depends on having an app type first
        ctx.get_result("nodejs").is_some()
            || ctx.get_result("nx").is_some()
            || ctx.get_result("python").is_some()
            || ctx.get_result("redis").is_some()
            || ctx.get_result("traefik").is_some()
            || ctx.get_result("rust").is_some()
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let mut commands = HashMap::new();
        let mut metadata = HashMap::new();
        let mut detected_app_type = None;

        // Detect commands based on app type results
        if ctx.get_result("nodejs").is_some() {
            detected_app_type = Some("nodejs");
            extract_nodejs_commands(ctx, &mut commands);
        }

        if ctx.get_result("nx").is_some() {
            detected_app_type = Some("nx");
            extract_nx_commands(ctx, &mut commands);
        }

        if ctx.get_result("python").is_some() {
            detected_app_type = Some("python");
            extract_python_commands(ctx, &mut commands);
        }

        if ctx.get_result("redis").is_some() {
            detected_app_type = Some("redis");
            extract_redis_commands(ctx, &mut commands);
        }

        if ctx.get_result("traefik").is_some() {
            detected_app_type = Some("traefik");
            extract_traefik_commands(ctx, &mut commands);
        }

        if ctx.get_result("rust").is_some() {
            detected_app_type = Some("rust");
            extract_rust_commands(ctx, &mut commands);
        }

        if let Some(app_type) = detected_app_type {
            metadata.insert("app_type".to_string(), serde_json::json!(app_type));
        }

        // Suggest a default command
        let suggested_default = suggest_default_command(detected_app_type, &commands);

        metadata.insert("command_count".to_string(), serde_json::json!(commands.len()));

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::LocalEnv(LocalEnvInfo {
                commands,
                suggested_default,
                metadata,
            }),
            suggested_strategies: vec![],
        })
    }

    fn depends_on(&self) -> Vec<&str> {
        // Depends on app type strategies
        vec!["nodejs", "nx", "python", "redis", "traefik", "rust"]
    }
}

fn extract_nodejs_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    if let Ok(content) = ctx.read_file("package.json") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
                for (key, _value) in scripts {
                    commands.insert(key.clone(), format!("npm run {}", key));
                }
            }
        }
    }
}

fn extract_nx_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    // Nx uses npm scripts like nodejs
    extract_nodejs_commands(ctx, commands);
}

fn extract_python_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    // Standard Python commands
    commands.insert("start".to_string(), "python main.py".to_string());

    // Check for tests
    if ctx.file_exists("tests") || ctx.file_exists("test") {
        commands.insert("test".to_string(), "pytest".to_string());
    }

    // Check for requirements.txt
    if ctx.file_exists("requirements.txt") {
        commands.insert(
            "install".to_string(),
            "pip install -r requirements.txt".to_string(),
        );
    }

    // Check for poetry
    if ctx.file_exists("pyproject.toml") && ctx.file_exists("poetry.lock") {
        commands.insert("install".to_string(), "poetry install".to_string());
        commands.insert("start".to_string(), "poetry run python main.py".to_string());
    }
}

fn extract_redis_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    let mut config_found = false;

    // Check for config files in current directory
    if ctx.file_exists("redis.conf") {
        commands.insert("start".to_string(), "redis-server redis.conf".to_string());
        config_found = true;
    }

    // Check for config files in redis/ subdirectory
    if !config_found && ctx.file_exists("redis") {
        if ctx.file_exists("redis/redis.conf") {
            commands.insert(
                "start".to_string(),
                "redis-server redis/redis.conf".to_string(),
            );
            config_found = true;
        } else if ctx.file_exists("redis/redis.config") {
            commands.insert(
                "start".to_string(),
                "redis-server redis/redis.config".to_string(),
            );
            config_found = true;
        }
    }

    // Check for any redis config files in current directory
    if !config_found {
        if let Ok(entries) = std::fs::read_dir(&ctx.root_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.starts_with("redis")
                    && (file_name_str.ends_with(".conf") || file_name_str.ends_with(".config"))
                {
                    commands.insert(
                        "start".to_string(),
                        format!("redis-server {}", file_name_str),
                    );
                    config_found = true;
                    break;
                }
            }
        }
    }

    // Fallback to generic redis-server if no config found
    if !config_found {
        commands.insert("start".to_string(), "redis-server".to_string());
    }
}

fn extract_traefik_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    let mut config_found = false;

    // Check for config files in current directory
    for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
        if ctx.file_exists(file) {
            commands.insert(
                "start".to_string(),
                format!("traefik --configFile={}", file),
            );
            config_found = true;
            break;
        }
    }

    // Check for config files in traefik/ subdirectory
    if !config_found && ctx.file_exists("traefik") {
        for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
            if ctx.file_exists(format!("traefik/{}", file)) {
                commands.insert(
                    "start".to_string(),
                    format!("traefik --configFile=traefik/{}", file),
                );
                config_found = true;
                break;
            }
        }
    }

    // Check for any traefik config files in current directory
    if !config_found {
        if let Ok(entries) = std::fs::read_dir(&ctx.root_path) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.starts_with("traefik")
                    && (file_name_str.ends_with(".yml")
                        || file_name_str.ends_with(".yaml")
                        || file_name_str.ends_with(".toml"))
                {
                    commands.insert(
                        "start".to_string(),
                        format!("traefik --configFile={}", file_name_str),
                    );
                    config_found = true;
                    break;
                }
            }
        }
    }

    // Fallback if no config file found
    if !config_found {
        commands.insert("start".to_string(), "traefik".to_string());
    }
}

fn extract_rust_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    // Standard Rust commands via Cargo
    commands.insert("build".to_string(), "cargo build".to_string());
    commands.insert("run".to_string(), "cargo run".to_string());
    commands.insert("test".to_string(), "cargo test".to_string());
    commands.insert("check".to_string(), "cargo check".to_string());

    // Check if it's a library or binary
    if let Ok(content) = ctx.read_file("Cargo.toml") {
        if content.contains("[[bin]]") || content.contains("[bin]") {
            commands.insert("start".to_string(), "cargo run --release".to_string());
        }
    }
}

fn suggest_default_command(
    app_type: Option<&str>,
    commands: &HashMap<String, String>,
) -> Option<String> {
    if commands.is_empty() {
        return None;
    }

    match app_type {
        Some("nodejs") | Some("nx") => {
            // For Node/Nx apps, prefer dev server commands
            if commands.contains_key("serve") {
                return Some("serve".to_string());
            }
            if commands.contains_key("start") {
                return Some("start".to_string());
            }
            if commands.contains_key("dev") {
                return Some("dev".to_string());
            }
        }
        Some("python") => {
            if commands.contains_key("start") {
                return Some("start".to_string());
            }
        }
        Some("rust") => {
            if commands.contains_key("run") {
                return Some("run".to_string());
            }
        }
        Some("redis") | Some("traefik") => {
            if commands.contains_key("start") {
                return Some("start".to_string());
            }
        }
        _ => {}
    }

    // Fallback: just use the first available command
    commands.keys().next().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_local_env_nodejs() {
        let temp_dir = TempDir::new().unwrap();

        // Create package.json with scripts
        let package_json = temp_dir.path().join("package.json");
        let mut file = fs::File::create(&package_json).unwrap();
        file.write_all(
            br#"{
  "name": "test-app",
  "scripts": {
    "start": "node index.js",
    "dev": "nodemon index.js",
    "test": "jest"
  }
}"#,
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        // First detect nodejs
        let nodejs_strategy = crate::strategies::NodeJsStrategy;
        let nodejs_result = nodejs_strategy.detect(&ctx).unwrap();
        ctx.store_result(nodejs_result);

        // Then detect local env
        let strategy = LocalEnvStrategy;
        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "local-env");

        match result.data {
            DetectionData::LocalEnv(info) => {
                assert_eq!(info.commands.len(), 3);
                assert_eq!(info.commands.get("start"), Some(&"npm run start".to_string()));
                assert_eq!(info.commands.get("dev"), Some(&"npm run dev".to_string()));
                assert_eq!(info.commands.get("test"), Some(&"npm run test".to_string()));
                // Should suggest "start" as default
                assert_eq!(info.suggested_default, Some("start".to_string()));
            }
            _ => panic!("Expected LocalEnv data"),
        }
    }

    #[test]
    fn test_local_env_python() {
        let temp_dir = TempDir::new().unwrap();

        // Create Python files
        fs::File::create(temp_dir.path().join("main.py")).unwrap();
        fs::File::create(temp_dir.path().join("requirements.txt")).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        // First detect python
        let python_strategy = crate::strategies::PythonStrategy;
        let python_result = python_strategy.detect(&ctx).unwrap();
        ctx.store_result(python_result);

        // Then detect local env
        let strategy = LocalEnvStrategy;
        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::LocalEnv(info) => {
                assert!(info.commands.contains_key("start"));
                assert!(info.commands.contains_key("install"));
                assert_eq!(info.commands.get("start"), Some(&"python main.py".to_string()));
                assert_eq!(
                    info.commands.get("install"),
                    Some(&"pip install -r requirements.txt".to_string())
                );
            }
            _ => panic!("Expected LocalEnv data"),
        }
    }

    #[test]
    fn test_local_env_no_app_type() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = LocalEnvStrategy;

        // Should not apply without any app type detected
        assert!(!strategy.can_apply(&ctx));
    }
}
