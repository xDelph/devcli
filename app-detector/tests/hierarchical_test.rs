//! Integration tests for hierarchical (monorepo) detection

use app_detector::{
    engine::{DetectionConfig, DetectionEngine},
    registry::StrategyRegistry,
    types::{AppTypeCategory, DetectionData, EnvCapabilityCategory},
};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Build the default engine with all strategies registered
fn default_engine() -> DetectionEngine {
    DetectionEngine::new(StrategyRegistry::with_defaults())
}

// ─── Fixture: nx-monorepo ─────────────────────────────────────────────────────

#[test]
fn test_nx_monorepo_creates_child_reports() {
    let engine = default_engine();
    let path = fixture_path("nx-monorepo");
    let report = engine.detect(&path).expect("Detection failed");

    // Root should detect Nx (not Node.js — suppressed by conflict resolution)
    let nx_results = report.by_app_type(&AppTypeCategory::Monorepo);
    assert!(!nx_results.is_empty(), "Should detect Nx at root");
    assert_eq!(nx_results[0].strategy_id, "nx");

    let nodejs_at_root = report.by_app_type(&AppTypeCategory::Language);
    assert!(
        nodejs_at_root.is_empty(),
        "Node.js should be suppressed at root by Nx"
    );

    // Root should have child reports for apps/web and apps/api
    assert!(
        report.is_workspace_root(),
        "Root report should be a workspace root"
    );
    assert_eq!(
        report.children.len(),
        2,
        "Should have 2 workspace children (apps/web, apps/api)"
    );
}

#[test]
fn test_nx_workspace_children_detect_nodejs() {
    let engine = default_engine();
    let path = fixture_path("nx-monorepo");
    let report = engine.detect(&path).expect("Detection failed");

    // Each workspace should detect Node.js independently
    for child in &report.children {
        let nodejs = child.by_app_type(&AppTypeCategory::Language);
        assert!(
            !nodejs.is_empty(),
            "Workspace {:?} should detect Node.js",
            child.path
        );
        assert_eq!(
            nodejs[0].strategy_id, "nodejs",
            "Workspace {:?} should have nodejs strategy",
            child.path
        );
    }
}

#[test]
fn test_nx_workspace_docker_scoped_to_workspace() {
    let engine = default_engine();
    let path = fixture_path("nx-monorepo");
    let report = engine.detect(&path).expect("Detection failed");

    // Docker at root should NOT be detected (no root-level Dockerfile)
    let docker_at_root = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert!(
        docker_at_root.is_empty(),
        "Docker should not be detected at monorepo root (no root Dockerfile)"
    );

    // Each workspace should detect its own Docker
    for child in &report.children {
        let docker = child.by_env_capability(&EnvCapabilityCategory::Docker);
        assert!(
            !docker.is_empty(),
            "Workspace {:?} should detect Docker",
            child.path
        );

        // Docker should only see Dockerfiles in its own workspace, not siblings
        match &docker[0].data {
            DetectionData::DockerEnv(info) => {
                assert_eq!(
                    info.dockerfiles.len(),
                    1,
                    "Workspace {:?} Docker should only see 1 Dockerfile (its own), not siblings",
                    child.path
                );
            }
            _ => panic!("Expected DockerEnv data"),
        }
    }
}

#[test]
fn test_workspace_web_has_multi_stage_docker_commands() {
    let engine = default_engine();
    let path = fixture_path("nx-monorepo");
    let report = engine.detect(&path).expect("Detection failed");

    // Find the web workspace
    let web_child = report
        .children
        .iter()
        .find(|c| c.path.ends_with("apps/web"))
        .expect("Should have apps/web child");

    let docker = web_child.by_env_capability(&EnvCapabilityCategory::Docker);
    assert!(!docker.is_empty());

    match &docker[0].data {
        DetectionData::DockerEnv(info) => {
            // web/Dockerfile has builder and production stages
            assert!(info.stages.contains(&"builder".to_string()), "Should detect 'builder' stage");
            assert!(info.stages.contains(&"production".to_string()), "Should detect 'production' stage");
            assert!(
                info.commands.contains_key("build-builder"),
                "Should have stage-specific build-builder command"
            );
            assert!(
                info.commands.contains_key("build-production"),
                "Should have stage-specific build-production command"
            );
        }
        _ => panic!("Expected DockerEnv data"),
    }
}

#[test]
fn test_all_reports_flat_traversal() {
    let engine = default_engine();
    let path = fixture_path("nx-monorepo");
    let report = engine.detect(&path).expect("Detection failed");

    let all = report.all_reports();
    // Should include root + 2 workspace children
    assert_eq!(all.len(), 3, "all_reports() should return root + 2 children");
}

// ─── Workspace detection config ───────────────────────────────────────────────

#[test]
fn test_disable_workspace_detection() {
    let registry = StrategyRegistry::with_defaults();
    let config = DetectionConfig {
        enable_workspace_detection: false,
        ..DetectionConfig::default()
    };
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("nx-monorepo");
    let report = engine
        .detect_with_config(&path, &config)
        .expect("Detection failed");

    assert!(
        report.children.is_empty(),
        "Workspace detection should produce no children when disabled"
    );
    assert!(
        !report.is_workspace_root(),
        "Should not be a workspace root when detection is disabled"
    );
}

#[test]
fn test_flat_project_has_no_children() {
    let engine = default_engine();
    let path = fixture_path("nodejs-docker");
    let report = engine.detect(&path).expect("Detection failed");

    assert!(
        report.children.is_empty(),
        "Flat Node.js project should have no workspace children"
    );
    assert!(
        !report.is_workspace_root(),
        "Flat project should not be a workspace root"
    );
}

// ─── Depth limit and cycle detection ─────────────────────────────────────────

#[test]
fn test_depth_limit_prevents_deep_recursion() {
    // Create a nested monorepo structure deeper than max_depth
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Level 0: root Nx monorepo
    create_nx_workspace(root, "apps/inner");

    // Level 1: inner is another Nx monorepo pointing deeper
    let inner_path = root.join("apps/inner");
    create_nx_workspace(&inner_path, "apps/deep");

    let config = DetectionConfig {
        max_workspace_depth: 1,
        ..DetectionConfig::default()
    };
    let engine = default_engine();

    let report = engine
        .detect_with_config(root, &config)
        .expect("Detection failed");

    // Root should have 1 child (apps/inner)
    assert_eq!(report.children.len(), 1, "Should detect inner workspace");

    // Inner should NOT have children (depth limit reached)
    let inner = &report.children[0];
    assert!(
        inner.children.is_empty(),
        "Inner workspace should not recurse further (depth limit = 1)"
    );
}

// ─── JSON serialization ───────────────────────────────────────────────────────

#[test]
fn test_hierarchical_report_json_serialization() {
    let engine = default_engine();
    let path = fixture_path("nx-monorepo");
    let report = engine.detect(&path).expect("Detection failed");

    let json = serde_json::to_string(&report).expect("Serialization failed");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Parse failed");

    assert!(
        parsed.get("children").is_some(),
        "JSON should include 'children' field for workspace root"
    );
    let children = parsed["children"].as_array().unwrap();
    assert_eq!(children.len(), 2, "JSON children should have 2 workspaces");
}

#[test]
fn test_flat_report_json_omits_children() {
    let engine = default_engine();
    let path = fixture_path("nodejs-docker");
    let report = engine.detect(&path).expect("Detection failed");

    let json = serde_json::to_string(&report).expect("Serialization failed");
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Parse failed");

    // children field should be omitted for flat projects (skip_serializing_if)
    assert!(
        parsed.get("children").is_none(),
        "JSON should NOT include 'children' for flat projects (backward compatible)"
    );
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Create a minimal Nx monorepo at `root` with one workspace at `workspace_path`
fn create_nx_workspace(root: &std::path::Path, workspace_path: &str) {
    // nx.json
    let mut f = fs::File::create(root.join("nx.json")).unwrap();
    f.write_all(b"{\"extends\": \"nx/presets/npm.json\"}").unwrap();

    // package.json
    let mut f = fs::File::create(root.join("package.json")).unwrap();
    f.write_all(b"{\"name\": \"test-monorepo\", \"version\": \"1.0.0\"}").unwrap();

    // Create workspace directory
    let ws_path = root.join(workspace_path);
    fs::create_dir_all(&ws_path).unwrap();

    // workspace package.json
    let ws_name = ws_path.file_name().unwrap().to_str().unwrap();
    let pkg_content = format!(
        r#"{{"name": "@test/{}", "version": "0.0.1", "scripts": {{"start": "node index.js"}}}}"#,
        ws_name
    );
    let mut f = fs::File::create(ws_path.join("package.json")).unwrap();
    f.write_all(pkg_content.as_bytes()).unwrap();
}
