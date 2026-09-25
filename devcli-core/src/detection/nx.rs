// Nx monorepo detection — app-detector hierarchical workspaces + nx show project enhancement

use crate::utils::path::contract_tilde;
use crate::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::detection::DetectedApp;

/// Detect all apps within an Nx monorepo via app-detector Phase 1.5.
pub fn detect_nx_apps(workspace_root: &Path) -> Result<Vec<DetectedApp>> {
    let mut detected_apps = Vec::new();

    let report = crate::app_detector_support::detect(workspace_root)?;
    let child_paths = crate::app_detector_support::workspace_child_paths(&report);

    if !child_paths.is_empty() {
        for child_path in child_paths {
            if let Ok(app) = detect_single_nx_app(&child_path, workspace_root) {
                detected_apps.push(app);
            }
        }
    } else {
        for dir_name in &["apps", "packages", "libs"] {
            let dir_path = workspace_root.join(dir_name);
            if !dir_path.exists() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(&dir_path) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Ok(app) = detect_single_nx_app(&entry.path(), workspace_root) {
                            detected_apps.push(app);
                        }
                    }
                }
            }
        }
    }

    if let Ok(workspace_app) = detect_nx_workspace(workspace_root) {
        detected_apps.push(workspace_app);
    }

    if detected_apps.is_empty() {
        anyhow::bail!("No apps found in apps/, packages/, or libs/ directories");
    }

    Ok(detected_apps)
}

/// Detect a single Nx app: app-detector env capabilities + `nx show project` targets.
pub fn detect_single_nx_app(app_path: &Path, workspace_root: &Path) -> Result<DetectedApp> {
    let app_name = app_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Could not extract app name"))?
        .to_string();

    let report = crate::app_detector_support::detect_single(app_path)?;
    let env = crate::app_detector_support::build_detected_app(app_path, &report)?;

    let mut local_commands = nx_show_project_commands(&app_name, workspace_root);
    if local_commands.is_empty() {
        local_commands = env.local_commands.clone().unwrap_or_default();
    }

    let suggested_local_default = if local_commands.contains_key("serve") {
        Some("serve".to_string())
    } else if local_commands.contains_key("start") {
        Some("start".to_string())
    } else {
        local_commands.keys().next().cloned()
    };

    Ok(DetectedApp {
        app_type: "nx".to_string(),
        app_name,
        path: contract_tilde(app_path),
        local_commands: if local_commands.is_empty() {
            None
        } else {
            Some(local_commands)
        },
        docker_commands: env.docker_commands,
        orbstack_commands: env.orbstack_commands,
        k8s_commands: env.k8s_commands,
        suggested_local_default,
        suggested_docker_default: env.suggested_docker_default,
        suggested_orbstack_default: env.suggested_orbstack_default,
        dockerfile_path: env.dockerfile_path,
        env_files: None,
    })
}

fn nx_show_project_commands(app_name: &str, workspace_root: &Path) -> HashMap<String, String> {
    let mut local_commands = HashMap::new();

    for nx_cmd in &["npx nx", "nx"] {
        let mut cmd = std::process::Command::new(if nx_cmd.contains("npx") { "npx" } else { "nx" });
        if nx_cmd.contains("npx") {
            cmd.arg("nx");
        }

        if let Ok(output) = cmd
            .args(["show", "project", app_name, "--json"])
            .current_dir(workspace_root)
            .output()
        {
            if output.status.success() {
                if let Ok(json_str) = String::from_utf8(output.stdout) {
                    if let Ok(project_config) = serde_json::from_str::<Value>(&json_str) {
                        if let Some(targets) =
                            project_config.get("targets").and_then(|t| t.as_object())
                        {
                            for (target_name, _) in targets {
                                local_commands.insert(
                                    target_name.clone(),
                                    format!("{} run {}:{}", nx_cmd, app_name, target_name),
                                );
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    local_commands
}

/// Workspace-level Nx commands merged with app-detector nx-env targets.
pub fn detect_nx_workspace(workspace_root: &Path) -> Result<DetectedApp> {
    let report = crate::app_detector_support::detect_single(workspace_root)?;
    let env = crate::app_detector_support::build_detected_app(workspace_root, &report)?;

    let mut local_commands = HashMap::new();
    let nx_cmd = "npx nx";

    if let Some(nx_env) = report.get("nx-env") {
        if let app_detector::DetectionData::NxEnv(info) = &nx_env.data {
            local_commands.extend(info.commands.clone());
        }
    }

    local_commands.insert(
        "build".to_string(),
        format!("{} run-many --target=build --all", nx_cmd),
    );
    local_commands.insert(
        "test".to_string(),
        format!("{} run-many --target=test --all", nx_cmd),
    );
    local_commands.insert(
        "lint".to_string(),
        format!("{} run-many --target=lint --all", nx_cmd),
    );
    local_commands.insert(
        "start".to_string(),
        format!("{} run-many --target=serve --all", nx_cmd),
    );
    local_commands.insert(
        "build:parallel".to_string(),
        format!("{} run-many --target=build --all --parallel", nx_cmd),
    );
    local_commands.insert(
        "test:parallel".to_string(),
        format!("{} run-many --target=test --all --parallel", nx_cmd),
    );
    local_commands.insert(
        "lint:parallel".to_string(),
        format!("{} run-many --target=lint --all --parallel", nx_cmd),
    );
    local_commands.insert(
        "start:parallel".to_string(),
        format!("{} run-many --target=serve --all --parallel", nx_cmd),
    );
    local_commands.insert(
        "build:apps".to_string(),
        format!(
            "{} run-many --target=build --projects=type:application --all",
            nx_cmd
        ),
    );
    local_commands.insert(
        "build:libs".to_string(),
        format!(
            "{} run-many --target=build --projects=type:library --all",
            nx_cmd
        ),
    );
    local_commands.insert(
        "test:apps".to_string(),
        format!(
            "{} run-many --target=test --projects=type:application --all",
            nx_cmd
        ),
    );
    local_commands.insert(
        "test:libs".to_string(),
        format!(
            "{} run-many --target=test --projects=type:library --all",
            nx_cmd
        ),
    );
    local_commands.insert(
        "lint:apps".to_string(),
        format!(
            "{} run-many --target=lint --projects=type:application --all",
            nx_cmd
        ),
    );
    local_commands.insert(
        "lint:libs".to_string(),
        format!(
            "{} run-many --target=lint --projects=type:library --all",
            nx_cmd
        ),
    );
    local_commands.insert(
        "start:apps".to_string(),
        format!(
            "{} run-many --target=serve --projects=type:application --all",
            nx_cmd
        ),
    );
    local_commands.insert(
        "build:prod".to_string(),
        format!(
            "{} run-many --target=build --configuration=production --all",
            nx_cmd
        ),
    );
    local_commands.insert(
        "start:prod".to_string(),
        format!(
            "{} run-many --target=serve --configuration=production --all",
            nx_cmd
        ),
    );
    local_commands.insert(
        "start:dev".to_string(),
        format!(
            "{} run-many --target=serve --configuration=development --all",
            nx_cmd
        ),
    );
    local_commands.insert("affected".to_string(), format!("{} affected", nx_cmd));
    local_commands.insert(
        "affected:build".to_string(),
        format!("{} affected --target=build", nx_cmd),
    );
    local_commands.insert(
        "affected:test".to_string(),
        format!("{} affected --target=test", nx_cmd),
    );
    local_commands.insert(
        "affected:lint".to_string(),
        format!("{} affected --target=lint", nx_cmd),
    );
    local_commands.insert(
        "affected:e2e".to_string(),
        format!("{} affected --target=e2e", nx_cmd),
    );
    local_commands.insert("graph".to_string(), format!("{} graph", nx_cmd));
    local_commands.insert(
        "list-projects".to_string(),
        format!("{} show projects", nx_cmd),
    );
    local_commands.insert("list".to_string(), format!("{} list", nx_cmd));
    local_commands.insert("reset".to_string(), format!("{} reset", nx_cmd));
    local_commands.insert("repair".to_string(), format!("{} repair", nx_cmd));
    local_commands.insert("migrate".to_string(), format!("{} migrate", nx_cmd));
    local_commands.insert("daemon".to_string(), format!("{} daemon", nx_cmd));

    let suggested_local_default = if local_commands.contains_key("build") {
        Some("build".to_string())
    } else if local_commands.contains_key("graph") {
        Some("graph".to_string())
    } else {
        local_commands.keys().next().cloned()
    };

    Ok(DetectedApp {
        app_type: "nx-workspace".to_string(),
        app_name: "workspace".to_string(),
        path: contract_tilde(workspace_root),
        local_commands: if local_commands.is_empty() {
            None
        } else {
            Some(local_commands)
        },
        docker_commands: env.docker_commands,
        orbstack_commands: env.orbstack_commands,
        k8s_commands: env.k8s_commands,
        suggested_local_default,
        suggested_docker_default: env.suggested_docker_default,
        suggested_orbstack_default: env.suggested_orbstack_default,
        dockerfile_path: env.dockerfile_path,
        env_files: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_nx_workspace() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("nx.json"), r#"{"version": 2}"#).unwrap();

        let result = detect_nx_workspace(temp_dir.path()).unwrap();
        assert_eq!(result.app_type, "nx-workspace");
        assert_eq!(result.app_name, "workspace");
        assert!(result.local_commands.is_some());

        let commands = result.local_commands.unwrap();
        assert!(commands.contains_key("build"));
        assert!(commands.contains_key("test"));
        assert!(commands.contains_key("lint"));
        assert!(commands.contains_key("start"));
        assert!(commands.contains_key("build:parallel"));
        assert!(commands.contains_key("build:apps"));
        assert!(commands.contains_key("build:libs"));
        assert!(commands.contains_key("affected:build"));
        assert!(commands.contains_key("graph"));
        assert_eq!(result.suggested_local_default, Some("build".to_string()));
    }

    #[test]
    fn test_detect_nx_apps_includes_workspace() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("nx.json"), r#"{"version": 2}"#).unwrap();

        let apps_dir = temp_dir.path().join("apps");
        fs::create_dir(&apps_dir).unwrap();
        let app_dir = apps_dir.join("test-app");
        fs::create_dir(&app_dir).unwrap();
        fs::write(
            app_dir.join("package.json"),
            r#"{"name": "test-app", "scripts": {"build": "nx build"}}"#,
        )
        .unwrap();

        let result = detect_nx_apps(temp_dir.path()).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|app| app.app_name == "workspace"));
    }

    #[test]
    fn test_nx_app_with_dockerfile_generates_orbstack_commands() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("nx.json"), r#"{"version": 2}"#).unwrap();

        let app_dir = temp_dir.path().join("apps/test-app");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(
            app_dir.join("package.json"),
            r#"{"name": "test-app", "scripts": {"build": "nx build"}}"#,
        )
        .unwrap();
        fs::write(
            app_dir.join("Dockerfile"),
            "FROM node:18\nCOPY . .\nRUN npm install\nCMD [\"npm\", \"start\"]",
        )
        .unwrap();

        let result = detect_single_nx_app(&app_dir, temp_dir.path()).unwrap();
        assert_eq!(result.app_type, "nx");
        assert!(result.docker_commands.is_some());
        assert!(result.orbstack_commands.is_some());
        let orbstack = result.orbstack_commands.unwrap();
        assert!(orbstack
            .get("build")
            .unwrap()
            .contains("--context orbstack"));
        assert!(orbstack.get("run").unwrap().contains("-p 3000:3000"));
    }

    #[test]
    fn test_nx_workspace_with_dockerfile_generates_orbstack_commands() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("nx.json"), r#"{"version": 2}"#).unwrap();
        fs::write(
            temp_dir.path().join("Dockerfile"),
            "FROM node:18\nCOPY . .\nRUN npm install\nCMD [\"npm\", \"start\"]",
        )
        .unwrap();

        let result = detect_nx_workspace(temp_dir.path()).unwrap();
        assert!(result.docker_commands.is_some());
        assert!(result.orbstack_commands.is_some());
        let orbstack = result.orbstack_commands.unwrap();
        assert!(orbstack
            .get("build")
            .unwrap()
            .contains("--context orbstack"));
    }
}
