//! Integration tests with real project fixtures

use app_detector::{
    engine::DetectionEngine,
    registry::StrategyRegistry,
    types::{AppTypeCategory, EnvCapabilityCategory},
};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn test_detect_nodejs_docker_project() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("nodejs-docker");
    let report = engine.detect(&path).expect("Detection failed");

    // Should detect both Node.js and Docker
    assert!(!report.by_app_type(&AppTypeCategory::Language).is_empty());
    assert!(!report
        .by_env_capability(&EnvCapabilityCategory::Docker)
        .is_empty());

    // Check Node.js detection
    let nodejs = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(nodejs.len(), 1);
    assert_eq!(nodejs[0].strategy_id, "nodejs");

    // Check Docker detection
    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(docker.len(), 1);
    assert_eq!(docker[0].strategy_id, "docker");

    // Verify detection data
    use app_detector::types::DetectionData;
    match &nodejs[0].data {
        DetectionData::Language(info) => {
            assert_eq!(info.name, "Node.js");
            assert!(info.metadata.contains_key("package_name"));
            assert_eq!(
                info.metadata.get("package_name").unwrap(),
                &serde_json::json!("test-nodejs-app")
            );
        }
        _ => panic!("Expected Language data"),
    }

    match &docker[0].data {
        DetectionData::DockerEnv(info) => {
            assert_eq!(info.dockerfiles.len(), 1);
            assert_eq!(info.stages.len(), 2);
            assert!(info.stages.contains(&"builder".to_string()));
            assert!(info.stages.contains(&"production".to_string()));
            assert!(info.base_images.contains(&"node:18".to_string()));
            assert!(info.base_images.contains(&"node:18-alpine".to_string()));
            assert!(info.exposed_ports.contains(&3000));
        }
        _ => panic!("Expected DockerEnv data"),
    }
}

#[test]
fn test_detect_rust_project() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("rust-project");
    let report = engine.detect(&path).expect("Detection failed");

    // Should detect Rust language
    assert!(!report.by_app_type(&AppTypeCategory::Language).is_empty());

    let rust = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(rust.len(), 1);
    assert_eq!(rust[0].strategy_id, "rust");

    // Verify detection data
    use app_detector::types::DetectionData;
    match &rust[0].data {
        DetectionData::Language(info) => {
            assert_eq!(info.name, "Rust");
            assert!(info.metadata.contains_key("package_name"));
            assert_eq!(
                info.metadata.get("package_name").unwrap(),
                &serde_json::json!("test-rust-app")
            );
            assert_eq!(
                info.metadata.get("edition").unwrap(),
                &serde_json::json!("2021")
            );
        }
        _ => panic!("Expected Language data"),
    }
}

#[test]
fn test_detect_nodejs_react_project() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("nodejs-react");
    let report = engine.detect(&path).expect("Detection failed");

    // Should detect Node.js language
    assert!(!report.by_app_type(&AppTypeCategory::Language).is_empty());

    let nodejs = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(nodejs.len(), 1);
    assert_eq!(nodejs[0].strategy_id, "nodejs");

    // Should suggest React framework
    assert!(nodejs[0]
        .suggested_strategies
        .contains(&"react".to_string()));

    // Verify TypeScript detection
    use app_detector::types::DetectionData;
    match &nodejs[0].data {
        DetectionData::Language(info) => {
            assert_eq!(info.name, "Node.js");
            assert!(info.metadata.contains_key("typescript"));
            assert_eq!(
                info.metadata.get("typescript").unwrap(),
                &serde_json::json!(true)
            );
        }
        _ => panic!("Expected Language data"),
    }
}

#[test]
fn test_detect_nonexistent_directory() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = PathBuf::from("/nonexistent/path");
    let result = engine.detect(&path);

    assert!(result.is_err());
}

#[test]
fn test_detect_empty_directory() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let report = engine.detect(temp_dir.path()).expect("Detection failed");

    // Should return empty report
    assert!(report.results.is_empty());
}

// === New Comprehensive Tests ===

#[test]
fn test_detect_python_project() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("python-app");
    let report = engine.detect(&path).expect("Detection failed");

    // Should detect Python language
    let python = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(python.len(), 1);
    assert_eq!(python[0].strategy_id, "python");

    // Should suggest Flask framework
    assert!(python[0]
        .suggested_strategies
        .contains(&"flask".to_string()));

    // Should detect Docker environment
    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(docker.len(), 1);

    // Should detect Local environment
    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(local.len(), 1);

    use app_detector::types::DetectionData;
    match &local[0].data {
        DetectionData::LocalEnv(info) => {
            assert!(info.commands.contains_key("start"));
            assert!(info.commands.contains_key("install"));
        }
        _ => panic!("Expected LocalEnv data"),
    }
}

#[test]
fn test_detect_nx_monorepo() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("nx-monorepo");
    let report = engine.detect(&path).expect("Detection failed");

    // Should detect Nx monorepo
    let nx = report.by_app_type(&AppTypeCategory::Monorepo);
    assert_eq!(nx.len(), 1);
    assert_eq!(nx[0].strategy_id, "nx");

    // Node.js should NOT be detected at root level (Nx suppresses it)
    // Node.js apps exist within the monorepo workspaces
    let nodejs = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(
        nodejs.len(),
        0,
        "Node.js should be suppressed by Nx at root level"
    );

    // Should detect Local environment
    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(local.len(), 1);

    use app_detector::types::DetectionData;
    match &nx[0].data {
        DetectionData::Monorepo(info) => {
            assert_eq!(info.tool, "nx");
            assert!(info.workspaces.len() >= 2); // web and api apps
        }
        _ => panic!("Expected Monorepo data"),
    }
}

#[test]
fn test_detect_redis_service() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("redis-service");
    let report = engine.detect(&path).expect("Detection failed");

    // Should detect Redis service
    let redis = report.by_app_type(&AppTypeCategory::Service);
    assert_eq!(redis.len(), 1);
    assert_eq!(redis[0].strategy_id, "redis");

    // Should detect Docker environment (via docker-compose)
    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(docker.len(), 1);

    // Should detect Local environment
    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(local.len(), 1);

    use app_detector::types::DetectionData;
    match &redis[0].data {
        DetectionData::Service(info) => {
            assert_eq!(info.name, "Redis");
            assert!(info.config_files.contains(&PathBuf::from("redis.conf")));
        }
        _ => panic!("Expected Service data"),
    }
}

#[test]
fn test_detect_traefik_proxy() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("traefik-proxy");
    let report = engine.detect(&path).expect("Detection failed");

    // Should detect Traefik reverse proxy
    let traefik = report.by_app_type(&AppTypeCategory::ReverseProxy);
    assert_eq!(traefik.len(), 1);
    assert_eq!(traefik[0].strategy_id, "traefik");

    // Should detect Docker environment
    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(docker.len(), 1);

    // Should detect Local environment
    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(local.len(), 1);

    use app_detector::types::DetectionData;
    match &traefik[0].data {
        DetectionData::Service(info) => {
            assert_eq!(info.name, "Traefik");
            assert!(info.config_files.contains(&PathBuf::from("traefik.yml")));
        }
        _ => panic!("Expected Service data"),
    }
}

#[test]
fn test_detect_k8s_app_multi_environment() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("k8s-app");
    let report = engine.detect(&path).expect("Detection failed");

    // Should detect Node.js language
    let nodejs = report.by_app_type(&AppTypeCategory::Language);
    assert_eq!(nodejs.len(), 1);
    assert_eq!(nodejs[0].strategy_id, "nodejs");

    // Should detect multiple environments: Docker, OrbStack, Kubernetes, Local
    let env_caps = report.env_capabilities();
    eprintln!("Detected {} env capabilities:", env_caps.len());
    for cap in &env_caps {
        eprintln!("  - {}", cap.strategy_id);
    }
    assert_eq!(
        env_caps.len(),
        4,
        "Expected 4 env capabilities: docker, orbstack-env, kubernetes-env, local-env"
    );

    // Check Docker
    let docker = report.by_env_capability(&EnvCapabilityCategory::Docker);
    assert_eq!(docker.len(), 1);

    // Check OrbStack
    let orbstack = report.by_env_capability(&EnvCapabilityCategory::OrbStack);
    assert_eq!(orbstack.len(), 1);

    // Compare Docker and OrbStack outputs
    use app_detector::types::DetectionData;
    let docker_data = match &docker[0].data {
        DetectionData::DockerEnv(info) => info,
        _ => panic!("Expected DockerEnv data"),
    };
    let orbstack_data = match &orbstack[0].data {
        DetectionData::OrbStackEnv(info) => info,
        _ => panic!("Expected OrbStackEnv data"),
    };

    eprintln!("\n=== DOCKER ===");
    eprintln!(
        "Commands: {:?}",
        docker_data.commands.keys().collect::<Vec<_>>()
    );
    for (k, v) in &docker_data.commands {
        eprintln!("  {}: {}", k, v);
    }
    eprintln!("Metadata: {:?}", docker_data.metadata);

    eprintln!("\n=== ORBSTACK ===");
    eprintln!(
        "Commands: {:?}",
        orbstack_data.commands.keys().collect::<Vec<_>>()
    );
    for (k, v) in &orbstack_data.commands {
        eprintln!("  {}: {}", k, v);
    }
    eprintln!("Metadata: {:?}", orbstack_data.metadata);

    // OrbStack should mirror Docker with `--context orbstack`.
    assert_eq!(
        docker_data.commands.len(),
        orbstack_data.commands.len(),
        "Docker and OrbStack should have the same number of commands"
    );
    for (key, docker_cmd) in &docker_data.commands {
        let expected = if let Some(rest) = docker_cmd.strip_prefix("docker ") {
            format!("docker --context orbstack {rest}")
        } else {
            docker_cmd.to_string()
        };
        assert_eq!(
            orbstack_data.commands.get(key),
            Some(&expected),
            "OrbStack should have the same '{}' command as Docker (with context)",
            key
        );
    }

    // Check Kubernetes
    let k8s = report.by_env_capability(&EnvCapabilityCategory::Kubernetes);
    assert_eq!(k8s.len(), 1);

    match &k8s[0].data {
        DetectionData::KubernetesEnv(info) => {
            assert!(info.manifests.len() >= 2); // deployment.yaml and service.yaml
            assert!(info.commands.contains_key("apply"));
        }
        _ => panic!("Expected KubernetesEnv data"),
    }

    // Check Local
    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(local.len(), 1);
}

#[test]
fn test_two_phase_detection_order() {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let path = fixture_path("nodejs-docker");
    let report = engine.detect(&path).expect("Detection failed");

    // All app types should be detected before environment capabilities
    let app_types = report.app_types();
    let env_caps = report.env_capabilities();

    assert!(!app_types.is_empty());
    assert!(!env_caps.is_empty());

    // LocalEnv should be able to access nodejs results
    let local = report.by_env_capability(&EnvCapabilityCategory::Local);
    assert_eq!(local.len(), 1);

    use app_detector::types::DetectionData;
    match &local[0].data {
        DetectionData::LocalEnv(info) => {
            // Should have extracted npm scripts from Node.js app
            assert!(info.commands.len() >= 3);
            assert!(info.commands.contains_key("start"));
            assert_eq!(
                info.metadata.get("app_type").unwrap(),
                &serde_json::json!("nodejs")
            );
        }
        _ => panic!("Expected LocalEnv data"),
    }
}
