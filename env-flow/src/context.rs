/// Auto-detection of `Stage` and `RuntimeContext` from the process environment.
use std::path::Path;

use crate::types::{RuntimeContext, Stage};

/// Detect which stage is active by inspecting well-known env vars.
///
/// First match wins:
/// `APP_ENV` → `RUST_ENV` → `NODE_ENV` → `RAILS_ENV` → `FLASK_ENV`
pub fn detect_stage() -> Option<Stage> {
    for key in &["APP_ENV", "RUST_ENV", "NODE_ENV", "RAILS_ENV", "FLASK_ENV"] {
        if let Ok(val) = std::env::var(key) {
            if !val.is_empty() {
                tracing::debug!(key, value = %val, "detected stage from env var");
                return Some(Stage::from(val.as_str()));
            }
        }
    }
    None
}

/// Detect the current runtime context.
///
/// `rootfs` is normally `/` on real systems; tests can inject a `TempDir` path
/// so that fake `/.dockerenv` files can be placed without root access.
pub fn detect_runtime_context(rootfs: &Path) -> RuntimeContext {
    // Kubernetes: guaranteed env var injected by the kubelet
    if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        tracing::debug!("detected Kubernetes via KUBERNETES_SERVICE_HOST");
        return RuntimeContext::Kubernetes;
    }

    // Docker / OrbStack: /.dockerenv marker file
    let dockerenv = rootfs.join(".dockerenv");
    if dockerenv.exists() {
        if is_orbstack(rootfs) {
            tracing::debug!("detected OrbStack via /.dockerenv + OrbStack marker");
            return RuntimeContext::OrbStack;
        }
        tracing::debug!("detected Docker via /.dockerenv");
        return RuntimeContext::Docker;
    }

    // CI: multiple well-known env vars
    if is_ci_environment() {
        tracing::debug!("detected CI environment");
        return RuntimeContext::CI;
    }

    // Docker Compose: project name var
    if std::env::var("COMPOSE_PROJECT_NAME").is_ok() {
        tracing::debug!("detected Docker Compose via COMPOSE_PROJECT_NAME");
        return RuntimeContext::DockerCompose;
    }

    RuntimeContext::Local
}

/// Convenience wrapper using the real filesystem root.
pub fn detect_runtime_context_default() -> RuntimeContext {
    detect_runtime_context(Path::new("/"))
}

/// Heuristic for OrbStack: look for OrbStack-specific markers inside the container.
fn is_orbstack(rootfs: &Path) -> bool {
    // OrbStack sets `ORBSTACK_RUNTIME` or writes `/run/orbstack` inside the VM
    if std::env::var("ORBSTACK_RUNTIME").is_ok() {
        return true;
    }
    rootfs.join("run").join("orbstack").exists()
        || rootfs.join("etc").join("orbstack-release").exists()
}

/// Check common CI environment variables.
fn is_ci_environment() -> bool {
    // Generic CI=true/1 (used by many platforms including GitHub Actions)
    if let Ok(val) = std::env::var("CI") {
        if val == "true" || val == "1" {
            return true;
        }
    }
    // Platform-specific
    std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("GITLAB_CI").is_ok()
        || std::env::var("CIRCLECI").is_ok()
        || std::env::var("JENKINS_URL").is_ok()
        || std::env::var("TRAVIS").is_ok()
        || std::env::var("BUILDKITE").is_ok()
        || std::env::var("CODEBUILD_BUILD_ID").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: temporarily set an env var for the duration of a closure, then restore.
    struct EnvGuard {
        key: String,
        old: Option<String>,
    }
    impl EnvGuard {
        fn set(key: &str, value: &str) -> Self {
            let old = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key: key.to_string(), old }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(v) => std::env::set_var(&self.key, v),
                None => std::env::remove_var(&self.key),
            }
        }
    }

    // All these tests mutate process env vars and must not run in parallel.
    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_all_stage_vars() {
        for k in &["APP_ENV", "RUST_ENV", "NODE_ENV", "RAILS_ENV", "FLASK_ENV"] {
            std::env::remove_var(k);
        }
    }

    #[test]
    fn detects_stage_from_app_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_all_stage_vars();
        let _g = EnvGuard::set("APP_ENV", "prod");
        assert_eq!(detect_stage(), Some(Stage::Prod));
    }

    #[test]
    fn detects_stage_from_node_env() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_all_stage_vars();
        let _g = EnvGuard::set("NODE_ENV", "staging");
        assert_eq!(detect_stage(), Some(Stage::Staging));
    }

    #[test]
    fn no_stage_env_returns_none() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_all_stage_vars();
        assert_eq!(detect_stage(), None);
    }

    fn clear_runtime_vars() {
        for k in &[
            "KUBERNETES_SERVICE_HOST",
            "ORBSTACK_RUNTIME",
            "CI",
            "GITHUB_ACTIONS",
            "GITLAB_CI",
            "CIRCLECI",
            "JENKINS_URL",
            "TRAVIS",
            "BUILDKITE",
            "CODEBUILD_BUILD_ID",
            "COMPOSE_PROJECT_NAME",
        ] {
            std::env::remove_var(k);
        }
    }

    #[test]
    fn detects_docker_via_dockerenv_file() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".dockerenv"), b"").unwrap();
        clear_runtime_vars();

        let ctx = detect_runtime_context(dir.path());
        assert_eq!(ctx, RuntimeContext::Docker);
    }

    #[test]
    fn detects_orbstack_via_env_var() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".dockerenv"), b"").unwrap();
        clear_runtime_vars();
        let _g = EnvGuard::set("ORBSTACK_RUNTIME", "1");

        let ctx = detect_runtime_context(dir.path());
        assert_eq!(ctx, RuntimeContext::OrbStack);
    }

    #[test]
    fn detects_kubernetes() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        clear_runtime_vars();
        let _g = EnvGuard::set("KUBERNETES_SERVICE_HOST", "10.96.0.1");

        let ctx = detect_runtime_context(dir.path());
        assert_eq!(ctx, RuntimeContext::Kubernetes);
    }

    #[test]
    fn detects_ci_github_actions() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        clear_runtime_vars();
        let _g = EnvGuard::set("GITHUB_ACTIONS", "true");

        let ctx = detect_runtime_context(dir.path());
        assert_eq!(ctx, RuntimeContext::CI);
    }

    #[test]
    fn defaults_to_local() {
        let _lock = ENV_LOCK.lock().unwrap();
        let dir = TempDir::new().unwrap();
        clear_all_stage_vars();
        clear_runtime_vars();

        let ctx = detect_runtime_context(dir.path());
        assert_eq!(ctx, RuntimeContext::Local);
    }
}
