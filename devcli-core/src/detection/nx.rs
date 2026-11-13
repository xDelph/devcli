// Nx monorepo detection — app-detector hierarchical workspaces + the Nx project graph

use crate::utils::path::contract_tilde;
use crate::Result;
use app_detector::utils::package_manager::{detect_node_package_manager_at, nx_runner};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::detection::DetectedApp;

/// How long the one `nx graph` call may take before detection goes on
/// without it (project.json files, then package.json scripts).
const NX_GRAPH_TIMEOUT: Duration = Duration::from_secs(30);

/// Targets that make a project something to start rather than only build.
const RUNNABLE_TARGETS: &[&str] = &["serve", "start", "dev", "preview"];

/// One Nx project, as the project graph (or its project.json) describes it.
#[derive(Debug, Clone, Default, PartialEq)]
struct NxProject {
    name: String,
    /// `application`, `library` or `e2e` when known.
    project_type: Option<String>,
    targets: Vec<String>,
}

/// What detection knows about the workspace as a whole.
struct NxWorkspace {
    /// How to invoke the workspace's own nx (`npx --no-install nx`, ...).
    runner: &'static str,
    /// Projects by root directory, relative to the workspace root.
    projects: HashMap<PathBuf, NxProject>,
}

impl NxWorkspace {
    fn load(workspace_root: &Path) -> Self {
        Self {
            runner: workspace_nx_runner(workspace_root),
            projects: nx_project_graph(workspace_root),
        }
    }
}

/// Detect all apps within an Nx monorepo via app-detector Phase 1.5.
///
/// Libraries are left out unless they have something to start
/// (`serve`/`start`/`dev`/`preview`): a buildable lib is not an app
/// devcli can run.
pub fn detect_nx_apps(workspace_root: &Path) -> Result<Vec<DetectedApp>> {
    let mut detected_apps = Vec::new();
    let workspace = NxWorkspace::load(workspace_root);

    let report = crate::app_detector_support::detect(workspace_root)?;
    let mut child_paths = crate::app_detector_support::workspace_child_paths(&report);

    if child_paths.is_empty() {
        for dir_name in &["apps", "packages", "libs"] {
            let Ok(entries) = fs::read_dir(workspace_root.join(dir_name)) else {
                continue;
            };
            let mut dirs: Vec<PathBuf> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect();
            dirs.sort();
            child_paths.extend(dirs);
        }
    }

    for child_path in child_paths {
        let project = project_for(&child_path, workspace_root, &workspace);
        let Ok(app) = detect_nx_app_with(&child_path, &workspace, project.as_ref()) else {
            continue;
        };
        if is_library(&child_path, workspace_root, project.as_ref()) && !is_runnable(&app) {
            continue;
        }
        detected_apps.push(app);
    }

    if let Ok(workspace_app) = detect_nx_workspace(workspace_root) {
        detected_apps.push(workspace_app);
    }

    if detected_apps.is_empty() {
        anyhow::bail!("No apps found in apps/, packages/, or libs/ directories");
    }

    Ok(detected_apps)
}

/// Detect a single Nx app: app-detector env capabilities + its Nx targets.
pub fn detect_single_nx_app(app_path: &Path, workspace_root: &Path) -> Result<DetectedApp> {
    let workspace = NxWorkspace::load(workspace_root);
    let project = project_for(app_path, workspace_root, &workspace);
    detect_nx_app_with(app_path, &workspace, project.as_ref())
}

fn detect_nx_app_with(
    app_path: &Path,
    workspace: &NxWorkspace,
    project: Option<&NxProject>,
) -> Result<DetectedApp> {
    let app_name = app_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Could not extract app name"))?
        .to_string();

    let report = crate::app_detector_support::detect_single(app_path)?;
    // A project with only a project.json has no app type app-detector
    // knows; its Nx targets are what makes it an app.
    let env = match crate::app_detector_support::build_detected_app(app_path, &report) {
        Ok(env) => env,
        Err(err) if project.is_none_or(|p| p.targets.is_empty()) => return Err(err),
        Err(_) => env_capabilities_only(app_path, &report),
    };

    let mut local_commands: HashMap<String, String> = project
        .map(|project| {
            project
                .targets
                .iter()
                .map(|target| {
                    (
                        target.clone(),
                        format!("{} run {}:{}", workspace.runner, project.name, target),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    if local_commands.is_empty() {
        local_commands = env.local_commands.clone().unwrap_or_default();
    }

    let suggested_local_default = crate::app_detector_support::pick_default_command(
        Some(&local_commands),
        &["serve", "start", "dev"],
    );

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

/// Docker / OrbStack / k8s detection of a folder with no app type.
fn env_capabilities_only(app_path: &Path, report: &app_detector::DetectionReport) -> DetectedApp {
    use crate::app_detector_support as support;
    let docker_commands = support::extract_docker_commands(report);
    let orbstack_commands = support::extract_orbstack_commands(report);
    DetectedApp {
        app_type: "nx".to_string(),
        app_name: String::new(),
        path: contract_tilde(app_path),
        local_commands: None,
        suggested_docker_default: support::suggest_docker_default(&docker_commands),
        suggested_orbstack_default: support::suggest_orbstack_default(&orbstack_commands),
        docker_commands,
        orbstack_commands,
        k8s_commands: support::extract_k8s_commands(report),
        suggested_local_default: None,
        dockerfile_path: support::resolve_dockerfile_path(app_path, report),
        env_files: None,
    }
}

/// The project at `app_path`: from the project graph, else from its
/// project.json (targets declared there; inferred ones need the graph).
fn project_for(
    app_path: &Path,
    workspace_root: &Path,
    workspace: &NxWorkspace,
) -> Option<NxProject> {
    relative_root(app_path, workspace_root)
        .and_then(|root| workspace.projects.get(&root).cloned())
        .or_else(|| project_from_project_json(app_path))
}

fn relative_root(app_path: &Path, workspace_root: &Path) -> Option<PathBuf> {
    if let Ok(rel) = app_path.strip_prefix(workspace_root) {
        return Some(rel.to_path_buf());
    }
    let app = app_path.canonicalize().ok()?;
    let root = workspace_root.canonicalize().ok()?;
    app.strip_prefix(root).ok().map(Path::to_path_buf)
}

fn project_from_project_json(app_path: &Path) -> Option<NxProject> {
    let content = fs::read_to_string(app_path.join("project.json")).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;
    let folder = app_path.file_name()?.to_str()?;
    let mut targets: Vec<String> = json
        .get("targets")
        .and_then(Value::as_object)
        .map(|targets| targets.keys().cloned().collect())
        .unwrap_or_default();
    targets.sort();
    Some(NxProject {
        name: nx_project_name(app_path, folder),
        project_type: json
            .get("projectType")
            .and_then(Value::as_str)
            .map(str::to_string),
        targets,
    })
}

/// A library: Nx says so, or (without Nx's word) it lives under `libs/`.
fn is_library(app_path: &Path, workspace_root: &Path, project: Option<&NxProject>) -> bool {
    match project.and_then(|p| p.project_type.as_deref()) {
        Some(kind) => kind == "library",
        None => relative_root(app_path, workspace_root)
            .and_then(|rel| rel.components().next().map(|c| c.as_os_str() == "libs"))
            .unwrap_or(false),
    }
}

fn is_runnable(app: &DetectedApp) -> bool {
    app.local_commands
        .as_ref()
        .is_some_and(|cmds| RUNNABLE_TARGETS.iter().any(|t| cmds.contains_key(*t)))
        || app.docker_commands.is_some()
}

/// The Nx project name of `app_path`.
///
/// Nx addresses projects by the name in `project.json` (or `package.json`),
/// often scoped (`@org/web`), not by folder; asking for the folder name made
/// `nx show project` fail and the app fall back to generic commands.
fn nx_project_name(app_path: &Path, folder_name: &str) -> String {
    ["project.json", "package.json"]
        .iter()
        .filter_map(|file| fs::read_to_string(app_path.join(file)).ok())
        .filter_map(|content| serde_json::from_str::<Value>(&content).ok())
        .find_map(|json| json.get("name")?.as_str().map(str::to_string))
        .unwrap_or_else(|| folder_name.to_string())
}

/// How to invoke the workspace's own nx, for the commands devcli saves.
fn workspace_nx_runner(workspace_root: &Path) -> &'static str {
    nx_runner(&detect_node_package_manager_at(workspace_root))
}

/// Every project with its targets, from one `nx graph --file=stdout`.
///
/// One call for the whole workspace instead of one `nx show project` per
/// folder, bounded by [`NX_GRAPH_TIMEOUT`]. Only the workspace's own nx is
/// run (its `node_modules/.bin/nx`, else the package manager's runner, which
/// never downloads one): a global nx may be another major version. Empty
/// when nx is not installed, fails or takes too long.
fn nx_project_graph(workspace_root: &Path) -> HashMap<PathBuf, NxProject> {
    let local_bin = workspace_root.join("node_modules/.bin/nx");
    let mut command = if local_bin.is_file() {
        Command::new(local_bin)
    } else {
        let mut parts = workspace_nx_runner(workspace_root).split_whitespace();
        let Some(program) = parts.next() else {
            return HashMap::new();
        };
        let mut command = Command::new(program);
        command.args(parts);
        command
    };
    command
        .args(["graph", "--file=stdout"])
        .current_dir(workspace_root)
        .stdin(Stdio::null())
        .stderr(Stdio::null());

    output_within(&mut command, NX_GRAPH_TIMEOUT)
        .map(|stdout| parse_project_graph(&stdout))
        .unwrap_or_default()
}

/// Stdout of `command` when it succeeds within `timeout`; killed otherwise.
fn output_within(command: &mut Command, timeout: Duration) -> Option<Vec<u8>> {
    let deadline = Instant::now() + timeout;
    let mut child = command.stdout(Stdio::piped()).spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    // Read on a thread: a full pipe would block the child, and a process the
    // child leaves behind can keep the pipe open after it exits.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    if !status.success() {
        return None;
    }
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .max(Duration::from_secs(1));
    rx.recv_timeout(remaining).ok()
}

/// Projects by root from `nx graph --file=stdout` output.
fn parse_project_graph(stdout: &[u8]) -> HashMap<PathBuf, NxProject> {
    let text = String::from_utf8_lossy(stdout);
    // Tolerate anything nx prints around the JSON document.
    let Some(start) = text.find('{') else {
        return HashMap::new();
    };
    let Some(Ok(graph)) = serde_json::Deserializer::from_str(&text[start..])
        .into_iter::<Value>()
        .next()
    else {
        return HashMap::new();
    };
    let Some(nodes) = graph
        .get("graph")
        .and_then(|g| g.get("nodes"))
        .and_then(Value::as_object)
    else {
        return HashMap::new();
    };

    nodes
        .iter()
        .filter_map(|(key, node)| {
            let data = node.get("data")?;
            let root = data.get("root")?.as_str()?;
            let mut targets: Vec<String> = data
                .get("targets")
                .and_then(Value::as_object)
                .map(|targets| targets.keys().cloned().collect())
                .unwrap_or_default();
            targets.sort();
            let project_type = data
                .get("projectType")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    match node.get("type")?.as_str()? {
                        "app" => Some("application"),
                        "lib" => Some("library"),
                        "e2e" => Some("e2e"),
                        _ => None,
                    }
                    .map(str::to_string)
                });
            let name = node
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(key)
                .to_string();
            Some((
                PathBuf::from(root),
                NxProject {
                    name,
                    project_type,
                    targets,
                },
            ))
        })
        .collect()
}

/// Workspace-level Nx commands merged with app-detector nx-env targets.
pub fn detect_nx_workspace(workspace_root: &Path) -> Result<DetectedApp> {
    let report = crate::app_detector_support::detect_single(workspace_root)?;
    let env = crate::app_detector_support::build_detected_app(workspace_root, &report)?;

    let mut local_commands = HashMap::new();
    // The same runner as the nx-env commands merged below: a bare or
    // `npx nx` here and there made some commands need a global nx (or
    // download one) while others did not.
    let nx_cmd = workspace_nx_runner(workspace_root);

    if let Some(nx_env) = report.get("nx-env")
        && let app_detector::DetectionData::NxEnv(info) = &nx_env.data
    {
        local_commands.extend(info.commands.clone());
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

    let suggested_local_default = crate::app_detector_support::pick_default_command(
        Some(&local_commands),
        &["build", "graph"],
    );

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

    #[test]
    fn nx_project_name_prefers_project_json() {
        let dir = tempfile::TempDir::new().unwrap();
        assert_eq!(nx_project_name(dir.path(), "web"), "web");
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"name":"@org/web-pkg"}"#,
        )
        .unwrap();
        assert_eq!(nx_project_name(dir.path(), "web"), "@org/web-pkg");
        std::fs::write(dir.path().join("project.json"), r#"{"name":"@org/web"}"#).unwrap();
        assert_eq!(nx_project_name(dir.path(), "web"), "@org/web");
    }

    /// An Nx workspace root of its own repository (the package-manager
    /// search stops at `.git`).
    fn nx_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join("nx.json"), r#"{"version": 2}"#).unwrap();
        dir
    }

    #[test]
    fn workspace_commands_all_run_the_workspace_nx() {
        let dir = nx_repo();
        fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        fs::write(
            dir.path().join("nx.json"),
            r#"{"targetDefaults": {"serve": {}, "e2e": {}, "build": {}}}"#,
        )
        .unwrap();

        let commands = detect_nx_workspace(dir.path())
            .unwrap()
            .local_commands
            .unwrap();
        assert_eq!(commands["serve"], "pnpm exec nx run-many --target=serve");
        assert_eq!(
            commands["affected-e2e"],
            "pnpm exec nx affected --target=e2e"
        );
        assert_eq!(
            commands["build"],
            "pnpm exec nx run-many --target=build --all"
        );
        for (key, cmd) in &commands {
            assert!(cmd.starts_with("pnpm exec nx "), "{key}: {cmd}");
        }
    }

    #[test]
    fn project_json_targets_are_used_without_an_nx_install() {
        let dir = nx_repo();
        let app = dir.path().join("apps/web");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            app.join("project.json"),
            r#"{"name":"@org/web","projectType":"application","targets":{"serve":{},"build":{}}}"#,
        )
        .unwrap();

        let detected = detect_single_nx_app(&app, dir.path()).unwrap();
        let commands = detected.local_commands.unwrap();
        assert_eq!(commands["serve"], "npx --no-install nx run @org/web:serve");
        assert_eq!(commands["build"], "npx --no-install nx run @org/web:build");
        assert_eq!(detected.suggested_local_default.as_deref(), Some("serve"));
    }

    #[test]
    fn libraries_without_anything_to_start_are_not_apps() {
        let dir = nx_repo();
        let write_project = |path: &str, json: &str| {
            let project = dir.path().join(path);
            fs::create_dir_all(&project).unwrap();
            fs::write(project.join("project.json"), json).unwrap();
        };
        write_project(
            "apps/web",
            r#"{"name":"web","projectType":"application","targets":{"serve":{}}}"#,
        );
        write_project(
            "libs/ui",
            r#"{"name":"ui","projectType":"library","targets":{"build":{},"test":{}}}"#,
        );
        write_project(
            "libs/storybook",
            r#"{"name":"storybook","projectType":"library","targets":{"serve":{}}}"#,
        );

        let names: Vec<String> = detect_nx_apps(dir.path())
            .unwrap()
            .into_iter()
            .map(|app| app.app_name)
            .collect();
        assert!(names.contains(&"web".to_string()), "{names:?}");
        assert!(names.contains(&"storybook".to_string()), "{names:?}");
        assert!(!names.contains(&"ui".to_string()), "{names:?}");
    }

    #[test]
    fn project_graph_output_is_parsed_by_root() {
        let stdout = br#"
 NX  Daemon is not running
{"graph":{"nodes":{
  "web":{"name":"web","type":"app","data":{"root":"apps/web","targets":{"serve":{},"build":{}}}},
  "ui":{"name":"ui","type":"lib","data":{"root":"libs/ui","projectType":"library","targets":{"build":{}}}}
},"dependencies":{}}}
"#;
        let projects = parse_project_graph(stdout);
        assert_eq!(
            projects[Path::new("apps/web")],
            NxProject {
                name: "web".to_string(),
                project_type: Some("application".to_string()),
                targets: vec!["build".to_string(), "serve".to_string()],
            }
        );
        assert_eq!(
            projects[Path::new("libs/ui")].project_type.as_deref(),
            Some("library")
        );
        assert!(parse_project_graph(b"command not found").is_empty());
    }

    #[test]
    fn a_hanging_command_is_killed_at_the_timeout() {
        let started = Instant::now();
        let output = output_within(Command::new("sleep").arg("10"), Duration::from_millis(200));
        assert!(output.is_none());
        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(
            output_within(Command::new("echo").arg("{}"), Duration::from_secs(5)).as_deref(),
            Some(&b"{}\n"[..])
        );
    }
}
