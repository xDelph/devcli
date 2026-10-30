// Dependency resolution and checking
// Ensures apps start in the correct order and dependencies are running.
//
// Transitive chain resolution goes through config_manager_support
// (config-manager DependencyGraph). Runtime "is running?" checks stay here.

use super::models::Config;
use super::resolver::ResolvedApp;
use crate::Result;
use process_manager::StateStore;

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
    let Some(instance) = &resolved_app.worktree else {
        return crate::config_manager_support::resolve_dependency_chain(config, resolved_app);
    };
    // The dependencies are declared on the configured app.
    let base = ResolvedApp {
        project: instance.base_project.clone(),
        app_name: resolved_app.app_name.clone(),
        app: resolved_app.app.clone(),
        worktree: None,
    };
    let chain = crate::config_manager_support::resolve_dependency_chain(config, &base)?;
    Ok(chain
        .into_iter()
        .map(|dep| in_same_worktree(config, dep, instance))
        .collect())
}

/// `dep` as run alongside an app of a git worktree: its copy in the same
/// worktree when it lives in the same repository (the API next to the
/// frontend being changed), the configured instance otherwise (a shared
/// database, an app of another repository).
fn in_same_worktree(
    config: &Config,
    dep: ResolvedApp,
    instance: &crate::worktrees::Instance,
) -> ResolvedApp {
    let path = crate::utils::path::expand_path(&dep.app.path);
    let same_repo = crate::worktrees::git::checkout_of(&path)
        .is_some_and(|checkout| checkout.common_dir == instance.common_dir);
    if !same_repo {
        return dep;
    }
    let project = format!("{}@{}", dep.project, instance.slug);
    match crate::worktrees::resolve(
        config,
        &dep.app_name,
        Some(&project),
        crate::worktrees::Mode::Exact,
    ) {
        Ok(copy) => copy,
        // Not on this worktree's branch: the configured one will do.
        Err(e) => {
            tracing::debug!(error = %e, "Dependency runs from the configured checkout");
            dep
        }
    }
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

#[cfg(test)]
mod worktree_tests {
    use super::*;
    use crate::worktrees::git::tests::{init_repo, run_git};

    #[test]
    fn dependencies_of_a_worktree_instance_stay_in_its_worktree() {
        let _env = crate::test_utils::TempConfigDir::new();
        let dir = tempfile::TempDir::new().unwrap();
        let repo = init_repo(dir.path());
        std::fs::create_dir_all(repo.join("apps/web")).unwrap();
        std::fs::write(repo.join("apps/web/README"), "web\n").unwrap();
        run_git(&repo, &["add", "."]);
        run_git(&repo, &["commit", "-q", "-m", "web"]);
        run_git(
            &repo,
            &["worktree", "add", "-q", "../wt/feat", "-b", "feat"],
        );
        let redis = dir.path().join("redis");
        std::fs::create_dir_all(&redis).unwrap();

        let app = |path: std::path::PathBuf, deps: serde_json::Value| {
            serde_json::json!({
                "type": "custom", "path": path,
                "commands": { "local": { "start": "true" } },
                "defaults": { "local": "start" },
                "dependencies": deps,
            })
        };
        let config: Config = serde_json::from_value(serde_json::json!({ "projects": {
            "proj": { "apps": {
                "web": app(repo.join("apps/web"), serde_json::json!([
                    { "project": "proj", "app": "api" },
                    { "project": "infra", "app": "redis" },
                ])),
                "api": app(repo.join("apps/api"), serde_json::json!([])),
            } },
            "infra": { "apps": { "redis": app(redis, serde_json::json!([])) } },
        } }))
        .unwrap();

        let web =
            crate::worktrees::resolve(&config, "web@feat", None, crate::worktrees::Mode::Exact)
                .unwrap();
        let chain = resolve_dependency_chain(&config, &web).unwrap();
        let mut names: Vec<_> = chain
            .iter()
            .map(|d| format!("{}/{}", d.project, d.app_name))
            .collect();
        names.sort();
        assert_eq!(names, vec!["infra/redis", "proj@feat/api"]);

        let api = chain.iter().find(|d| d.app_name == "api").unwrap();
        assert!(api.app.path.contains("wt/feat"), "{}", api.app.path);

        // The configured web keeps the configured api.
        let base_web =
            crate::worktrees::resolve(&config, "web", None, crate::worktrees::Mode::Exact).unwrap();
        let chain = resolve_dependency_chain(&config, &base_web).unwrap();
        assert!(chain.iter().all(|d| d.worktree.is_none()));
    }
}
