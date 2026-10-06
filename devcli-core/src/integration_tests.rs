// Integration tests for devcli
// Tests complete workflows from detection to execution

#[cfg(test)]
mod tests {
    use crate::config::*;
    use crate::detection::*;
    use crate::process_manager_support::process_id;
    use crate::test_utils::AppBuilder;
    use process_manager::state::{ManagedProcess, ProcessRuntime, StateStore};
    use process_manager::{HealthCheck, RestartPolicy, Task};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
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
            serde_json::to_string_pretty(&pkg).unwrap(),
        )
        .unwrap();

        // Create a dummy server.js
        fs::write(
            dir.path().join("server.js"),
            "console.log('Server started');\n",
        )
        .unwrap();

        dir
    }

    // Helper: Create a test config
    fn create_test_config_with_app(app_name: &str, app_path: &str) -> Config {
        let mut projects = HashMap::new();
        let mut apps = HashMap::new();

        apps.insert(
            app_name.to_string(),
            AppBuilder::new("nodejs", app_path)
                .with_local_command("start", "echo 'Starting app'")
                .with_local_command("test", "echo 'Running tests'")
                .with_local_default("start")
                .build(),
        );

        projects.insert(
            "test-project".to_string(),
            Project {
                apps,
                alternative_name: None,
            },
        );

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
        let resolved_with_project =
            get_app_by_project(&config, "test-project", "test-app").unwrap();
        assert_eq!(resolved_with_project.app_name, "test-app");
    }

    // Test: Process manager state workflow (isolated temp store)
    #[test]
    fn test_integration_process_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();
        let current_pid = std::process::id();
        let id = process_id("test-project", "integration-test-app", "local");

        let task = Task {
            id: id.clone(),
            command: "test command".to_string(),
            args: vec![],
            working_dir: PathBuf::from("/tmp"),
            env: HashMap::new(),
            is_detached: true,
            log_file: None,
            health_check: HealthCheck::Process {},
            restart_policy: RestartPolicy {
                enabled: false,
                ..RestartPolicy::default()
            },
        };

        let mut metadata = HashMap::new();
        metadata.insert("project".to_string(), "test-project".to_string());
        metadata.insert(
            "app_config_name".to_string(),
            "integration-test-app".to_string(),
        );
        metadata.insert("environment".to_string(), "local".to_string());
        metadata.insert("command_variant".to_string(), "start".to_string());

        let process = ManagedProcess {
            id: id.clone(),
            pid: current_pid,
            pgid: None,
            task,
            start_time: chrono::Utc::now(),
            metadata,
            runtime: ProcessRuntime::default(),
        };

        store.save(&process).unwrap();

        let retrieved = store.load(&id).unwrap().expect("process should exist");
        assert_eq!(
            retrieved
                .metadata
                .get("app_config_name")
                .map(String::as_str),
            Some("integration-test-app")
        );
        assert_eq!(retrieved.pid, current_pid);
        assert!(store.is_running(&retrieved));

        let processes = store.list().unwrap();
        assert!(processes.iter().any(|p| p.id == id));

        store.delete(&id).unwrap();
        assert!(store.load(&id).unwrap().is_none());
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
            AppBuilder::new("redis", "/tmp/db")
                .with_local_command("start", "redis-server")
                .with_local_default("start")
                .build(),
        );

        // Create api (depends on database)
        apps.insert(
            "api".to_string(),
            AppBuilder::new("nodejs", "/tmp/api")
                .with_local_command("start", "npm start")
                .with_local_default("start")
                .with_dependency("test", "database")
                .build(),
        );

        // Create frontend (depends on api)
        apps.insert(
            "frontend".to_string(),
            AppBuilder::new("nodejs", "/tmp/frontend")
                .with_local_command("start", "npm start")
                .with_local_default("start")
                .with_dependency("test", "api")
                .build(),
        );

        config.projects.insert(
            "test".to_string(),
            Project {
                apps,
                alternative_name: None,
            },
        );

        // Step 1: Resolve frontend app
        let frontend = resolve_app(&config, "frontend", None).unwrap();

        // Step 2: Resolve dependency chain
        let deps =
            crate::config::dependencies::resolve_dependency_chain(&config, &frontend).unwrap();

        // Step 3: Verify chain includes both dependencies
        assert_eq!(deps.len(), 2);

        let dep_names: Vec<&str> = deps.iter().map(|d| d.app_name.as_str()).collect();
        assert!(dep_names.contains(&"database"));
        assert!(dep_names.contains(&"api"));
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
            serde_json::to_string_pretty(&pkg).unwrap(),
        )
        .unwrap();

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
"#,
        )
        .unwrap();

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
            docker_platform: "linux/amd64".to_string(),
            default_stage: None,
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
                    AppBuilder::new("nodejs", &format!("/tmp/app-{}-{}", i, j))
                        .with_local_command("start", "npm start")
                        .with_local_default("start")
                        .build(),
                );
            }
            config.projects.insert(
                format!("project-{}", i),
                Project {
                    apps,
                    alternative_name: None,
                },
            );
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
            serde_json::to_string_pretty(&pkg).unwrap(),
        )
        .unwrap();

        // Create k8s directory with manifests
        fs::create_dir(dir.path().join("k8s")).unwrap();
        fs::write(
            dir.path().join("k8s/deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("k8s/service.yaml"),
            "apiVersion: v1\nkind: Service\n",
        )
        .unwrap();

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

    // === Metrics Integration Tests ===

    #[test]
    fn test_integration_metrics_collector_basic_operations() {
        use crate::metrics::{MetricsCollector, OperationTiming};
        use chrono::Utc;

        // Step 1: Create metrics collector
        let collector = MetricsCollector::new();

        // Step 2: Record some operations
        for i in 0..5 {
            collector.record_operation(OperationTiming {
                operation: "start".to_string(),
                app: format!("app-{}", i),
                duration_ms: 100 + i * 10,
                timestamp: Utc::now(),
                success: true,
            });
        }

        // Step 3: Record exit codes
        collector.record_exit_code(0);
        collector.record_exit_code(0);
        collector.record_exit_code(1);

        // Step 4: Increment loop iterations
        for _ in 0..10 {
            collector.increment_loop_iteration();
        }

        // Verify operations work without panicking
        // (Actual verification would require exposing internal state or collect_all())
    }

    #[test]
    fn test_integration_metrics_json_serialization() {
        use crate::metrics::{
            AllMetrics, AppMetrics, PerformanceMetrics, ProcessMetrics, SystemMetrics,
        };
        use chrono::Utc;
        use std::collections::HashMap;

        // Step 1: Create sample metrics
        let metrics = AllMetrics {
            processes: ProcessMetrics {
                total_processes: 5,
                running_processes: 3,
                stopped_processes: 2,
                total_restarts: 10,
                restarts_last_hour: 2,
                health_check_success_rate: 0.95,
                exit_code_distribution: {
                    let mut map = HashMap::new();
                    map.insert(0, 8);
                    map.insert(1, 2);
                    map
                },
                apps: vec![
                    AppMetrics {
                        project: "test-project".to_string(),
                        name: "api-server".to_string(),
                        status: "running".to_string(),
                        uptime_seconds: Some(3600),
                        restart_count: 2,
                        last_exit_code: None,
                        health_status: "healthy".to_string(),
                        health_check_failures: 0,
                    },
                    AppMetrics {
                        project: "test-project".to_string(),
                        name: "worker".to_string(),
                        status: "stopped".to_string(),
                        uptime_seconds: None,
                        restart_count: 0,
                        last_exit_code: Some(0),
                        health_status: "unknown".to_string(),
                        health_check_failures: 0,
                    },
                ],
            },
            system: SystemMetrics {
                monitor_uptime_seconds: 3600,
                monitor_loop_iterations: 1200,
                devcli_version: "0.1.0".to_string(),
                total_apps_configured: 10,
                config_last_modified: None,
            },
            performance: PerformanceMetrics {
                avg_startup_time_ms: 500.0,
                avg_health_check_duration_ms: 50.0,
                avg_restart_duration_ms: 1000.0,
                recent_operations: vec![],
            },
            timestamp: Utc::now(),
        };

        // Step 2: Serialize to JSON
        let json = serde_json::to_string_pretty(&metrics).unwrap();

        // Step 3: Verify JSON contains expected fields
        assert!(json.contains("total_processes"));
        assert!(json.contains("monitor_uptime_seconds"));
        assert!(json.contains("avg_startup_time_ms"));
        assert!(json.contains("api-server"));
        assert!(json.contains("exit_code_distribution"));

        // Step 4: Deserialize back
        let deserialized: AllMetrics = serde_json::from_str(&json).unwrap();

        // Step 5: Verify critical fields match
        assert_eq!(deserialized.processes.total_processes, 5);
        assert_eq!(deserialized.processes.running_processes, 3);
        assert_eq!(deserialized.system.monitor_loop_iterations, 1200);
        assert_eq!(deserialized.processes.apps.len(), 2);
        assert_eq!(deserialized.processes.apps[0].name, "api-server");
    }

    #[test]
    fn test_integration_metrics_operation_timing_limit() {
        use crate::metrics::{MetricsCollector, OperationTiming};
        use chrono::Utc;

        let collector = MetricsCollector::new();

        // Step 1: Record more than 1000 operations
        for i in 0..1200 {
            collector.record_operation(OperationTiming {
                operation: "test".to_string(),
                app: format!("app-{}", i),
                duration_ms: 100,
                timestamp: Utc::now(),
                success: true,
            });
        }

        // The collector should have pruned operations to keep memory bounded
        // (Verification would require exposing internal state, but we test it doesn't panic)
    }

    #[test]
    fn test_integration_metrics_exit_code_distribution() {
        use crate::metrics::MetricsCollector;

        let collector = MetricsCollector::new();

        // Step 1: Record various exit codes
        for _ in 0..10 {
            collector.record_exit_code(0); // Success
        }
        for _ in 0..3 {
            collector.record_exit_code(1); // Error
        }
        for _ in 0..2 {
            collector.record_exit_code(137); // SIGKILL
        }

        // Exit code distribution should be tracked
        // (Actual verification would require collect_all() with deterministic state snapshots)
    }

    #[test]
    fn test_integration_metrics_types_defaults() {
        use crate::metrics::{AppMetrics, ProcessMetrics, SystemMetrics};
        use std::collections::HashMap;

        // Step 1: Create metrics with minimal data
        let app = AppMetrics {
            project: "test".to_string(),
            name: "app".to_string(),
            status: "running".to_string(),
            uptime_seconds: None,
            restart_count: 0,
            last_exit_code: None,
            health_status: "unknown".to_string(),
            health_check_failures: 0,
        };

        // Step 2: Verify optional fields work
        assert!(app.uptime_seconds.is_none());
        assert!(app.last_exit_code.is_none());

        // Step 3: Create process metrics with empty collections
        let process_metrics = ProcessMetrics {
            total_processes: 0,
            running_processes: 0,
            stopped_processes: 0,
            total_restarts: 0,
            restarts_last_hour: 0,
            health_check_success_rate: 1.0,
            exit_code_distribution: HashMap::new(),
            apps: vec![],
        };

        // Step 4: Verify empty state is valid
        assert_eq!(process_metrics.total_processes, 0);
        assert!(process_metrics.exit_code_distribution.is_empty());
        assert!(process_metrics.apps.is_empty());

        // Step 5: Create system metrics
        let system_metrics = SystemMetrics {
            monitor_uptime_seconds: 0,
            monitor_loop_iterations: 0,
            devcli_version: env!("CARGO_PKG_VERSION").to_string(),
            total_apps_configured: 0,
            config_last_modified: None,
        };

        // Step 6: Verify system metrics structure
        assert!(!system_metrics.devcli_version.is_empty());
    }

    #[test]
    fn test_integration_metrics_performance_averages() {
        use crate::metrics::{OperationTiming, PerformanceMetrics};
        use chrono::Utc;

        // Step 1: Create performance metrics with operations
        let operations = vec![
            OperationTiming {
                operation: "start".to_string(),
                app: "app1".to_string(),
                duration_ms: 100,
                timestamp: Utc::now(),
                success: true,
            },
            OperationTiming {
                operation: "start".to_string(),
                app: "app2".to_string(),
                duration_ms: 200,
                timestamp: Utc::now(),
                success: true,
            },
            OperationTiming {
                operation: "health_check".to_string(),
                app: "app1".to_string(),
                duration_ms: 50,
                timestamp: Utc::now(),
                success: true,
            },
        ];

        let metrics = PerformanceMetrics {
            avg_startup_time_ms: 150.0, // Average of 100 and 200
            avg_health_check_duration_ms: 50.0,
            avg_restart_duration_ms: 0.0,
            recent_operations: operations.clone(),
        };

        // Step 2: Verify structure
        assert_eq!(metrics.recent_operations.len(), 3);
        assert!(metrics.avg_startup_time_ms > 0.0);

        // Step 3: Serialize and deserialize
        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: PerformanceMetrics = serde_json::from_str(&json).unwrap();

        // Step 4: Verify values preserved
        assert_eq!(deserialized.avg_startup_time_ms, 150.0);
        assert_eq!(deserialized.recent_operations.len(), 3);
    }

    #[cfg(unix)]
    mod monitor_restart {
        use super::*;
        use process_manager::engine;
        use std::path::{Path, PathBuf};
        use std::time::Duration;

        fn locate_pm_daemon_dir() -> Option<PathBuf> {
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace_root = manifest.parent()?;
            for profile in ["debug", "release"] {
                let candidate = workspace_root
                    .join("target")
                    .join(profile)
                    .join("pm-daemon");
                if candidate.exists() {
                    return candidate.parent().map(Path::to_path_buf);
                }
            }
            None
        }

        #[tokio::test]
        #[ignore = "requires a built pm-daemon binary and real process supervision; timing-sensitive on CI (run with --ignored)"]
        async fn test_monitor_restarts_crashed_process() {
            let Some(daemon_dir) = locate_pm_daemon_dir() else {
                eprintln!("Skipping monitor restart test: pm-daemon binary not built yet");
                return;
            };

            let temp_dir = TempDir::new().unwrap();
            let store = StateStore::new(temp_dir.path().to_path_buf()).unwrap();
            let id = process_id("test-project", "monitor-restart-app", "local");

            let task = Task {
                id: id.clone(),
                command: "sleep 120".to_string(),
                args: vec![],
                working_dir: PathBuf::from("/tmp"),
                env: HashMap::new(),
                is_detached: true,
                log_file: None,
                health_check: HealthCheck::Process {},
                restart_policy: RestartPolicy::default(),
            };

            let running = engine::spawn(&task).await.expect("spawn sleep");
            let original_pid = running.pid;
            let pgid = running.pgid;
            drop(running.output_rx);

            let mut metadata = HashMap::new();
            metadata.insert("project".to_string(), "test-project".to_string());
            metadata.insert(
                "app_config_name".to_string(),
                "monitor-restart-app".to_string(),
            );
            metadata.insert("environment".to_string(), "local".to_string());

            let process = ManagedProcess {
                id: id.clone(),
                pid: original_pid,
                pgid,
                task: task.clone(),
                start_time: chrono::Utc::now(),
                metadata,
                runtime: ProcessRuntime::default(),
            };
            store.save(&process).unwrap();

            let original_path = std::env::var("PATH").unwrap_or_default();
            std::env::set_var(
                "PATH",
                format!("{}:{}", daemon_dir.display(), original_path),
            );
            store.ensure_daemon_running().expect("start pm-daemon");

            engine::terminate(original_pid, pgid, true)
                .await
                .expect("kill original process");

            let mut restarted = false;
            for _ in 0..20 {
                tokio::time::sleep(Duration::from_millis(500)).await;
                if let Some(updated) = store.load(&id).unwrap() {
                    if updated.pid != original_pid && store.is_running(&updated) {
                        restarted = true;
                        let _ = engine::terminate(updated.pid, updated.pgid, true).await;
                        break;
                    }
                }
            }

            store.delete(&id).unwrap();
            std::env::set_var("PATH", original_path);

            assert!(
                restarted,
                "pm-daemon should restart the crashed process with a new PID"
            );
        }
    }
}
