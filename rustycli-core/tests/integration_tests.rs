// Integration tests for RustyCLI
// Tests complete workflows from detection to execution

use rustycli_core::config::*;
use rustycli_core::detection::*;
use rustycli_core::process::*;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

// Helper: Create a test Node.js project
fn create_test_nodejs_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    
    // Create package.json
    let pkg = serde_json::json!({
        "name": "test-integration-app",
        "version": "1.0.0",
        "scripts": {
            "start": "node server.js",
            "test": "jest",
            "build": "webpack"
        }
    });
    
    fs::write(
        dir.path().join("package.json"),
        serde_json::to_string_pretty(&pkg).unwrap()
    ).unwrap();
    
    // Create a dummy server.js
    fs::write(
        dir.path().join("server.js"),
        "console.log('Server started');\n"
    ).unwrap();
    
    dir
}

// Helper: Create a test config
fn create_test_config_with_app(app_name: &str, app_path: &str) -> Config {
    let mut projects = HashMap::new();
    let mut apps = HashMap::new();
    
    apps.insert(
        app_name.to_string(),
        App {
            app_type: "nodejs".to_string(),
            path: app_path.to_string(),
            commands: Commands {
                local: Some({
                    let mut cmds = HashMap::new();
                    cmds.insert("start".to_string(), "echo 'Starting app'".to_string());
                    cmds.insert("test".to_string(), "echo 'Running tests'".to_string());
                    cmds
                }),
                docker: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                k8s: None,
            },
        },
    );
    
    projects.insert("test-project".to_string(), Project { apps });
    
    Config { projects }
}

// Test: Full detection workflow
#[test]
fn test_integration_detect_and_configure() {
    let project = create_test_nodejs_project();
    
    // Step 1: Detect app
    let detected = detect_app(project.path()).unwrap();
    
    // Step 2: Verify detection results
    assert_eq!(detected.app_type, "nodejs");
    assert_eq!(detected.app_name, "test-integration-app");
    assert!(detected.local_commands.is_some());
    
    // Step 3: Verify commands were detected
    let local_cmds = detected.local_commands.unwrap();
    assert!(local_cmds.contains_key("start"));
    assert!(local_cmds.contains_key("test"));
    assert!(local_cmds.contains_key("build"));
    
    // Step 4: Verify default command suggestion
    assert!(detected.suggested_local_default.is_some());
    assert_eq!(detected.suggested_local_default.unwrap(), "start");
}

// Test: Config resolution workflow
#[test]
fn test_integration_config_resolution() {
    let project = create_test_nodejs_project();
    let config = create_test_config_with_app("test-app", project.path().to_str().unwrap());
    
    // Step 1: Resolve app by name
    let resolved = resolve_app(&config, "test-app", None).unwrap();
    
    // Step 2: Verify resolution
    assert_eq!(resolved.app_name, "test-app");
    assert_eq!(resolved.project, "test-project");
    assert_eq!(resolved.app.app_type, "nodejs");
    
    // Step 3: Get app with project specified
    let resolved_with_project = get_app_by_project(&config, "test-project", "test-app").unwrap();
    assert_eq!(resolved_with_project.app_name, "test-app");
}

// Test: Process tracker workflow
#[test]
fn test_integration_process_tracking() {
    let tracker = ProcessTracker::new().unwrap();
    let current_pid = std::process::id();
    
    // Step 1: Register a test process
    let process = ProcessInfo {
        app_name: "integration-test-app".to_string(),
        pid: current_pid,
        command: "test command".to_string(),
        working_dir: "/tmp".to_string(),
        start_time: chrono::Utc::now(),
        env_vars: HashMap::new(),
        project: Some("test-project".to_string()),
        app_config_name: Some("integration-test-app".to_string()),
        environment: Some("local".to_string()),
        command_variant: Some("start".to_string()),
    };
    
    tracker.register_process(process).unwrap();
    
    // Step 2: Verify process is tracked
    let retrieved = tracker.get_process("integration-test-app").unwrap();
    assert!(retrieved.is_some());
    
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.app_name, "integration-test-app");
    assert_eq!(retrieved.pid, current_pid);
    
    // Step 3: Verify process is running
    assert!(tracker.is_running(current_pid));
    
    // Step 4: List all processes (should include ours)
    let processes = tracker.list_processes().unwrap();
    let our_process = processes.iter().find(|p| p.app_name == "integration-test-app");
    assert!(our_process.is_some());
    
    // Step 5: Clean up
    tracker.remove_process("integration-test-app").unwrap();
    
    // Step 6: Verify it's removed
    let retrieved_after = tracker.get_process("integration-test-app").unwrap();
    assert!(retrieved_after.is_none());
}

// Test: Dependency chain resolution
#[test]
fn test_integration_dependency_chain() {
    let mut config = Config {
        projects: HashMap::new(),
    };
    
    let mut apps = HashMap::new();
    
    // Create database (no deps)
    apps.insert(
        "database".to_string(),
        App {
            app_type: "redis".to_string(),
            path: "/tmp/db".to_string(),
            commands: Commands {
                local: Some({
                    let mut cmds = HashMap::new();
                    cmds.insert("start".to_string(), "redis-server".to_string());
                    cmds
                }),
                docker: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                k8s: None,
            },
        },
    );
    
    // Create api (depends on database)
    apps.insert(
        "api".to_string(),
        App {
            app_type: "nodejs".to_string(),
            path: "/tmp/api".to_string(),
            commands: Commands {
                local: Some({
                    let mut cmds = HashMap::new();
                    cmds.insert("start".to_string(), "npm start".to_string());
                    cmds
                }),
                docker: None,
                k8s: None,
            },
            dependencies: vec![Dependency {
                project: "test".to_string(),
                app: "database".to_string(),
            }],
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                k8s: None,
            },
        },
    );
    
    // Create frontend (depends on api)
    apps.insert(
        "frontend".to_string(),
        App {
            app_type: "nodejs".to_string(),
            path: "/tmp/frontend".to_string(),
            commands: Commands {
                local: Some({
                    let mut cmds = HashMap::new();
                    cmds.insert("start".to_string(), "npm start".to_string());
                    cmds
                }),
                docker: None,
                k8s: None,
            },
            dependencies: vec![Dependency {
                project: "test".to_string(),
                app: "api".to_string(),
            }],
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                k8s: None,
            },
        },
    );
    
    config.projects.insert("test".to_string(), Project { apps });
    
    // Step 1: Resolve frontend app
    let frontend = resolve_app(&config, "frontend", None).unwrap();
    
    // Step 2: Resolve dependency chain
    let deps = rustycli_core::config::dependencies::resolve_dependency_chain(&config, &frontend).unwrap();
    
    // Step 3: Verify chain includes both dependencies
    assert_eq!(deps.len(), 2);
    
    let dep_names: Vec<&str> = deps.iter().map(|d| d.app_name.as_str()).collect();
    assert!(dep_names.contains(&"database"));
    assert!(dep_names.contains(&"api"));
    
    // Both dependencies should be in the chain
    // (BFS ordering isn't strictly guaranteed, but both must be present)
}

// Test: Multi-stage Docker detection workflow
#[test]
fn test_integration_multistage_docker() {
    let dir = TempDir::new().unwrap();
    
    // Create package.json
    let pkg = serde_json::json!({
        "name": "docker-stages-app",
        "scripts": {
            "start": "node server.js"
        }
    });
    fs::write(
        dir.path().join("package.json"),
        serde_json::to_string_pretty(&pkg).unwrap()
    ).unwrap();
    
    // Create multi-stage Dockerfile
    fs::write(
        dir.path().join("Dockerfile"),
        r#"FROM node:18 AS build
RUN npm install
RUN npm run build

FROM node:18 AS test
COPY --from=build /app .
RUN npm test

FROM node:18-slim AS production
COPY --from=build /app/dist .
CMD ["node", "server.js"]
"#
    ).unwrap();
    
    // Step 1: Detect app
    let detected = detect_app(dir.path()).unwrap();
    
    // Step 2: Verify Docker commands include stages
    assert!(detected.docker_commands.is_some());
    let docker_cmds = detected.docker_commands.unwrap();
    
    // Step 3: Verify all stages detected
    assert!(docker_cmds.contains_key("build"));
    assert!(docker_cmds.contains_key("test"));
    assert!(docker_cmds.contains_key("production"));
    
    // Step 4: Verify test stage has run command
    assert!(docker_cmds.contains_key("test-run"));
    
    // Step 5: Verify commands are properly formatted
    let test_cmd = docker_cmds.get("test").unwrap();
    assert!(test_cmd.contains("--target test"));
    assert!(test_cmd.contains("docker build"));
}

// Test: Config serialization and persistence
#[test]
fn test_integration_config_persistence() {
    let project = create_test_nodejs_project();
    
    // Step 1: Create config
    let config = create_test_config_with_app("persist-test", project.path().to_str().unwrap());
    
    // Step 2: Serialize config
    let json = serde_json::to_string_pretty(&config).unwrap();
    
    // Step 3: Deserialize config
    let deserialized: Config = serde_json::from_str(&json).unwrap();
    
    // Step 4: Verify structure matches
    assert_eq!(deserialized.projects.len(), 1);
    assert!(deserialized.projects.contains_key("test-project"));
    
    let project = &deserialized.projects["test-project"];
    assert!(project.apps.contains_key("persist-test"));
    
    let app = &project.apps["persist-test"];
    assert_eq!(app.app_type, "nodejs");
    assert!(app.commands.local.is_some());
}

// Test: Preferences workflow
#[test]
fn test_integration_preferences() {
    // Step 1: Create preferences with custom values
    let prefs = Preferences {
        default_env: "docker".to_string(),
        detached_mode: true,
        auto_start_deps: false,
    };
    
    // Step 2: Serialize
    let json = serde_json::to_string(&prefs).unwrap();
    
    // Step 3: Deserialize
    let deserialized: Preferences = serde_json::from_str(&json).unwrap();
    
    // Step 4: Verify all fields
    assert_eq!(deserialized.default_env, "docker");
    assert!(deserialized.detached_mode);
    assert!(!deserialized.auto_start_deps);
}

// Test: Default preferences
#[test]
fn test_integration_default_preferences() {
    let prefs = Preferences::default();
    
    // Verify new defaults
    assert_eq!(prefs.default_env, "local");
    assert!(!prefs.detached_mode); // Shows output by default now
    assert!(prefs.auto_start_deps);
}

// Test: List all apps across projects
#[test]
fn test_integration_list_all_apps() {
    let mut config = Config {
        projects: HashMap::new(),
    };
    
    // Create 3 projects with multiple apps each
    for i in 1..=3 {
        let mut apps = HashMap::new();
        for j in 1..=2 {
            apps.insert(
                format!("app-{}-{}", i, j),
                App {
                    app_type: "nodejs".to_string(),
                    path: format!("/tmp/app-{}-{}", i, j),
                    commands: Commands {
                        local: Some({
                            let mut cmds = HashMap::new();
                            cmds.insert("start".to_string(), "npm start".to_string());
                            cmds
                        }),
                        docker: None,
                        k8s: None,
                    },
                    dependencies: Vec::new(),
                    defaults: Defaults {
                        local: Some("start".to_string()),
                        docker: None,
                        k8s: None,
                    },
                },
            );
        }
        config.projects.insert(format!("project-{}", i), Project { apps });
    }
    
    // Step 1: List all apps
    let all_apps = list_all_apps(&config);
    
    // Step 2: Verify count (3 projects × 2 apps = 6 apps)
    assert_eq!(all_apps.len(), 6);
    
    // Step 3: Verify each app has correct project (tuple is: project_name, app_name, app)
    for (project_name, app_name, _app) in all_apps {
        assert!(project_name.starts_with("project-"));
        assert!(app_name.starts_with("app-"));
    }
}

// Test: K8s environment detection and configuration
#[test]
fn test_integration_k8s_environment() {
    let dir = TempDir::new().unwrap();
    
    // Create package.json
    let pkg = serde_json::json!({
        "name": "k8s-test-app",
        "scripts": {
            "start": "node server.js"
        }
    });
    fs::write(
        dir.path().join("package.json"),
        serde_json::to_string_pretty(&pkg).unwrap()
    ).unwrap();
    
    // Create k8s directory with manifests
    fs::create_dir(dir.path().join("k8s")).unwrap();
    fs::write(
        dir.path().join("k8s/deployment.yaml"),
        "apiVersion: apps/v1\nkind: Deployment\n"
    ).unwrap();
    fs::write(
        dir.path().join("k8s/service.yaml"),
        "apiVersion: v1\nkind: Service\n"
    ).unwrap();
    
    // Step 1: Detect app
    let detected = detect_app(dir.path()).unwrap();
    
    // Step 2: Verify k8s commands detected
    assert!(detected.k8s_commands.is_some());
    
    let k8s_cmds = detected.k8s_commands.unwrap();
    
    // Step 3: Verify standard k8s commands
    assert!(k8s_cmds.contains_key("apply"));
    assert!(k8s_cmds.contains_key("delete"));
    assert!(k8s_cmds.contains_key("restart"));
    
    // Step 4: Verify commands reference k8s directory
    let apply_cmd = k8s_cmds.get("apply").unwrap();
    assert!(apply_cmd.contains("kubectl"));
    assert!(apply_cmd.contains("k8s/"));
}

