//! Nx monorepo detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use serde::Deserialize;
use std::collections::HashMap;

/// Detects Nx monorepo via nx.json
#[derive(Default)]
pub struct NxStrategy;

#[derive(Deserialize)]
struct NxJson {
    #[serde(default)]
    extends: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    affected: Option<serde_json::Value>,
    #[serde(default)]
    plugins: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct PackageJson {
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    workspaces: Option<serde_json::Value>,
}

impl DetectionStrategy for NxStrategy {
    fn id(&self) -> &str {
        "nx"
    }

    fn name(&self) -> &str {
        "Nx Monorepo"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::Monorepo)
    }

    fn priority(&self) -> usize {
        50 // Higher priority than language detection (runs first)
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.file_exists("nx.json")
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let nx_json: NxJson = ctx.parse_json("nx.json")?;

        let mut metadata = HashMap::new();

        // Parse package.json for monorepo name
        let _package_name = if ctx.file_exists("package.json") {
            let package_json: PackageJson = ctx.parse_json("package.json")?;
            metadata.insert("package_name".to_string(), serde_json::json!(package_json.name));
            if let Some(ref version) = package_json.version {
                metadata.insert("package_version".to_string(), serde_json::json!(version));
            }
            Some(package_json.name)
        } else {
            None
        };

        // Check for Nx configuration extension
        if let Some(ref extends) = nx_json.extends {
            metadata.insert("extends".to_string(), serde_json::json!(extends));
        }

        // Count plugins
        let plugin_count = nx_json.plugins.as_ref().map(|p| p.len()).unwrap_or(0);
        metadata.insert("plugin_count".to_string(), serde_json::json!(plugin_count));

        // Detect workspaces with structured info
        let workspace_info = detect_nx_workspace_info(ctx);
        let workspaces: Vec<String> = workspace_info.iter().map(|w| w.path.clone()).collect();
        metadata.insert("workspace_count".to_string(), serde_json::json!(workspace_info.len()));

        // Suggest Node.js strategy since Nx is built on Node
        let suggested_strategies = vec!["nodejs".to_string()];

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::Monorepo(MonorepoInfo {
                tool: "nx".to_string(),
                version: None, // Would need to parse nx version from package.json
                config_file: std::path::PathBuf::from("nx.json"),
                workspace_info,
                workspaces,
                metadata,
            }),
            suggested_strategies,
        })
    }

    fn depends_on(&self) -> Vec<&str> {
        vec![] // Runs first, no dependencies
    }

    fn conflicts_with(&self) -> Vec<&str> {
        // Nx monorepo should suppress root-level Node.js detection
        // Node.js apps exist within the monorepo workspaces
        vec!["nodejs"]
    }
}

fn detect_nx_workspace_info(ctx: &DetectionContext) -> Vec<WorkspaceInfo> {
    let mut infos = Vec::new();

    // Look for apps/ and libs/ directories (common Nx structure)
    for dir in &["apps", "libs", "packages"] {
        if ctx.file_exists(dir) {
            if let Ok(entries) = std::fs::read_dir(ctx.root_path.join(dir)) {
                let mut dir_entries: Vec<_> = entries.flatten()
                    .filter(|e| e.path().is_dir())
                    .collect();
                // Sort for deterministic order
                dir_entries.sort_by_key(|e| e.file_name());

                for entry in dir_entries {
                    if let Some(name) = entry.file_name().to_str() {
                        infos.push(WorkspaceInfo {
                            path: format!("{}/{}", dir, name),
                            name: Some(name.to_string()),
                            should_detect: true,
                        });
                    }
                }
            }
        }
    }

    infos
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_nx_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create nx.json
        let nx_json = temp_dir.path().join("nx.json");
        let mut file = fs::File::create(&nx_json).unwrap();
        file.write_all(
            br#"{
  "extends": "nx/presets/npm.json",
  "affected": {
    "defaultBase": "main"
  }
}"#,
        )
        .unwrap();

        // Create package.json
        let package_json = temp_dir.path().join("package.json");
        let mut file = fs::File::create(&package_json).unwrap();
        file.write_all(
            br#"{
  "name": "my-monorepo",
  "version": "1.0.0"
}"#,
        )
        .unwrap();

        // Create apps directory
        fs::create_dir(temp_dir.path().join("apps")).unwrap();
        fs::create_dir(temp_dir.path().join("apps/web")).unwrap();
        fs::create_dir(temp_dir.path().join("apps/api")).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = NxStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "nx");
        assert_eq!(result.confidence, 1.0);

        // Should suggest nodejs
        assert!(result.suggested_strategies.contains(&"nodejs".to_string()));

        // Check that it's Monorepo data
        match result.data {
            DetectionData::Monorepo(info) => {
                assert_eq!(info.tool, "nx");
                assert_eq!(info.config_file, std::path::PathBuf::from("nx.json"));
                assert!(info.workspace_info.len() >= 2, "Should detect workspace_info");
                assert_eq!(info.workspace_info.len(), info.workspaces.len(), "workspace_info and workspaces should match");
                assert!(info.workspace_info.iter().any(|w| w.path == "apps/web"));
                assert!(info.workspace_info.iter().any(|w| w.path == "apps/api"));
                assert!(info.metadata.contains_key("package_name"));
            }
            _ => panic!("Expected Monorepo data"),
        }
    }

    #[test]
    fn test_nx_no_detection() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = NxStrategy;

        // Should not apply without nx.json
        assert!(!strategy.can_apply(&ctx));
    }
}
