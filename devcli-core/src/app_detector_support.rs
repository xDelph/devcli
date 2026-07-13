//! Adapter between devcli-core and the `app-detector` crate.
//!
//! All detection orchestration goes through app-detector. devcli-specific command
//! selection is applied here when converting `DetectionReport` → `DetectedApp`.

use crate::detection::{build_env_files_map, detect_env_files, DetectedApp};
use crate::utils::path::contract_tilde;
use crate::Result;
use app_detector::{
    DetectionConfig, DetectionData, DetectionEngine, DetectionReport, StrategyRegistry,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn engine() -> &'static DetectionEngine {
    static ENGINE: OnceLock<DetectionEngine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let registry = StrategyRegistry::with_defaults();
        DetectionEngine::new(registry)
    })
}

/// Run app-detector on `path` (includes hierarchical workspace recursion).
pub fn detect(path: &Path) -> Result<DetectionReport> {
    engine()
        .detect(path)
        .map_err(|err| anyhow::anyhow!("{err}"))
}

/// Run app-detector without workspace recursion (single directory scope).
pub fn detect_single(path: &Path) -> Result<DetectionReport> {
    let config = DetectionConfig {
        enable_workspace_detection: false,
        ..DetectionConfig::default()
    };
    engine()
        .detect_with_config(path, &config)
        .map_err(|err| anyhow::anyhow!("{err}"))
}

/// Phase 1: detect app type via app-detector.
pub fn detect_app_type(path: &Path) -> Result<String> {
    let report = detect_single(path)?;
    resolve_app_type(&report, path)
}

/// Extract app name via app-detector metadata + fallbacks.
pub fn extract_app_name(path: &Path, app_type: &str) -> Result<String> {
    let report = detect_single(path)?;
    resolve_app_name(&report, path, app_type)
}

/// Full devcli detection for a single app directory.
pub fn detect_app(path: &Path) -> Result<DetectedApp> {
    let report = detect_single(path)?;
    build_detected_app(path, &report)
}

/// Convert an app-detector report into devcli's `DetectedApp`.
pub fn build_detected_app(path: &Path, report: &DetectionReport) -> Result<DetectedApp> {
    let app_type = resolve_app_type(report, path)?;
    let app_name = resolve_app_name(report, path, &app_type)?;

    let mut local_commands = extract_local_commands(report);
    if app_type == "nx" {
        if let Some(nx_env) = report.get("nx-env") {
            if let DetectionData::NxEnv(info) = &nx_env.data {
                if !info.commands.is_empty() {
                    local_commands = Some(info.commands.clone());
                }
            }
        }
    }

    let docker_commands = extract_docker_commands(report);
    let orbstack_commands = extract_orbstack_commands(report);
    let k8s_commands = extract_k8s_commands(report);

    let suggested_local_default = suggest_local_default(&app_type, &local_commands);
    let suggested_docker_default = suggest_docker_default(&docker_commands);
    let suggested_orbstack_default = suggest_orbstack_default(&orbstack_commands);

    let dockerfile_path = resolve_dockerfile_path(path, report);

    let env_files = detect_env_files(path, dockerfile_path.as_deref())
        .ok()
        .filter(|files: &Vec<_>| !files.is_empty())
        .map(|files| build_env_files_map(&files));

    Ok(DetectedApp {
        app_type,
        app_name,
        path: contract_tilde(path),
        local_commands,
        docker_commands,
        orbstack_commands,
        k8s_commands,
        suggested_local_default,
        suggested_docker_default,
        suggested_orbstack_default,
        dockerfile_path,
        env_files,
    })
}

/// Resolve devcli app type from an app-detector report (devcli priority order).
pub fn resolve_app_type(report: &DetectionReport, path: &Path) -> Result<String> {
    if report.has("nx") || path.join("nx.json").exists() {
        return Ok("nx".to_string());
    }
    if report.has("nodejs") {
        return Ok("nodejs".to_string());
    }
    if report.has("redis") {
        return Ok("redis".to_string());
    }
    if report.has("traefik") {
        return Ok("traefik".to_string());
    }
    if report.has("rust") || path.join("Cargo.toml").exists() {
        return Ok("rust".to_string());
    }
    if report.has("python") {
        return Ok("python".to_string());
    }

    anyhow::bail!("No supported app type detected in {}", path.display())
}

/// Extract app name from report metadata or config files.
pub fn resolve_app_name(report: &DetectionReport, path: &Path, app_type: &str) -> Result<String> {
    if let Some(name) = report.app_name() {
        if (app_type == "nodejs" || app_type == "nx") && name.contains('/') {
            if let Some(short) = name.split('/').next_back() {
                return Ok(short.to_string());
            }
        }
        return Ok(name);
    }

    if app_type == "nodejs" || app_type == "nx" {
        if let Ok(content) = fs::read_to_string(path.join("package.json")) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                    if name.contains('/') {
                        if let Some(short) = name.split('/').next_back() {
                            return Ok(short.to_string());
                        }
                    }
                    return Ok(name.to_string());
                }
            }
        }
    }

    if app_type == "python" {
        if let Ok(content) = fs::read_to_string(path.join("pyproject.toml")) {
            for line in content.lines() {
                if line.starts_with("name") && line.contains('=') {
                    if let Some(name) = line.split('=').nth(1) {
                        let name = name.trim().trim_matches('"').trim_matches('\'');
                        return Ok(name.to_string());
                    }
                }
            }
        }
    }

    path.file_name()
        .and_then(|n| n.to_str())
        .filter(|name| !name.is_empty() && *name != ".")
        .map(|s| s.to_string())
        .or_else(|| directory_basename(path))
        .ok_or_else(|| anyhow::anyhow!("Could not extract app name from path"))
}

/// Best-effort directory name for app naming (`devcli-private` for `.` or relative paths).
pub fn directory_basename(path: &Path) -> Option<String> {
    path.canonicalize()
        .ok()
        .and_then(|canonical| canonical.file_name().map(|n| n.to_string_lossy().into_owned()))
        .or_else(|| {
            std::env::current_dir().ok().and_then(|cwd| {
                cwd.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
        })
}

/// Workspace member directory names when `path` is a Cargo workspace root.
pub fn cargo_workspace_member_dirs(path: &Path) -> Option<std::collections::HashSet<String>> {
    let content = fs::read_to_string(path.join("Cargo.toml")).ok()?;
    let members = app_detector::utils::workspace::cargo_workspace_members(&content)?;
    if members.is_empty() {
        return None;
    }

    let expanded =
        app_detector::utils::workspace::expand_workspace_members(path, &members);
    Some(
        expanded
            .into_iter()
            .map(|member| {
                member
                    .path
                    .split('/')
                    .next_back()
                    .unwrap_or(&member.path)
                    .to_string()
            })
            .collect(),
    )
}

/// Local commands from the `local-env` strategy result.
pub fn extract_local_commands(report: &DetectionReport) -> Option<HashMap<String, String>> {
    let result = report.get("local-env")?;
    match &result.data {
        DetectionData::LocalEnv(info) if !info.commands.is_empty() => Some(info.commands.clone()),
        _ => None,
    }
}

/// K8s commands from the `kubernetes-env` strategy result (devcli-normalized).
pub fn extract_k8s_commands(report: &DetectionReport) -> Option<HashMap<String, String>> {
    let result = report.get("kubernetes-env")?;
    match &result.data {
        DetectionData::KubernetesEnv(info) if !info.commands.is_empty() => Some(info.commands.clone()),
        _ => None,
    }
}

/// Docker commands from the `docker` strategy result.
pub fn extract_docker_commands(report: &DetectionReport) -> Option<HashMap<String, String>> {
    let result = report.get("docker")?;
    match &result.data {
        DetectionData::DockerEnv(info) if !info.commands.is_empty() => Some(info.commands.clone()),
        _ => None,
    }
}

/// OrbStack commands from the `orbstack-env` strategy result.
pub fn extract_orbstack_commands(report: &DetectionReport) -> Option<HashMap<String, String>> {
    let result = report.get("orbstack-env")?;
    match &result.data {
        DetectionData::OrbStackEnv(info) if !info.commands.is_empty() => Some(info.commands.clone()),
        _ => None,
    }
}

/// Workspace child paths from a monorepo root report.
pub fn workspace_child_paths(report: &DetectionReport) -> Vec<PathBuf> {
    report
        .children
        .iter()
        .map(|child| child.path.clone())
        .collect()
}

/// Relative dockerfile path when app-detector found Docker support.
pub fn resolve_dockerfile_path(path: &Path, report: &DetectionReport) -> Option<String> {
    if let Some(result) = report.get("docker") {
        if let DetectionData::DockerEnv(info) = &result.data {
            if let Some(df) = info.dockerfiles.first() {
                return Some(df.to_string_lossy().replace('\\', "/"));
            }
        }
    }

    find_dockerfile(path)
        .ok()
        .flatten()
        .and_then(|dockerfile| {
            dockerfile
                .strip_prefix(path)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
}

pub fn suggest_local_default(
    app_type: &str,
    commands: &Option<HashMap<String, String>>,
) -> Option<String> {
    let cmds = commands.as_ref()?;
    match app_type {
        "nodejs" | "nx" => {
            if cmds.contains_key("serve") {
                return Some("serve".to_string());
            }
            if cmds.contains_key("start") {
                return Some("start".to_string());
            }
            if cmds.contains_key("dev") {
                return Some("dev".to_string());
            }
        }
        "python" | "redis" | "traefik" => {
            if cmds.contains_key("start") {
                return Some("start".to_string());
            }
        }
        _ => {}
    }
    cmds.keys().next().cloned()
}

pub fn suggest_docker_default(commands: &Option<HashMap<String, String>>) -> Option<String> {
    let cmds = commands.as_ref()?;
    if cmds.contains_key("run") {
        Some("run".to_string())
    } else {
        cmds.keys().next().cloned()
    }
}

pub fn suggest_orbstack_default(commands: &Option<HashMap<String, String>>) -> Option<String> {
    let cmds = commands.as_ref()?;
    if cmds.contains_key("run") {
        Some("run".to_string())
    } else {
        cmds.keys().next().cloned()
    }
}

/// Fallback Dockerfile lookup (used when a strategy didn't report dockerfiles).
fn find_dockerfile(path: &Path) -> Result<Option<PathBuf>> {
    if path.join("Dockerfile").exists() {
        return Ok(Some(path.join("Dockerfile")));
    }

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let subdir_dockerfile = entry.path().join("Dockerfile");
                if subdir_dockerfile.exists() {
                    return Ok(Some(subdir_dockerfile));
                }

                if let Ok(subentries) = fs::read_dir(entry.path()) {
                    for subentry in subentries.flatten() {
                        if subentry.path().is_dir() {
                            let nested = subentry.path().join("Dockerfile");
                            if nested.exists() {
                                return Ok(Some(nested));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

// Note: docker-compose heuristics live in app-detector strategies (redis/traefik).

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_package_json(dir: &Path, name: &str, scripts: &[(&str, &str)]) {
        let mut pkg = serde_json::json!({
            "name": name,
            "version": "1.0.0",
            "scripts": {}
        });
        for (key, value) in scripts {
            pkg["scripts"][key] = serde_json::json!(value);
        }
        fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&pkg).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn adapter_detects_nodejs_with_docker_and_k8s() {
        let dir = TempDir::new().unwrap();
        write_package_json(
            dir.path(),
            "full-app",
            &[("start", "node server.js"), ("test", "jest")],
        );
        fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:18\nCMD [\"npm\", \"start\"]\n",
        )
        .unwrap();
        fs::create_dir(dir.path().join("k8s")).unwrap();
        fs::write(
            dir.path().join("k8s/deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n",
        )
        .unwrap();

        let detected = detect_app(dir.path()).unwrap();
        assert_eq!(detected.app_type, "nodejs");
        assert!(detected.local_commands.is_some());
        assert!(detected.docker_commands.is_some());
        assert!(detected.k8s_commands.is_some());
        let docker = detected.docker_commands.unwrap();
        assert!(docker.contains_key("build"));
        assert!(docker.contains_key("run"));
        assert!(docker.contains_key("stop"));
    }

    #[test]
    fn adapter_detects_redis_via_compose() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("docker-compose.yml"),
            "services:\n  redis:\n    image: redis:7-alpine\n",
        )
        .unwrap();

        let detected = detect_app(dir.path()).unwrap();
        assert_eq!(detected.app_type, "redis");
    }

    #[test]
    fn adapter_detects_rust_project() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"my-rust-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").unwrap();

        let detected = detect_app(dir.path()).unwrap();
        assert_eq!(detected.app_type, "rust");
        assert_eq!(detected.app_name, "my-rust-app");
        let local = detected.local_commands.unwrap();
        assert!(local.contains_key("run"));
        assert!(local.contains_key("test"));
    }

    #[test]
    fn adapter_prefers_serve_as_local_default() {
        let dir = TempDir::new().unwrap();
        write_package_json(
            dir.path(),
            "defaults-test",
            &[
                ("dev", "node dev.js"),
                ("start", "node server.js"),
                ("serve", "node serve.js"),
            ],
        );

        let detected = detect_app(dir.path()).unwrap();
        assert_eq!(detected.suggested_local_default, Some("serve".to_string()));
    }

    #[test]
    fn adapter_discovers_nx_workspace_children_including_libs() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("nx.json"), r#"{"version": 2}"#).unwrap();

        for (parent, name) in [("apps", "web"), ("libs", "shared")] {
            let workspace = dir.path().join(parent).join(name);
            fs::create_dir_all(&workspace).unwrap();
            write_package_json(&workspace, name, &[("build", "nx build")]);
        }

        let report = detect(dir.path()).unwrap();
        let children = workspace_child_paths(&report);
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn adapter_multistage_docker_uses_stage_keys() {
        let dir = TempDir::new().unwrap();
        write_package_json(dir.path(), "app", &[("start", "node server.js")]);
        fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:18 AS build\nRUN npm install\nFROM node:18 AS test\nRUN npm test\n",
        )
        .unwrap();

        let detected = detect_app(dir.path()).unwrap();
        let docker = detected.docker_commands.unwrap();
        assert!(docker.contains_key("build"));
        assert!(docker.contains_key("test"));
        assert!(docker.contains_key("run"));
    }

    #[test]
    fn adapter_orbstack_requires_orbstack_env_strategy() {
        let dir = TempDir::new().unwrap();
        write_package_json(dir.path(), "app", &[("start", "node server.js")]);
        fs::write(dir.path().join("Dockerfile"), "FROM node:18\n").unwrap();

        let detected = detect_app(dir.path()).unwrap();
        assert!(detected.orbstack_commands.is_some());
        let orbstack = detected.orbstack_commands.unwrap();
        assert!(orbstack.get("build").unwrap().contains("--context orbstack"));
    }

    #[test]
    fn adapter_k8s_apply_uses_k8s_directory() {
        let dir = TempDir::new().unwrap();
        write_package_json(dir.path(), "app", &[("start", "node server.js")]);
        fs::create_dir(dir.path().join("k8s")).unwrap();
        fs::write(
            dir.path().join("k8s/deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n",
        )
        .unwrap();

        let detected = detect_app(dir.path()).unwrap();
        let k8s = detected.k8s_commands.unwrap();
        assert!(k8s.get("apply").unwrap().contains("k8s"));
        assert!(k8s.contains_key("delete"));
    }

    #[test]
    fn adapter_k8s_suffix_manifest() {
        let dir = TempDir::new().unwrap();
        write_package_json(dir.path(), "app", &[("start", "node server.js")]);
        fs::write(
            dir.path().join("deployment.k8s.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n",
        )
        .unwrap();

        let detected = detect_app(dir.path()).unwrap();
        assert!(detected.k8s_commands.is_some());
    }

    #[test]
    fn adapter_report_has_expected_strategies_for_nodejs() {
        let dir = TempDir::new().unwrap();
        write_package_json(dir.path(), "app", &[("start", "node server.js")]);

        let report = detect_single(dir.path()).unwrap();
        assert!(report.has("nodejs"));
        assert!(report.has("local-env"));
    }

    #[test]
    fn detect_app_type_and_extract_name_via_adapter() {
        let dir = TempDir::new().unwrap();
        write_package_json(dir.path(), "@scope/pkg", &[("start", "node")]);

        assert_eq!(detect_app_type(dir.path()).unwrap(), "nodejs");
        assert_eq!(extract_app_name(dir.path(), "nodejs").unwrap(), "pkg");
    }
}
