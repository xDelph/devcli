//! Nx monorepo task-runner environment capability detection.
//!
//! Discovers available targets from two sources — **no hardcoded target names**:
//!
//! - **Source A**: `nx.json` → `targetDefaults` keys
//!   These are targets explicitly configured at the workspace level.
//!
//! - **Source B**: each `apps/<name>/project.json`, `libs/<name>/project.json`,
//!   `packages/<name>/project.json` → `targets` keys
//!   These reflect every target that actually exists across all projects.
//!
//! The union of both sources yields the command set:
//! ```text
//! nx run-many --target=<target>
//! ```

use crate::{context::DetectionContext, strategy::DetectionStrategy, types::*, Result};
use std::collections::{BTreeSet, HashMap};
use std::process::Command;

/// Generates Nx task-runner commands from workspace config files.
#[derive(Default)]
pub struct NxEnvStrategy;

fn nx_show_project_targets(ctx: &DetectionContext) -> Option<Vec<String>> {
    let crate::context::ScopeType::Workspace {
        monorepo_root,
        workspace_name,
    } = &ctx.scope.scope_type
    else {
        return None;
    };

    for nx_cmd in ["npx", "nx"] {
        let mut cmd = Command::new(nx_cmd);
        if nx_cmd == "npx" {
            cmd.arg("nx");
        }

        let output = cmd
            .args(["show", "project", workspace_name, "--json"])
            .current_dir(monorepo_root)
            .output()
            .ok()?;

        if !output.status.success() {
            continue;
        }

        let json_str = String::from_utf8(output.stdout).ok()?;
        let project_config = serde_json::from_str::<serde_json::Value>(&json_str).ok()?;
        let targets = project_config.get("targets")?.as_object()?;
        let mut names: Vec<String> = targets.keys().cloned().collect();
        names.sort();
        return Some(names);
    }

    None
}

fn nx_run_prefix(ctx: &DetectionContext) -> &'static str {
    let crate::context::ScopeType::Workspace { .. } = &ctx.scope.scope_type else {
        return "nx";
    };
    "npx nx"
}

impl DetectionStrategy for NxEnvStrategy {
    fn id(&self) -> &str {
        "nx-env"
    }

    fn name(&self) -> &str {
        "Nx Task Runner"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::EnvCapability(EnvCapabilityCategory::Nx)
    }

    fn priority(&self) -> usize {
        460 // After Docker (400), OrbStack (410), K8s (450); before Local (500)
    }

    fn depends_on(&self) -> Vec<&str> {
        vec!["nx"] // Nx app-type must have been detected first
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.get_result("nx").is_some()
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let mut targets: BTreeSet<String> = BTreeSet::new();
        let mut metadata = HashMap::new();

        let show_targets = nx_show_project_targets(ctx).unwrap_or_default();
        if !show_targets.is_empty() {
            for t in &show_targets {
                targets.insert(t.clone());
            }
        }
        metadata.insert(
            "targets_from_show_project".to_string(),
            serde_json::json!(show_targets.len()),
        );

        // ── Source A: nx.json → targetDefaults ───────────────────────────────
        let from_defaults = collect_targets_from_nx_json(ctx, &mut targets);
        metadata.insert(
            "targets_from_defaults".to_string(),
            serde_json::json!(from_defaults),
        );

        // ── Source B: workspace project.json → targets ────────────────────────
        let from_projects = collect_targets_from_project_jsons(ctx, &mut targets);
        metadata.insert(
            "targets_from_projects".to_string(),
            serde_json::json!(from_projects),
        );

        // ── Build commands ────────────────────────────────────────────────────
        let sorted_targets: Vec<String> = targets.into_iter().collect(); // BTreeSet is already sorted
        let nx_cmd = nx_run_prefix(ctx);

        let mut commands: HashMap<String, String> = HashMap::new();

        if let crate::context::ScopeType::Workspace { workspace_name, .. } = &ctx.scope.scope_type {
            for t in &sorted_targets {
                commands.insert(t.clone(), format!("{nx_cmd} run {workspace_name}:{t}"));
            }
        } else {
            for t in &sorted_targets {
                commands.insert(t.clone(), format!("nx run-many --target={t}"));
            }
            // Bonus: if any targets found, add an `affected` meta-command keyed per target
            for target in &sorted_targets {
                commands.insert(
                    format!("affected-{target}"),
                    format!("nx affected --target={target}"),
                );
            }
        }

        // Suggested default: prefer serve > dev > build > test > first alphabetically
        let suggested_default = ["serve", "dev", "build", "test"]
            .iter()
            .find(|t| commands.contains_key(**t))
            .map(|t| t.to_string())
            .or_else(|| sorted_targets.first().cloned());

        metadata.insert(
            "target_count".to_string(),
            serde_json::json!(sorted_targets.len()),
        );

        let confidence = if sorted_targets.is_empty() { 0.5 } else { 1.0 };

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence,
            data: DetectionData::NxEnv(NxEnvInfo {
                targets: sorted_targets,
                commands,
                suggested_default,
                metadata,
            }),
            suggested_strategies: vec![],
        })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse `nx.json`, extract `targetDefaults` keys into `targets`.
/// Returns the number of new targets added.
fn collect_targets_from_nx_json(ctx: &DetectionContext, targets: &mut BTreeSet<String>) -> usize {
    let Ok(content) = ctx.read_file("nx.json") else {
        return 0;
    };
    let Ok(nx_json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return 0;
    };

    let Some(defaults) = nx_json.get("targetDefaults").and_then(|v| v.as_object()) else {
        return 0;
    };

    let mut added = 0;
    for key in defaults.keys() {
        if targets.insert(key.clone()) {
            added += 1;
        }
    }
    added
}

/// Scan `apps/`, `libs/`, and `packages/` for `project.json` files.
/// Collect all `targets` keys. Returns the number of new (deduplicated) targets added.
fn collect_targets_from_project_jsons(
    ctx: &DetectionContext,
    targets: &mut BTreeSet<String>,
) -> usize {
    let mut added = 0;

    for search_dir in &["apps", "libs", "packages"] {
        let dir_path = ctx.root_path.join(search_dir);
        if !dir_path.exists() {
            continue;
        }

        let Ok(workspace_entries) = std::fs::read_dir(&dir_path) else {
            continue;
        };

        for workspace in workspace_entries.flatten() {
            let project_json_path = workspace.path().join("project.json");
            if !project_json_path.is_file() {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(&project_json_path) else {
                continue;
            };
            let Ok(project) = serde_json::from_str::<serde_json::Value>(&content) else {
                continue;
            };

            let Some(project_targets) = project.get("targets").and_then(|v| v.as_object()) else {
                continue;
            };

            for key in project_targets.keys() {
                if targets.insert(key.clone()) {
                    added += 1;
                }
            }
        }
    }

    added
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_nx_context(dir: &TempDir) -> DetectionContext {
        // Write nx.json so the NxStrategy would fire, then manually store its result
        let ctx = DetectionContext::new(dir.path()).unwrap();
        // Simulate the nx app-type result being present
        ctx.store_result(crate::types::DetectionResult {
            strategy_id: "nx".to_string(),
            category: StrategyCategory::AppType(AppTypeCategory::Monorepo),
            confidence: 1.0,
            data: DetectionData::Monorepo(crate::types::MonorepoInfo {
                tool: "nx".to_string(),
                version: None,
                config_file: std::path::PathBuf::from("nx.json"),
                workspace_info: vec![],
                workspaces: vec![],
                metadata: HashMap::new(),
            }),
            suggested_strategies: vec![],
        });
        ctx
    }

    #[test]
    fn test_nx_env_from_target_defaults_only() {
        let dir = TempDir::new().unwrap();

        // nx.json with targetDefaults only
        let mut f = fs::File::create(dir.path().join("nx.json")).unwrap();
        f.write_all(br#"{"targetDefaults": {"build": {}, "test": {}, "lint": {}}}"#)
            .unwrap();

        let ctx = make_nx_context(&dir);
        let strategy = NxEnvStrategy;

        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "nx-env");
        assert_eq!(result.confidence, 1.0);

        match result.data {
            DetectionData::NxEnv(info) => {
                assert!(info.targets.contains(&"build".to_string()));
                assert!(info.targets.contains(&"test".to_string()));
                assert!(info.targets.contains(&"lint".to_string()));
                assert_eq!(info.commands["build"], "nx run-many --target=build");
                assert_eq!(info.commands["test"], "nx run-many --target=test");
                assert!(info.metadata["targets_from_defaults"].as_u64().unwrap() >= 3);
                assert_eq!(info.metadata["targets_from_projects"].as_u64().unwrap(), 0);
            }
            _ => panic!("Expected NxEnv data"),
        }
    }

    #[test]
    fn test_nx_env_from_project_json_only() {
        let dir = TempDir::new().unwrap();

        // nx.json without targetDefaults
        fs::write(
            dir.path().join("nx.json"),
            r#"{"extends": "nx/presets/npm.json"}"#,
        )
        .unwrap();

        // apps/web/project.json with targets
        fs::create_dir_all(dir.path().join("apps/web")).unwrap();
        let mut f = fs::File::create(dir.path().join("apps/web/project.json")).unwrap();
        f.write_all(br#"{"name":"web","targets":{"build":{},"serve":{},"test":{}}}"#)
            .unwrap();

        // apps/api/project.json with overlapping + unique targets
        fs::create_dir_all(dir.path().join("apps/api")).unwrap();
        let mut f = fs::File::create(dir.path().join("apps/api/project.json")).unwrap();
        f.write_all(br#"{"name":"api","targets":{"build":{},"serve":{},"e2e":{}}}"#)
            .unwrap();

        let ctx = make_nx_context(&dir);
        let strategy = NxEnvStrategy;
        let result = strategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::NxEnv(info) => {
                // build, serve, test, e2e — deduplicated
                assert!(info.targets.contains(&"build".to_string()));
                assert!(info.targets.contains(&"serve".to_string()));
                assert!(info.targets.contains(&"test".to_string()));
                assert!(info.targets.contains(&"e2e".to_string()));
                assert_eq!(info.targets.len(), 4, "Duplicates must be deduplicated");
                assert_eq!(info.metadata["targets_from_defaults"].as_u64().unwrap(), 0);
                assert!(info.metadata["targets_from_projects"].as_u64().unwrap() >= 3);
                // suggested default should prefer serve
                assert_eq!(info.suggested_default, Some("serve".to_string()));
            }
            _ => panic!("Expected NxEnv data"),
        }
    }

    #[test]
    fn test_nx_env_union_of_both_sources() {
        let dir = TempDir::new().unwrap();

        // nx.json with targetDefaults
        let mut f = fs::File::create(dir.path().join("nx.json")).unwrap();
        f.write_all(br#"{"targetDefaults": {"build": {}, "lint": {}}}"#)
            .unwrap();

        // project.json with overlapping + unique targets
        fs::create_dir_all(dir.path().join("apps/web")).unwrap();
        let mut f = fs::File::create(dir.path().join("apps/web/project.json")).unwrap();
        f.write_all(br#"{"targets":{"build":{},"serve":{},"test":{}}}"#)
            .unwrap();

        let ctx = make_nx_context(&dir);
        let result = NxEnvStrategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::NxEnv(info) => {
                // Union: build (both), lint (A only), serve (B only), test (B only)
                assert_eq!(info.targets.len(), 4);
                assert!(info.targets.contains(&"build".to_string()));
                assert!(info.targets.contains(&"lint".to_string()));
                assert!(info.targets.contains(&"serve".to_string()));
                assert!(info.targets.contains(&"test".to_string()));
                // Both affected-* and direct commands generated
                assert!(info.commands.contains_key("build"));
                assert!(info.commands.contains_key("affected-build"));
            }
            _ => panic!("Expected NxEnv data"),
        }
    }

    #[test]
    fn test_nx_env_no_targets_low_confidence() {
        let dir = TempDir::new().unwrap();

        // nx.json with no targetDefaults, no project.json files
        fs::write(
            dir.path().join("nx.json"),
            r#"{"extends": "nx/presets/npm.json"}"#,
        )
        .unwrap();

        let ctx = make_nx_context(&dir);
        let result = NxEnvStrategy.detect(&ctx).unwrap();

        // Should still produce a result but with lower confidence
        assert_eq!(result.confidence, 0.5);
        match result.data {
            DetectionData::NxEnv(info) => {
                assert!(info.targets.is_empty());
                assert!(info.commands.is_empty());
            }
            _ => panic!("Expected NxEnv data"),
        }
    }

    #[test]
    fn test_nx_env_not_applied_without_nx_result() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("nx.json"),
            r#"{"extends": "nx/presets/npm.json"}"#,
        )
        .unwrap();

        // Context without the nx result stored — simulates non-Nx project
        let ctx = DetectionContext::new(dir.path()).unwrap();
        assert!(!NxEnvStrategy.can_apply(&ctx));
    }

    #[test]
    fn test_nx_env_targets_are_sorted() {
        let dir = TempDir::new().unwrap();

        let mut f = fs::File::create(dir.path().join("nx.json")).unwrap();
        f.write_all(br#"{"targetDefaults": {"test": {}, "build": {}, "lint": {}, "serve": {}}}"#)
            .unwrap();

        let ctx = make_nx_context(&dir);
        let result = NxEnvStrategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::NxEnv(info) => {
                let sorted = {
                    let mut v = info.targets.clone();
                    v.sort();
                    v
                };
                assert_eq!(
                    info.targets, sorted,
                    "Targets must be alphabetically sorted"
                );
            }
            _ => panic!("Expected NxEnv data"),
        }
    }
}
