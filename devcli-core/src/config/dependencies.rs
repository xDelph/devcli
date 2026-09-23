// Dependency resolution and checking
// Ensures apps start in the correct order and dependencies are running.
//
// Transitive chain resolution goes through config_manager_support
// (config-manager DependencyGraph). Runtime "is running?" checks stay here.

use super::models::Config;
use super::resolver::ResolvedApp;
use process_manager::StateStore;
use crate::Result;

/// Remove duplicate dependencies, keyed by `project/app` (not app name alone).
pub fn dedup_dependencies(dependencies: &mut Vec<ResolvedApp>) {
    dependencies.sort_by(|a, b| (&a.project, &a.app_name).cmp(&(&b.project, &b.app_name)));
    dependencies.dedup_by(|a, b| a.project == b.project && a.app_name == b.app_name);
}

/// Resolve the full dependency chain for an app (dependencies-first, root excluded).
///
/// Example: Worker → API → Redis returns `[Redis, API]`.
/// Circular dependencies return an error.
pub fn resolve_dependency_chain(
    config: &Config,
    resolved_app: &ResolvedApp,
) -> Result<Vec<ResolvedApp>> {
    crate::config_manager_support::resolve_dependency_chain(config, resolved_app)
}

// Check which dependencies are NOT currently running.
// Returns a list of missing dependencies in "project/app" format.
//
// A dependency is satisfied when it is running in **any** environment (local, docker, …).
// This is intentional: e.g. MongoDB in Docker can satisfy a local Node app that connects to it.
pub fn check_dependencies_running(
    store: &StateStore,
    dependencies: &[ResolvedApp],
) -> Result<Vec<String>> {
    let mut missing = Vec::new();

    for dep in dependencies {
        let app_key = format!("{}/{}", dep.project, dep.app_name);

        let mut running = false;
        let project_processes = store.find_by_metadata("project", &dep.project)?;
        for process in project_processes {
            if process.metadata.get("app_config_name").map(String::as_str) == Some(&dep.app_name)
                && store.is_running(&process)
            {
                running = true;
                break;
            }
        }

        if !running {
            missing.push(app_key);
        }
    }

    Ok(missing)
}
