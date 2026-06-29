//! Adapter between devcli-core and the `env-flow` crate.
//!
//! Handles type bridges, devcli-specific quirks (empty → `"XXX"`), and will
//! eventually own cascade / config-map loading for `prepare.rs`.

use crate::Result;
use env_flow::{EnvFlow, EnvVars, RuntimeContext, Stage};
use std::collections::HashMap;
use std::path::Path;

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
}
