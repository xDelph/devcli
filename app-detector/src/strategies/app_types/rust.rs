//! Rust language and Cargo workspace detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    utils::workspace::{cargo_workspace_members, expand_workspace_members},
    Result,
};
use serde::Deserialize;
use std::collections::HashMap;

/// Detects Rust projects via Cargo.toml (single crate or workspace root)
#[derive(Default)]
pub struct RustStrategy;

#[derive(Deserialize)]
struct CargoToml {
    package: Option<PackageInfo>,
}

#[derive(Deserialize)]
struct PackageInfo {
    name: String,
    version: String,
    #[serde(default)]
    edition: Option<String>,
}

impl DetectionStrategy for RustStrategy {
    fn id(&self) -> &str {
        "rust"
    }

    fn name(&self) -> &str {
        "Rust"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::Language)
    }

    fn priority(&self) -> usize {
        55 // Before generic language detection; workspace roots surface early
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.file_exists("Cargo.toml")
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let content = ctx.read_file("Cargo.toml")?;

        if let Some(members) = cargo_workspace_members(&content) {
            if !members.is_empty() {
                return detect_workspace(ctx, &content, members);
            }
        }

        detect_crate(ctx, &content)
    }
}

fn detect_workspace(
    ctx: &DetectionContext,
    content: &str,
    member_patterns: Vec<String>,
) -> Result<DetectionResult> {
    let workspace_info = expand_workspace_members(&ctx.root_path, &member_patterns);
    let workspaces: Vec<String> = workspace_info.iter().map(|w| w.path.clone()).collect();

    let mut metadata = HashMap::new();
    if let Ok(cargo) = toml::from_str::<CargoToml>(content) {
        if let Some(package) = cargo.package {
            metadata.insert("package_name".to_string(), serde_json::json!(package.name));
            metadata.insert("package_version".to_string(), serde_json::json!(package.version));
        }
    }
    metadata.insert(
        "workspace_count".to_string(),
        serde_json::json!(workspace_info.len()),
    );

    Ok(DetectionResult {
        strategy_id: "rust".to_string(),
        category: StrategyCategory::AppType(AppTypeCategory::Monorepo),
        confidence: 1.0,
        data: DetectionData::Monorepo(MonorepoInfo {
            tool: "cargo".to_string(),
            version: detect_rust_version(),
            config_file: std::path::PathBuf::from("Cargo.toml"),
            workspace_info,
            workspaces,
            metadata,
        }),
        suggested_strategies: vec![],
    })
}

fn detect_crate(ctx: &DetectionContext, content: &str) -> Result<DetectionResult> {
    let cargo_toml: CargoToml = toml::from_str(content)
        .map_err(|e| anyhow::anyhow!("Failed to parse Cargo.toml: {e}"))?;
    let package = cargo_toml
        .package
        .ok_or_else(|| anyhow::anyhow!("Cargo.toml missing [package] section"))?;

    let version = detect_rust_version();
    let rust_files = ctx.glob("**/*.rs");

    let mut metadata = HashMap::new();
    metadata.insert("package_name".to_string(), serde_json::json!(package.name));
    metadata.insert("package_version".to_string(), serde_json::json!(package.version));
    if let Some(edition) = package.edition {
        metadata.insert("edition".to_string(), serde_json::json!(edition));
    }
    metadata.insert(
        "has_main".to_string(),
        serde_json::json!(ctx.file_exists("src/main.rs") || content.contains("[[bin]]")),
    );
    metadata.insert(
        "has_lib".to_string(),
        serde_json::json!(ctx.file_exists("src/lib.rs") || content.contains("[lib]")),
    );

    Ok(DetectionResult {
        strategy_id: "rust".to_string(),
        category: StrategyCategory::AppType(AppTypeCategory::Language),
        confidence: 1.0,
        data: DetectionData::Language(LanguageInfo {
            name: "Rust".to_string(),
            version,
            version_source: Some("rustc --version".to_string()),
            primary_files: rust_files,
            total_lines: None,
            metadata,
        }),
        suggested_strategies: vec![],
    })
}

fn detect_rust_version() -> Option<String> {
    use std::process::Command;

    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|s| s.split_whitespace().nth(1).map(|v| v.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_rust_detection() {
        let temp_dir = TempDir::new().unwrap();

        let cargo_toml = temp_dir.path().join("Cargo.toml");
        let mut file = fs::File::create(&cargo_toml).unwrap();
        file.write_all(
            br#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        fs::create_dir(temp_dir.path().join("src")).unwrap();
        fs::File::create(temp_dir.path().join("src/main.rs")).unwrap();
        fs::File::create(temp_dir.path().join("src/lib.rs")).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = RustStrategy;

        assert!(strategy.can_apply(&ctx));
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "rust");

        match result.data {
            DetectionData::Language(info) => {
                assert_eq!(info.name, "Rust");
                assert_eq!(info.metadata.get("package_name").unwrap(), "test-project");
            }
            _ => panic!("Expected Language data"),
        }
    }

    #[test]
    fn test_rust_workspace_detection() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("crates/api/src")).unwrap();
        fs::create_dir_all(temp_dir.path().join("crates/lib/src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[workspace]
members = ["crates/*"]
resolver = "2"
"#,
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("crates/api/Cargo.toml"),
            "[package]\nname = \"api\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("crates/lib/Cargo.toml"),
            "[package]\nname = \"lib\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let result = RustStrategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::Monorepo(info) => {
                assert_eq!(info.tool, "cargo");
                assert_eq!(info.workspace_info.len(), 2);
            }
            _ => panic!("Expected Monorepo data"),
        }
    }

    #[test]
    fn test_rust_no_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        assert!(!RustStrategy.can_apply(&ctx));
    }
}
