//! Comprehensive combination tests for all detection scenarios.
//!
//! Run with `cargo test --test combination_test -- --nocapture` to see all output.

use app_detector::{
    engine::DetectionEngine,
    registry::StrategyRegistry,
    types::{AppTypeCategory, DetectionData, EnvCapabilityCategory},
};
// EnvCapabilityCategory::Nx is used in Nx-specific assertions below
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn engine() -> DetectionEngine {
    DetectionEngine::new(StrategyRegistry::with_defaults())
}

/// Print a compact summary of a report for manual inspection.
fn summarize(label: &str, report: &app_detector::types::DetectionReport) {
    eprintln!("\n══════════════════════════════════════");
    eprintln!("  {}", label);
    eprintln!("  path: {}", report.path.display());
    eprintln!("══════════════════════════════════════");
    print_report(report, 0);
}

fn print_report(report: &app_detector::types::DetectionReport, depth: usize) {
    let pad = "  ".repeat(depth);

    let app_types = report.app_types();
    if app_types.is_empty() {
        eprintln!("{}  App types:  (none)", pad);
    } else {
        eprintln!("{}  App types:", pad);
        for r in &app_types {
            eprintln!(
                "{}    • {} ({:.0}%)",
                pad,
                r.strategy_id,
                r.confidence * 100.0
            );
            print_data(&r.data, &format!("{}      ", pad));
        }
    }

    let env_caps = report.env_capabilities();
    if env_caps.is_empty() {
        eprintln!("{}  Env caps:   (none)", pad);
    } else {
        eprintln!("{}  Env caps:", pad);
        for r in &env_caps {
            eprintln!(
                "{}    • {} ({:.0}%)",
                pad,
                r.strategy_id,
                r.confidence * 100.0
            );
            print_data(&r.data, &format!("{}      ", pad));
        }
    }

    if !report.children.is_empty() {
        eprintln!("{}  Workspaces: ({} found)", pad, report.children.len());
        for child in &report.children {
            eprintln!(
                "{}  ┌─ {}",
                pad,
                child.path.file_name().unwrap_or_default().to_string_lossy()
            );
            print_report(child, depth + 1);
        }
    }
}

fn print_data(data: &DetectionData, pad: &str) {
    match data {
        DetectionData::Language(info) => {
            eprintln!("{}lang={}", pad, info.name);
            if let Some(v) = &info.version {
                eprintln!("{}version={}", pad, v);
            }
            for (k, v) in &info.metadata {
                eprintln!("{}{}={}", pad, k, v);
            }
        }
        DetectionData::Service(info) => {
            eprintln!("{}service={}", pad, info.name);
            eprintln!("{}config_files={:?}", pad, info.config_files);
        }
        DetectionData::Monorepo(info) => {
            eprintln!("{}tool={}", pad, info.tool);
            eprintln!("{}workspaces={}", pad, info.workspace_info.len());
        }
        DetectionData::LocalEnv(info) => {
            eprintln!(
                "{}commands({})={:?}",
                pad,
                info.commands.len(),
                info.commands.keys().collect::<Vec<_>>()
            );
            eprintln!("{}suggested_default={:?}", pad, info.suggested_default);
        }
        DetectionData::DockerEnv(info) => {
            eprintln!("{}dockerfiles={:?}", pad, info.dockerfiles);
            eprintln!("{}stages={:?}", pad, info.stages);
            eprintln!("{}compose_files={:?}", pad, info.compose_files);
            eprintln!(
                "{}commands({})={:?}",
                pad,
                info.commands.len(),
                info.commands.keys().collect::<Vec<_>>()
            );
        }
        DetectionData::OrbStackEnv(info) => {
            eprintln!(
                "{}commands({})={:?}",
                pad,
                info.commands.len(),
                info.commands.keys().collect::<Vec<_>>()
            );
        }
        DetectionData::KubernetesEnv(info) => {
            eprintln!("{}manifests={:?}", pad, info.manifests);
            eprintln!(
                "{}commands({})={:?}",
                pad,
                info.commands.len(),
                info.commands.keys().collect::<Vec<_>>()
            );
        }
        DetectionData::NxEnv(info) => {
            let mut targets = info.targets.clone();
            targets.sort();
            eprintln!("{}targets({})={:?}", pad, targets.len(), targets);
            eprintln!("{}suggested_default={:?}", pad, info.suggested_default);
            eprintln!(
                "{}targets_from_defaults={}",
                pad,
                info.metadata
                    .get("targets_from_defaults")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            );
            eprintln!(
                "{}targets_from_projects={}",
                pad,
                info.metadata
                    .get("targets_from_projects")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            );
        }
        _ => {}
    }
}

// ─── Rust variants ────────────────────────────────────────────────────────────

#[test]
fn test_rust_binary() {
    let report = engine()
        .detect(fixture("rust-binary"))
        .expect("Detection failed");
    summarize("rust-binary: Rust [[bin]] project", &report);

    let lang = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(lang.len(), 1, "Should detect exactly 1 language");
    assert_eq!(lang[0].strategy_id, "rust");

    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(local.len(), 1, "Should detect local-env");

    match &local[0].data {
        DetectionData::LocalEnv(info) => {
            // Binary project should have `cargo run --release` as start
            assert!(
                info.commands.contains_key("run"),
                "Should have 'run' command"
            );
            assert!(
                info.commands.contains_key("build"),
                "Should have 'build' command"
            );
            assert!(
                info.commands.contains_key("test"),
                "Should have 'test' command"
            );
            assert!(
                info.commands.contains_key("start"),
                "Binary project should have 'start' = cargo run --release"
            );
            assert_eq!(info.commands["start"], "cargo run --release");
        }
        _ => panic!("Expected LocalEnv"),
    }

    // No docker
    assert!(
        report
            .by_env_capability(&EnvCapabilityCategory::Docker)
            .is_empty(),
        "No Dockerfile → no Docker env"
    );
    // No children
    assert!(report.children.is_empty(), "Rust project is not a monorepo");
}

#[test]
fn test_rust_docker() {
    let report = engine()
        .detect(fixture("rust-docker"))
        .expect("Detection failed");
    summarize("rust-docker: Rust + single-stage Dockerfile", &report);

    let lang = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(lang[0].strategy_id, "rust");

    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(docker.len(), 1, "Should detect Docker");

    match &docker[0].data {
        DetectionData::DockerEnv(info) => {
            assert_eq!(info.dockerfiles.len(), 1);
            // Multi-stage: builder + debian slim
            assert!(!info.stages.is_empty(), "Should detect stages");
            assert!(
                info.stages.contains(&"builder".to_string()),
                "Should detect 'builder' stage"
            );
            assert!(
                info.commands.contains_key("build"),
                "Should have build command"
            );
            assert!(
                info.commands.contains_key("builder"),
                "Should have stage-specific builder command"
            );
        }
        _ => panic!("Expected DockerEnv"),
    }

    // OrbStack should mirror Docker
    let orbstack = report.by_env_capability(&EnvCapabilityCategory::OrbStack);
    assert_eq!(orbstack.len(), 1, "OrbStack should be detected");
    match (&docker[0].data, &orbstack[0].data) {
        (DetectionData::DockerEnv(d), DetectionData::OrbStackEnv(o)) => {
            assert_eq!(
                d.commands.len(),
                o.commands.len(),
                "OrbStack and Docker should have identical command count"
            );
        }
        _ => panic!("Unexpected data types"),
    }
}

// ─── Python variants ──────────────────────────────────────────────────────────

#[test]
fn test_python_poetry() {
    let report = engine()
        .detect(fixture("python-poetry"))
        .expect("Detection failed");
    summarize("python-poetry: Python + Poetry", &report);

    let lang = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(lang.len(), 1, "Should detect exactly 1 language");
    assert_eq!(lang[0].strategy_id, "python");

    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(local.len(), 1, "Should detect local-env");

    match &local[0].data {
        DetectionData::LocalEnv(info) => {
            // Poetry project: install should use `poetry install`
            assert!(
                info.commands.contains_key("install"),
                "Should have install command"
            );
            assert_eq!(
                info.commands["install"], "poetry install",
                "Poetry project should use 'poetry install'"
            );
            // Start should use `poetry run python main.py`
            assert!(
                info.commands.contains_key("start"),
                "Should have start command"
            );
            assert_eq!(
                info.commands["start"], "poetry run python main.py",
                "Poetry project should use 'poetry run python main.py'"
            );
        }
        _ => panic!("Expected LocalEnv"),
    }

    // No docker in this fixture
    assert!(
        report
            .by_env_capability(&EnvCapabilityCategory::Docker)
            .is_empty(),
        "No Dockerfile → no Docker env"
    );
}

#[test]
fn test_python_docker_compose() {
    let report = engine()
        .detect(fixture("python-docker-compose"))
        .expect("Detection failed");
    summarize(
        "python-docker-compose: Python + docker-compose (no Dockerfile)",
        &report,
    );

    let lang = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(lang[0].strategy_id, "python");

    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(
        docker.len(),
        1,
        "docker-compose.yml should trigger Docker env"
    );

    match &docker[0].data {
        DetectionData::DockerEnv(info) => {
            assert_eq!(info.dockerfiles.len(), 0, "No Dockerfile in this fixture");
            assert_eq!(
                info.compose_files.len(),
                1,
                "Should detect docker-compose.yml"
            );
            assert!(
                info.commands.contains_key("up"),
                "Compose → should have 'up' command"
            );
            assert!(
                info.commands.contains_key("down"),
                "Compose → should have 'down' command"
            );
        }
        _ => panic!("Expected DockerEnv"),
    }
}

// ─── Node.js variants ─────────────────────────────────────────────────────────

#[test]
fn test_nodejs_docker_compose_only() {
    let report = engine()
        .detect(fixture("nodejs-docker-compose"))
        .expect("Detection failed");
    summarize(
        "nodejs-docker-compose: Node.js + docker-compose (no Dockerfile)",
        &report,
    );

    let lang = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(lang[0].strategy_id, "nodejs");

    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(local.len(), 1);

    match &local[0].data {
        DetectionData::LocalEnv(info) => {
            assert!(info.commands.contains_key("start"));
            assert!(info.commands.contains_key("dev"));
            assert!(info.commands.contains_key("test"));
            assert!(info.commands.contains_key("lint"));
        }
        _ => panic!("Expected LocalEnv"),
    }

    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(
        docker.len(),
        1,
        "docker-compose.yml should trigger Docker env"
    );

    match &docker[0].data {
        DetectionData::DockerEnv(info) => {
            assert_eq!(info.dockerfiles.len(), 0, "No Dockerfile in this fixture");
            assert_eq!(info.compose_files.len(), 1);
            assert!(info.commands.contains_key("up"));
        }
        _ => panic!("Expected DockerEnv"),
    }
}

#[test]
fn test_nodejs_full_all_envs() {
    let report = engine()
        .detect(fixture("nodejs-full"))
        .expect("Detection failed");
    summarize(
        "nodejs-full: Node.js + Dockerfile + docker-compose + k8s",
        &report,
    );

    let lang = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(lang[0].strategy_id, "nodejs");

    // Should detect all 4 env capabilities
    let env_caps = report.env_capabilities();
    let cap_ids: Vec<&str> = env_caps.iter().map(|r| r.strategy_id.as_str()).collect();
    eprintln!("Detected env capabilities: {:?}", cap_ids);

    assert!(cap_ids.contains(&"docker"), "Should detect Docker");
    assert!(cap_ids.contains(&"orbstack-env"), "Should detect OrbStack");
    assert!(
        cap_ids.contains(&"kubernetes-env"),
        "Should detect Kubernetes"
    );
    assert!(cap_ids.contains(&"local-env"), "Should detect local-env");

    // Docker should detect both Dockerfile AND docker-compose
    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    match &docker[0].data {
        DetectionData::DockerEnv(info) => {
            // Has docker-compose, so compose commands should win
            assert_eq!(
                info.compose_files.len(),
                1,
                "Should detect docker-compose.yml"
            );
            assert!(
                info.commands.contains_key("up"),
                "Should have 'up' command from compose"
            );
        }
        _ => panic!("Expected DockerEnv"),
    }

    let k8s = report.by_env_capability(&EnvCapabilityCategory::Kubernetes);
    match &k8s[0].data {
        DetectionData::KubernetesEnv(info) => {
            assert!(
                info.manifests.len() >= 2,
                "Should find deployment.yaml and service.yaml"
            );
            assert!(info.commands.contains_key("apply"));
        }
        _ => panic!("Expected KubernetesEnv"),
    }

    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    match &local[0].data {
        DetectionData::LocalEnv(info) => {
            assert!(info.commands.contains_key("start"));
            assert!(info.commands.contains_key("dev"));
            assert!(info.commands.contains_key("build"));
            assert!(info.commands.contains_key("test"));
        }
        _ => panic!("Expected LocalEnv"),
    }
}

#[test]
fn test_nodejs_no_scripts() {
    let report = engine()
        .detect(fixture("nodejs-no-scripts"))
        .expect("Detection failed");
    summarize("nodejs-no-scripts: package.json without scripts", &report);

    let lang = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(lang[0].strategy_id, "nodejs");

    // local-env should still be detected but have 0 commands (no scripts to extract)
    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(
        local.len(),
        1,
        "local-env should still run (nodejs detected)"
    );

    match &local[0].data {
        DetectionData::LocalEnv(info) => {
            assert_eq!(
                info.commands.len(),
                0,
                "No scripts in package.json → no local commands"
            );
            assert_eq!(
                info.suggested_default, None,
                "No commands → no suggested default"
            );
        }
        _ => panic!("Expected LocalEnv"),
    }
}

// ─── Docker-only (no app type) ────────────────────────────────────────────────

#[test]
fn test_docker_only() {
    let report = engine()
        .detect(fixture("docker-only"))
        .expect("Detection failed");
    summarize(
        "docker-only: Only Dockerfile, no recognized language",
        &report,
    );

    // No app type should be detected
    assert!(
        report.app_types().is_empty(),
        "No app type should be detected for a bare Dockerfile"
    );

    // Docker env should be detected
    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(docker.len(), 1, "Dockerfile should trigger Docker env");

    match &docker[0].data {
        DetectionData::DockerEnv(info) => {
            assert_eq!(info.dockerfiles.len(), 1);
            assert_eq!(
                info.stages.len(),
                0,
                "No FROM ... AS stages in this Dockerfile"
            );
            assert!(info.commands.contains_key("build"));
            // No stage-specific commands expected
            assert!(!info.commands.contains_key("build-builder"));
        }
        _ => panic!("Expected DockerEnv"),
    }

    // No local-env (no app type to generate commands for)
    assert!(
        report
            .by_env_capability(&EnvCapabilityCategory::Local)
            .is_empty(),
        "No local-env without a recognized app type"
    );
}

// ─── Nx monorepo variants ─────────────────────────────────────────────────────

#[test]
fn test_nx_monorepo_task_runner() {
    let report = engine()
        .detect(fixture("nx-monorepo"))
        .expect("Detection failed");
    summarize(
        "nx-monorepo: Nx task runner commands from targetDefaults + project.json",
        &report,
    );

    // Root: Nx detected
    let monorepos = report.by_app_type(&AppTypeCategory::Monorepo);
    assert_eq!(monorepos.len(), 1);
    assert_eq!(monorepos[0].strategy_id, "nx");

    // nx-env must be present at the root
    let nx_env = report.by_env_capability(&EnvCapabilityCategory::Nx);
    assert_eq!(
        nx_env.len(),
        1,
        "nx-env strategy should fire at monorepo root"
    );

    match &nx_env[0].data {
        DetectionData::NxEnv(info) => {
            // Source A (targetDefaults): build, test, lint
            // Source B (project.json web: build, serve, test, lint; api: build, serve, test)
            // Union: build, lint, serve, test
            assert!(
                info.targets.contains(&"build".to_string()),
                "Should discover 'build' target"
            );
            assert!(
                info.targets.contains(&"test".to_string()),
                "Should discover 'test' target"
            );
            assert!(
                info.targets.contains(&"lint".to_string()),
                "Should discover 'lint' target"
            );
            assert!(
                info.targets.contains(&"serve".to_string()),
                "Should discover 'serve' target from project.json"
            );

            // Commands must be nx run-many --target=X (no hardcoding)
            assert_eq!(info.commands["build"], "nx run-many --target=build");
            assert_eq!(info.commands["test"], "nx run-many --target=test");
            assert_eq!(info.commands["serve"], "nx run-many --target=serve");

            // Affected variants must also be generated
            assert!(
                info.commands.contains_key("affected-build"),
                "Should have affected-build command"
            );
            assert!(
                info.commands.contains_key("affected-test"),
                "Should have affected-test command"
            );

            // suggested_default should prefer serve or build
            assert!(
                info.suggested_default == Some("serve".to_string())
                    || info.suggested_default == Some("build".to_string()),
                "Suggested default should be 'serve' or 'build', got {:?}",
                info.suggested_default
            );

            // Targets must be alphabetically sorted
            let sorted = {
                let mut v = info.targets.clone();
                v.sort();
                v
            };
            assert_eq!(
                info.targets, sorted,
                "Targets must be alphabetically sorted"
            );

            assert_eq!(
                nx_env[0].confidence, 1.0,
                "Full confidence when targets found"
            );
        }
        _ => panic!("Expected NxEnv data"),
    }

    // nx-env must NOT appear in workspace children (it's root-only)
    for child in &report.children {
        let child_nx_env = child.by_env_capability(&EnvCapabilityCategory::Nx);
        assert!(
            child_nx_env.is_empty(),
            "nx-env should not appear inside workspace {:?}",
            child.path
        );
    }
}

#[test]
fn test_nx_with_root_compose() {
    let report = engine()
        .detect(fixture("nx-with-root-compose"))
        .expect("Detection failed");
    summarize(
        "nx-with-root-compose: Nx + root docker-compose.yml (infra)",
        &report,
    );

    // Root: Nx detected
    let nx = report.by_app_type(&AppTypeCategory::Monorepo);
    assert_eq!(nx.len(), 1);
    assert_eq!(nx[0].strategy_id, "nx");

    // Root-level docker-compose.yml should be detected at root
    let docker_at_root = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(
        docker_at_root.len(),
        1,
        "Root docker-compose.yml should be detected at root level"
    );

    match &docker_at_root[0].data {
        DetectionData::DockerEnv(info) => {
            assert_eq!(
                info.compose_files.len(),
                1,
                "Should detect root docker-compose.yml"
            );
            // Should NOT include workspace Dockerfiles
            assert!(
                info.dockerfiles.is_empty(),
                "Root-level Docker should not include workspace Dockerfiles"
            );
            assert!(
                info.commands.contains_key("up"),
                "Should have compose 'up' command"
            );
        }
        _ => panic!("Expected DockerEnv"),
    }

    // nx-env must be present at root with targets from project.json
    let nx_env = report.by_env_capability(&EnvCapabilityCategory::Nx);
    assert_eq!(nx_env.len(), 1, "nx-env should fire at monorepo root");
    match &nx_env[0].data {
        DetectionData::NxEnv(info) => {
            assert!(info.targets.contains(&"build".to_string()));
            assert!(info.targets.contains(&"serve".to_string()));
            assert!(info.targets.contains(&"test".to_string()));
            assert!(!info.commands.is_empty(), "nx-env must produce commands");
        }
        _ => panic!("Expected NxEnv data"),
    }

    // Should have 2 workspace children
    assert_eq!(report.children.len(), 2, "Should have 2 workspace children");

    // Each workspace detects its own Docker but NOT nx-env
    for child in &report.children {
        let child_docker = child.by_env_capability(&EnvCapabilityCategory::Docker);
        assert_eq!(
            child_docker.len(),
            1,
            "Workspace {:?} should have its own Docker detection",
            child.path
        );

        match &child_docker[0].data {
            DetectionData::DockerEnv(info) => {
                assert_eq!(
                    info.dockerfiles.len(),
                    1,
                    "Workspace should see only its own Dockerfile, not root compose"
                );
                assert!(
                    info.compose_files.is_empty(),
                    "Workspace should NOT see the root docker-compose.yml"
                );
            }
            _ => panic!("Expected DockerEnv"),
        }

        let child_nx_env = child.by_env_capability(&EnvCapabilityCategory::Nx);
        assert!(
            child_nx_env.is_empty(),
            "nx-env must not appear inside workspace {:?}",
            child.path
        );
    }
}

#[test]
fn test_nx_mixed_workspaces() {
    let report = engine()
        .detect(fixture("nx-mixed-workspaces"))
        .expect("Detection failed");
    summarize(
        "nx-mixed-workspaces: Nx with Node.js frontend + Rust backend",
        &report,
    );

    // Root: Nx detected
    let nx = report.by_app_type(&AppTypeCategory::Monorepo);
    assert_eq!(nx[0].strategy_id, "nx");

    // Root: no Docker (no root Dockerfile or compose)
    assert!(
        report
            .by_env_capability(&EnvCapabilityCategory::Docker)
            .is_empty(),
        "No Docker at monorepo root"
    );

    // nx-env must be at root with targets from targetDefaults + project.json
    let nx_env = report.by_env_capability(&EnvCapabilityCategory::Nx);
    assert_eq!(nx_env.len(), 1, "nx-env should fire at monorepo root");
    match &nx_env[0].data {
        DetectionData::NxEnv(info) => {
            // targetDefaults: build, test  |  project.json: build, serve, test (frontend + backend)
            assert!(info.targets.contains(&"build".to_string()));
            assert!(info.targets.contains(&"test".to_string()));
            assert!(
                info.targets.contains(&"serve".to_string()),
                "serve found in frontend/project.json"
            );
            assert!(!info.commands.is_empty(), "nx-env must produce commands");
            // Workspace children must NOT have nx-env
        }
        _ => panic!("Expected NxEnv data"),
    }

    // Should have 2 workspace children: frontend and backend
    assert_eq!(report.children.len(), 2, "Should have 2 workspace children");

    let children_names: Vec<String> = report
        .children
        .iter()
        .map(|c| {
            c.path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    eprintln!("Workspace children: {:?}", children_names);

    // Find frontend (Node.js) workspace
    let frontend = report
        .children
        .iter()
        .find(|c| c.path.ends_with("apps/frontend"))
        .expect("Should have apps/frontend child");

    let frontend_lang = frontend.by_app_type(&AppTypeCategory::Language);
    assert_eq!(
        frontend_lang[0].strategy_id, "nodejs",
        "Frontend workspace should detect Node.js"
    );

    let frontend_docker = frontend.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(
        frontend_docker.len(),
        1,
        "Frontend workspace should detect Docker"
    );

    // Find backend (Rust) workspace
    let backend = report
        .children
        .iter()
        .find(|c| c.path.ends_with("apps/backend"))
        .expect("Should have apps/backend child");

    let backend_lang = backend.by_app_type(&AppTypeCategory::Language);
    assert_eq!(
        backend_lang[0].strategy_id, "rust",
        "Backend workspace should detect Rust"
    );

    let backend_docker = backend.by_env_capability(&EnvCapabilityCategory::Docker);
    assert!(
        backend_docker.is_empty(),
        "Backend workspace has no Dockerfile → no Docker env"
    );

    let backend_local = backend.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(
        backend_local.len(),
        1,
        "Backend workspace should have local-env"
    );
    match &backend_local[0].data {
        DetectionData::LocalEnv(info) => {
            assert!(
                info.commands.contains_key("build"),
                "Rust: should have cargo build"
            );
            assert!(
                info.commands.contains_key("run"),
                "Rust: should have cargo run"
            );
            assert!(
                info.commands.contains_key("start"),
                "Rust binary: should have cargo run --release"
            );
        }
        _ => panic!("Expected LocalEnv"),
    }
}

// ─── Confidence and structural validity ───────────────────────────────────────

/// Every result from every fixture must have confidence in [0.0, 1.0]
/// and a non-empty strategy_id.
#[test]
fn test_all_fixtures_structural_validity() {
    let fixtures = [
        "nodejs-docker",
        "rust-project",
        "nodejs-react",
        "python-app",
        "nx-monorepo",
        "redis-service",
        "traefik-proxy",
        "k8s-app",
        "rust-binary",
        "rust-docker",
        "python-poetry",
        "python-docker-compose",
        "nodejs-docker-compose",
        "nodejs-full",
        "nodejs-no-scripts",
        "docker-only",
        "nx-with-root-compose",
        "nx-mixed-workspaces",
    ];

    for name in fixtures {
        let path = fixture(name);
        let report = engine()
            .detect(&path)
            .unwrap_or_else(|e| panic!("Detection failed for {name}: {e}"));

        for r in report.all_reports() {
            for result in &r.results {
                assert!(
                    !result.strategy_id.is_empty(),
                    "[{}] strategy_id must not be empty",
                    name
                );
                assert!(
                    result.confidence >= 0.0 && result.confidence <= 1.0,
                    "[{}] confidence {} out of range for strategy '{}'",
                    name,
                    result.confidence,
                    result.strategy_id
                );
            }
        }

        eprintln!(
            "[{}] ✓ {} result(s) across {} report(s)",
            name,
            report
                .all_reports()
                .iter()
                .map(|r| r.results.len())
                .sum::<usize>(),
            report.all_reports().len()
        );
    }
}

/// No fixture should return an env capability without any commands
/// (that would be useless output)
#[test]
fn test_env_capabilities_always_have_commands() {
    let fixtures = [
        "nodejs-docker",
        "python-app",
        "rust-docker",
        "python-docker-compose",
        "nodejs-docker-compose",
        "nodejs-full",
        "docker-only",
        "nx-with-root-compose",
        "nx-monorepo",
        "nx-mixed-workspaces",
    ];

    for name in fixtures {
        let report = engine()
            .detect(fixture(name))
            .unwrap_or_else(|e| panic!("Detection failed for {name}: {e}"));

        for r in report.all_reports() {
            for result in &r.results {
                if !result.category.is_env_capability() {
                    continue;
                }

                let cmd_count = match &result.data {
                    DetectionData::LocalEnv(i) => i.commands.len(),
                    DetectionData::DockerEnv(i) => i.commands.len(),
                    DetectionData::OrbStackEnv(i) => i.commands.len(),
                    DetectionData::KubernetesEnv(i) => i.commands.len(),
                    DetectionData::NxEnv(i) => i.commands.len(),
                    _ => continue,
                };

                // local-env on a no-scripts Node project is legitimately 0 — skip that case
                let is_empty_scripts =
                    name == "nodejs-no-scripts" && result.strategy_id == "local-env";

                if !is_empty_scripts {
                    assert!(
                        cmd_count > 0,
                        "[{}] strategy '{}' has 0 commands — useless env capability",
                        name,
                        result.strategy_id
                    );
                }
            }
        }
        eprintln!("[{}] ✓ all env capabilities have commands", name);
    }
}
