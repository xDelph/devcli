//! `env-flow` — 2D layered env file loader.
//!
//! Loads `.env` files according to a **stage × runtime context** matrix,
//! merging them in priority order with optional `${VAR}` interpolation
//! and full source tracking.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use env_flow::{EnvFlow, RuntimeContext, Stage};
//!
//! // Explicit stage + context
//! let vars = EnvFlow::from_dir("./my-app")
//!     .stage(Stage::Dev)
//!     .context(RuntimeContext::Docker)
//!     .load()
//!     .unwrap();
//!
//! println!("PORT = {}", vars.get("PORT").unwrap_or("3000"));
//!
//! // Auto-detect everything
//! let vars = EnvFlow::from_dir(".").auto_detect().load().unwrap();
//! vars.apply(); // push into std::env
//! ```

pub mod context;
pub mod detector;
pub mod error;
pub mod interpolator;
pub mod loader;
pub mod parser;
pub mod resolver;
pub mod types;

pub use error::{Error, Result};
pub use types::{EnvEntry, EnvVars, LayerType, ResolvedLayer, RuntimeContext, Stage};

use std::path::{Path, PathBuf};

/// Builder for loading env files.
///
/// ```rust,no_run
/// use env_flow::EnvFlow;
/// let vars = EnvFlow::from_dir(".").load().unwrap();
/// ```
pub struct EnvFlow {
    root: PathBuf,
    stage: Option<Stage>,
    context: RuntimeContext,
    interpolate: bool,
    strict: bool,
}

impl EnvFlow {
    /// Create a builder targeting `root` directory.
    pub fn from_dir(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            stage: None,
            context: RuntimeContext::Local,
            interpolate: true,
            strict: false,
        }
    }

    /// Set the deployment stage.
    pub fn stage(mut self, stage: impl Into<Stage>) -> Self {
        self.stage = Some(stage.into());
        self
    }

    /// Set an optional stage — `None` leaves stage unset.
    pub fn stage_opt(mut self, stage: Option<Stage>) -> Self {
        self.stage = stage;
        self
    }

    /// Set the runtime context.
    pub fn context(mut self, ctx: impl Into<RuntimeContext>) -> Self {
        self.context = ctx.into();
        self
    }

    /// Enable/disable `${VAR}` interpolation (default: enabled).
    pub fn interpolate(mut self, yes: bool) -> Self {
        self.interpolate = yes;
        self
    }

    /// Enable strict parsing (malformed lines → error instead of warning).
    pub fn strict(mut self, yes: bool) -> Self {
        self.strict = yes;
        self
    }

    /// Auto-detect stage and runtime context from the process environment.
    pub fn auto_detect(mut self) -> Self {
        if self.stage.is_none() {
            self.stage = context::detect_stage();
        }
        self.context = context::detect_runtime_context_default();
        self
    }

    /// Compute the layer plan without loading files.
    ///
    /// Useful for introspection / debugging: shows which files *would* be loaded,
    /// including non-existent ones (`exists: false`).
    pub fn layers(&self) -> Result<Vec<ResolvedLayer>> {
        resolver::resolve(&self.root, self.stage.as_ref(), &self.context)
    }

    /// Load, merge, and optionally interpolate all env layers.
    pub fn load(self) -> Result<EnvVars> {
        let layers = resolver::resolve(&self.root, self.stage.as_ref(), &self.context)?;

        tracing::debug!(
            root = %self.root.display(),
            stage = ?self.stage,
            context = ?self.context,
            layer_count = layers.len(),
            "loading env-flow layers"
        );

        let mut vars = loader::load(&layers, self.strict)?;

        if self.interpolate {
            interpolator::interpolate_all(&mut vars.0)?;
        }

        tracing::debug!(var_count = vars.len(), "env-flow load complete");

        Ok(vars)
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn dir() -> TempDir {
        TempDir::new().unwrap()
    }

    fn write(root: &Path, name: &str, content: &str) {
        let path = root.join(name);
        if let Some(p) = path.parent() {
            fs::create_dir_all(p).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    // ── Basic load ──────────────────────────────────────────────────────────

    #[test]
    fn loads_base_env() {
        let d = dir();
        write(d.path(), ".env", "PORT=3000\nHOST=localhost\n");

        let vars = EnvFlow::from_dir(d.path()).load().unwrap();
        assert_eq!(vars.get("PORT"), Some("3000"));
        assert_eq!(vars.get("HOST"), Some("localhost"));
    }

    #[test]
    fn stage_overrides_base() {
        let d = dir();
        write(d.path(), ".env", "DB=localhost\nPORT=3000\n");
        write(d.path(), ".env.dev", "DB=dev-db\n");

        let vars = EnvFlow::from_dir(d.path())
            .stage(Stage::Dev)
            .load()
            .unwrap();
        assert_eq!(vars.get("DB"), Some("dev-db"));
        assert_eq!(vars.get("PORT"), Some("3000")); // inherited from base
    }

    #[test]
    fn local_overrides_stage() {
        let d = dir();
        write(d.path(), ".env", "SECRET=base\n");
        write(d.path(), ".env.dev", "SECRET=dev\n");
        write(d.path(), ".env.dev.local", "SECRET=local-dev\n");

        let vars = EnvFlow::from_dir(d.path())
            .stage(Stage::Dev)
            .context(RuntimeContext::Local)
            .load()
            .unwrap();
        assert_eq!(vars.get("SECRET"), Some("local-dev"));
    }

    #[test]
    fn docker_skips_local_files() {
        let d = dir();
        write(d.path(), ".env", "PORT=3000\n");
        write(d.path(), ".env.local", "PORT=9999\n");

        let vars = EnvFlow::from_dir(d.path())
            .context(RuntimeContext::Docker)
            .load()
            .unwrap();
        assert_eq!(vars.get("PORT"), Some("3000")); // .env.local skipped
    }

    #[test]
    fn docker_dir_style_loaded() {
        let d = dir();
        write(d.path(), ".env", "PORT=3000\n");
        write(d.path(), "docker/.env", "PORT=80\n");

        let vars = EnvFlow::from_dir(d.path())
            .context(RuntimeContext::Docker)
            .load()
            .unwrap();
        assert_eq!(vars.get("PORT"), Some("80"));
    }

    #[test]
    fn suffix_style_preferred_over_dir_style() {
        let d = dir();
        write(d.path(), ".env", "PORT=3000\n");
        write(d.path(), ".env.docker", "PORT=80\n");
        write(d.path(), "docker/.env", "PORT=90\n"); // should be ignored

        let vars = EnvFlow::from_dir(d.path())
            .context(RuntimeContext::Docker)
            .load()
            .unwrap();
        assert_eq!(vars.get("PORT"), Some("80"));
    }

    #[test]
    fn interpolation_works() {
        let d = dir();
        write(d.path(), ".env", "BASE=hello\nGREETING=${BASE}_world\n");

        let vars = EnvFlow::from_dir(d.path()).load().unwrap();
        assert_eq!(vars.get("GREETING"), Some("hello_world"));
    }

    #[test]
    fn interpolation_disabled() {
        let d = dir();
        write(d.path(), ".env", "BASE=hello\nGREETING=${BASE}_world\n");

        let vars = EnvFlow::from_dir(d.path())
            .interpolate(false)
            .load()
            .unwrap();
        assert_eq!(vars.get("GREETING"), Some("${BASE}_world"));
    }

    #[test]
    fn single_quoted_not_interpolated() {
        let d = dir();
        write(d.path(), ".env", "BASE=hello\nLIT='${BASE}_world'\n");

        let vars = EnvFlow::from_dir(d.path()).load().unwrap();
        // Single-quoted → not interpolated even though interpolation is on
        assert_eq!(vars.get("LIT"), Some("${BASE}_world"));
    }

    #[test]
    fn layers_returns_plan_without_loading() {
        let d = dir();
        write(d.path(), ".env", "KEY=val\n");

        let plan = EnvFlow::from_dir(d.path())
            .stage(Stage::Dev)
            .context(RuntimeContext::Local)
            .layers()
            .unwrap();

        // Should have: base, stage, local-override, stage-local-override
        assert_eq!(plan.len(), 4);
        assert!(plan.iter().any(|l| l.layer_type == LayerType::Base && l.exists));
        assert!(plan.iter().any(|l| l.layer_type == LayerType::StageBase && !l.exists));
    }

    #[test]
    fn require_fails_on_missing_key() {
        let d = dir();
        write(d.path(), ".env", "PORT=3000\n");

        let vars = EnvFlow::from_dir(d.path()).load().unwrap();
        let result = vars.require(&["PORT", "DATABASE_URL"]);
        assert!(result.is_err());
    }

    #[test]
    fn get_parsed_converts_type() {
        let d = dir();
        write(d.path(), ".env", "PORT=8080\n");

        let vars = EnvFlow::from_dir(d.path()).load().unwrap();
        let port: u16 = vars.get_parsed("PORT").unwrap();
        assert_eq!(port, 8080);
    }

    #[test]
    fn get_with_source_returns_path() {
        let d = dir();
        write(d.path(), ".env", "KEY=val\n");

        let vars = EnvFlow::from_dir(d.path()).load().unwrap();
        let (value, path) = vars.get_with_source("KEY").unwrap();
        assert_eq!(value, "val");
        assert!(path.exists());
    }

    #[test]
    fn empty_directory_returns_empty_vars() {
        let d = dir();
        let vars = EnvFlow::from_dir(d.path()).load().unwrap();
        assert!(vars.is_empty());
    }

    #[test]
    fn ci_context_skips_local_and_loads_ci_file() {
        let d = dir();
        write(d.path(), ".env", "APP=myapp\n");
        write(d.path(), ".env.ci", "CI_TIMEOUT=60\n");
        write(d.path(), ".env.local", "SECRET=should-be-skipped\n");

        let vars = EnvFlow::from_dir(d.path())
            .context(RuntimeContext::CI)
            .load()
            .unwrap();
        assert_eq!(vars.get("APP"), Some("myapp"));
        assert_eq!(vars.get("CI_TIMEOUT"), Some("60"));
        assert_eq!(vars.get("SECRET"), None);
    }

    #[test]
    fn kubernetes_loads_k8s_dir() {
        let d = dir();
        write(d.path(), ".env", "APP=myapp\n");
        write(d.path(), "k8s/.env", "K8S_NS=production\n");

        let vars = EnvFlow::from_dir(d.path())
            .context(RuntimeContext::Kubernetes)
            .load()
            .unwrap();
        assert_eq!(vars.get("K8S_NS"), Some("production"));
    }

    #[test]
    fn default_value_interpolation() {
        let d = dir();
        write(d.path(), ".env", "PORT=${APP_PORT:-3000}\n");

        // Make sure APP_PORT is not in env
        std::env::remove_var("APP_PORT");

        let vars = EnvFlow::from_dir(d.path()).load().unwrap();
        assert_eq!(vars.get("PORT"), Some("3000"));
    }
}
