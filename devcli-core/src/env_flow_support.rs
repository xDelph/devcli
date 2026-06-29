//! Adapter between devcli-core and the `env-flow` crate.
//!
//! Handles type bridges, devcli-specific quirks (empty → `"XXX"`), cascade
//! loading for local runs, and strict config-map single-file loads.

use crate::Result;
use env_flow::{EnvFlow, EnvVars, ResolvedLayer, RuntimeContext, Stage};
use std::collections::HashMap;
use std::path::Path;

/// Result of loading env vars for a local runtime.
pub struct LocalEnvLoad {
    pub vars: HashMap<String, String>,
    /// Human-readable summary for CLI output (path or `cascade (N files)`).
    pub summary: String,
}

/// Map a devcli stage string to env-flow's `Stage`.
pub fn stage_from_opt(stage: Option<&str>) -> Option<Stage> {
    stage.map(|s| Stage::from(s))
}

/// Map a devcli runtime environment name to env-flow's `RuntimeContext`.
pub fn context_from_env(environment: &str) -> RuntimeContext {
    RuntimeContext::from(environment)
}

/// Convert env-flow vars to devcli's `HashMap`, applying devcli quirks.
pub fn env_vars_to_map(vars: EnvVars) -> HashMap<String, String> {
    vars.iter()
        .map(|(key, value)| {
            let value = if value.is_empty() {
                "XXX".to_string()
            } else {
                value.to_string()
            };
            (key.to_string(), value)
        })
        .collect()
}

/// Parse a single `.env` file using env-flow's parser (multiline, export, interpolation).
pub fn parse_env_file(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let vars = EnvFlow::from_file(path).load().map_err(map_error)?;
    Ok(env_vars_to_map(vars))
}

/// Load all env layers for a directory (cascade mode).
pub fn load_cascade(
    root: &Path,
    stage: Option<&str>,
    environment: &str,
) -> Result<HashMap<String, String>> {
    let mut builder = EnvFlow::from_dir(root).context(context_from_env(environment));

    if let Some(stage) = stage_from_opt(stage) {
        builder = builder.stage(stage);
    }

    let vars = builder.load().map_err(map_error)?;
    Ok(env_vars_to_map(vars))
}

/// Load env vars for local runtime.
///
/// - When `env_files_map` is set: strict single-file load from the configured path.
/// - Otherwise: cascade merge via env-flow (`.env` + stage + `.env.local` layers).
pub fn load_local_runtime(
    working_dir: &Path,
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
) -> Result<Option<LocalEnvLoad>> {
    if env_files_map.is_some() {
        let path = crate::detection::resolve_env_file_path(
            working_dir,
            env_files_map,
            stage,
            "local",
            None,
        )?;
        let Some(path) = path else {
            return Ok(None);
        };
        let vars = parse_env_file(&working_dir.join(&path))?;
        return Ok(Some(LocalEnvLoad {
            vars,
            summary: path,
        }));
    }

    let layers = EnvFlow::from_dir(working_dir)
        .stage_opt(stage_from_opt(stage))
        .context(RuntimeContext::Local)
        .layers()
        .map_err(map_error)?;

    let existing: Vec<&ResolvedLayer> = layers.iter().filter(|layer| layer.exists).collect();
    if existing.is_empty() {
        return Ok(None);
    }

    let vars = load_cascade(working_dir, stage, "local")?;
    Ok(Some(LocalEnvLoad {
        vars,
        summary: cascade_summary(&existing),
    }))
}

fn cascade_summary(layers: &[&ResolvedLayer]) -> String {
    if layers.len() == 1 {
        layers[0].relative_path.clone()
    } else {
        format!("cascade ({} files)", layers.len())
    }
}

fn map_error(err: env_flow::Error) -> anyhow::Error {
    anyhow::Error::new(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parse_multiline_and_export() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env");
        fs::write(
            &path,
            r#"export FOO=bar
MULTI="line1
line2"
"#,
        )
        .unwrap();

        let vars = parse_env_file(&path).unwrap();
        assert_eq!(vars.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(vars.get("MULTI"), Some(&"line1\nline2".to_string()));
    }

    #[test]
    fn parse_interpolation() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env");
        fs::write(&path, "BASE=hello\nGREETING=${BASE}_world\n").unwrap();

        let vars = parse_env_file(&path).unwrap();
        assert_eq!(vars.get("GREETING"), Some(&"hello_world".to_string()));
    }

    #[test]
    fn empty_value_becomes_xxx() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env");
        fs::write(&path, "EMPTY=\nFILLED=value\n").unwrap();

        let vars = parse_env_file(&path).unwrap();
        assert_eq!(vars.get("EMPTY"), Some(&"XXX".to_string()));
        assert_eq!(vars.get("FILLED"), Some(&"value".to_string()));
    }

    #[test]
    fn missing_file_returns_empty_map() {
        let dir = TempDir::new().unwrap();
        let vars = parse_env_file(&dir.path().join("missing.env")).unwrap();
        assert!(vars.is_empty());
    }

    #[test]
    fn load_cascade_merges_local_overrides() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\nDEBUG=false\n").unwrap();
        fs::write(dir.path().join(".env.local"), "DEBUG=true\n").unwrap();

        let vars = load_cascade(dir.path(), None, "local").unwrap();
        assert_eq!(vars.get("PORT"), Some(&"3000".to_string()));
        assert_eq!(vars.get("DEBUG"), Some(&"true".to_string()));
    }

    #[test]
    fn context_from_env_maps_orbstack() {
        assert_eq!(context_from_env("orbstack"), RuntimeContext::OrbStack);
        assert_eq!(context_from_env("docker"), RuntimeContext::Docker);
        assert_eq!(context_from_env("local"), RuntimeContext::Local);
    }

    #[test]
    fn load_local_runtime_cascade_with_stage() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();
        fs::write(dir.path().join(".env.dev"), "DB=dev\n").unwrap();
        fs::write(dir.path().join(".env.dev.local"), "SECRET=local\n").unwrap();

        let loaded = load_local_runtime(dir.path(), None, Some("dev"))
            .unwrap()
            .expect("expected cascade load");

        assert_eq!(loaded.vars.get("PORT"), Some(&"3000".to_string()));
        assert_eq!(loaded.vars.get("DB"), Some(&"dev".to_string()));
        assert_eq!(loaded.vars.get("SECRET"), Some(&"local".to_string()));
        assert_eq!(loaded.summary, "cascade (3 files)");
    }

    #[test]
    fn load_local_runtime_strict_config_map() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();
        fs::write(dir.path().join(".env.local"), "DEBUG=true\n").unwrap();
        fs::write(dir.path().join("custom.env"), "FROM_CONFIG=1\n").unwrap();

        let mut map = HashMap::new();
        map.insert(
            "base".to_string(),
            HashMap::from([("local".to_string(), "custom.env".to_string())]),
        );

        let loaded = load_local_runtime(dir.path(), Some(&map), None)
            .unwrap()
            .expect("expected config map load");

        assert_eq!(loaded.vars.get("FROM_CONFIG"), Some(&"1".to_string()));
        assert_eq!(loaded.vars.get("DEBUG"), None);
        assert_eq!(loaded.summary, "custom.env");
    }

    #[test]
    fn load_local_runtime_strict_config_map_no_match() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();

        let mut map = HashMap::new();
        map.insert("dev".to_string(), HashMap::from([("docker".to_string(), "x.env".to_string())]));

        let loaded = load_local_runtime(dir.path(), Some(&map), Some("dev")).unwrap();
        assert!(loaded.is_none());
    }
}
