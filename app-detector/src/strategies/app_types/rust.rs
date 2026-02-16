//! Rust language detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use serde::Deserialize;

/// Detects Rust projects via Cargo.toml
#[derive(Default)]
pub struct RustStrategy;

#[derive(Deserialize)]
struct CargoToml {
    package: PackageInfo,
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
        StrategyCategory::AppType(crate::types::AppTypeCategory::Language)
    }

    fn priority(&self) -> usize {
        100 // Languages have high priority
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.file_exists("Cargo.toml")
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let cargo_toml: CargoToml = ctx.parse_toml("Cargo.toml")?;

        // Try to detect Rust version
        let version = detect_rust_version();

        // Find all .rs files
        let rust_files = ctx.glob("**/*.rs");

        let mut metadata = std::collections::HashMap::new();
        metadata.insert(
            "package_name".to_string(),
            serde_json::json!(cargo_toml.package.name),
        );
        metadata.insert(
            "package_version".to_string(),
            serde_json::json!(cargo_toml.package.version),
        );
        if let Some(edition) = cargo_toml.package.edition {
            metadata.insert("edition".to_string(), serde_json::json!(edition));
        }

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
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
}

fn detect_rust_version() -> Option<String> {
    use std::process::Command;

    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|s| {
            // Output is like "rustc 1.75.0 (82e1608df 2023-12-21)"
            s.split_whitespace()
                .nth(1)
                .map(|v| v.to_string())
        })
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

        // Create Cargo.toml
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

        // Create some .rs files
        fs::create_dir(temp_dir.path().join("src")).unwrap();
        fs::File::create(temp_dir.path().join("src/main.rs")).unwrap();
        fs::File::create(temp_dir.path().join("src/lib.rs")).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = RustStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "rust");
        assert_eq!(result.confidence, 1.0);

        // Check that it's Language data
        match result.data {
            DetectionData::Language(info) => {
                assert_eq!(info.name, "Rust");
                assert!(info.primary_files.len() >= 2); // main.rs and lib.rs
            }
            _ => panic!("Expected Language data"),
        }
    }

    #[test]
    fn test_rust_no_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = RustStrategy;

        assert!(!strategy.can_apply(&ctx));
    }
}
