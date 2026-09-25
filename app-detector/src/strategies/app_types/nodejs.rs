//! Node.js language detection strategy

use crate::{context::DetectionContext, strategy::DetectionStrategy, types::*, Result};
use serde::Deserialize;
use std::collections::HashMap;

/// Detects Node.js projects via package.json
#[derive(Default)]
pub struct NodeJsStrategy;

#[derive(Deserialize)]
struct PackageJson {
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    scripts: HashMap<String, String>,
    #[serde(default)]
    dependencies: HashMap<String, String>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: HashMap<String, String>,
}

impl DetectionStrategy for NodeJsStrategy {
    fn id(&self) -> &str {
        "nodejs"
    }

    fn name(&self) -> &str {
        "Node.js"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(crate::types::AppTypeCategory::Language)
    }

    fn priority(&self) -> usize {
        100 // Languages have high priority
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.file_exists("package.json")
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let package_json: PackageJson = ctx.parse_json("package.json")?;

        // Detect Node.js version
        let version = detect_node_version();

        // Detect package manager (npm, yarn, pnpm, bun)
        let package_manager = crate::utils::package_manager::detect_node_package_manager(ctx);

        // Only track key configuration files, not all source files
        let key_files = vec!["package.json".into()];

        let mut metadata = HashMap::new();
        metadata.insert(
            "package_name".to_string(),
            serde_json::json!(package_json.name),
        );
        if let Some(ref version) = package_json.version {
            metadata.insert("package_version".to_string(), serde_json::json!(version));
        }
        metadata.insert(
            "package_manager".to_string(),
            serde_json::json!(package_manager),
        );
        metadata.insert(
            "script_count".to_string(),
            serde_json::json!(package_json.scripts.len()),
        );
        metadata.insert(
            "dependency_count".to_string(),
            serde_json::json!(package_json.dependencies.len()),
        );

        // Check for TypeScript
        let has_typescript = ctx.file_exists("tsconfig.json")
            || package_json.dev_dependencies.contains_key("typescript");
        metadata.insert("typescript".to_string(), serde_json::json!(has_typescript));

        // Suggest framework strategies based on dependencies
        let mut suggested = Vec::new();
        if package_json.dependencies.contains_key("react") {
            suggested.push("react".to_string());
        }
        if package_json.dependencies.contains_key("vue") {
            suggested.push("vue".to_string());
        }
        if package_json.dependencies.contains_key("@angular/core") {
            suggested.push("angular".to_string());
        }

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::Language(LanguageInfo {
                name: "Node.js".to_string(),
                version,
                version_source: Some("node --version".to_string()),
                primary_files: key_files,
                total_lines: None,
                metadata,
            }),
            suggested_strategies: suggested,
        })
    }
}

fn detect_node_version() -> Option<String> {
    use std::process::Command;

    Command::new("node")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().trim_start_matches('v').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_nodejs_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create package.json
        let package_json = temp_dir.path().join("package.json");
        let mut file = fs::File::create(&package_json).unwrap();
        file.write_all(
            br#"{
  "name": "test-app",
  "version": "1.0.0",
  "scripts": {
    "dev": "vite",
    "build": "vite build"
  },
  "dependencies": {
    "react": "^18.0.0"
  }
}"#,
        )
        .unwrap();

        // Create some JS files
        fs::create_dir(temp_dir.path().join("src")).unwrap();
        fs::File::create(temp_dir.path().join("src/index.js")).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = NodeJsStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "nodejs");
        assert_eq!(result.confidence, 1.0);

        // Should suggest React
        assert!(result.suggested_strategies.contains(&"react".to_string()));

        // Check that it's Language data
        match result.data {
            DetectionData::Language(info) => {
                assert_eq!(info.name, "Node.js");
                // Should only include key config files, not all source files
                assert_eq!(info.primary_files.len(), 1);
                assert!(info.primary_files[0]
                    .to_string_lossy()
                    .contains("package.json"));
            }
            _ => panic!("Expected Language data"),
        }
    }

    #[test]
    fn test_package_manager_detection() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("package.json"), r#"{"name":"app"}"#).unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        assert_eq!(
            crate::utils::package_manager::detect_node_package_manager(&ctx),
            "npm"
        );

        fs::File::create(temp_dir.path().join("yarn.lock")).unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        assert_eq!(
            crate::utils::package_manager::detect_node_package_manager(&ctx),
            "yarn"
        );
    }
}
