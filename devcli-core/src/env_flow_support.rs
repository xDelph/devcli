//! Adapter between devcli-core and the `env-flow` crate.

use crate::Result;
use env_flow::{EnvFlow, EnvVars, ResolvedLayer, RuntimeContext, Stage};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// One resolved env layer for display / introspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerInfo {
    pub relative_path: String,
    pub exists: bool,
}

/// Loaded env vars plus a short summary for CLI output.
pub struct EnvLoad {
    pub vars: HashMap<String, String>,
    pub summary: String,
    pub layers: Vec<LayerInfo>,
}

/// Container `--env-file` target (may be a merged temp file).
pub struct ContainerEnvFile {
    pub path: PathBuf,
    pub summary: String,
}

pub fn stage_from_opt(stage: Option<&str>) -> Option<Stage> {
    stage.map(Stage::from)
}

/// Config stage when set, otherwise auto-detect from process env (`APP_ENV`, `NODE_ENV`, …).
pub fn effective_stage(stage: Option<&str>) -> Option<Stage> {
    stage
        .map(Stage::from)
        .or_else(env_flow::context::detect_stage)
}

pub fn context_from_env(environment: &str) -> RuntimeContext {
    RuntimeContext::from(environment)
}

fn env_vars_to_map(vars: EnvVars) -> HashMap<String, String> {
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

fn layer_infos(layers: &[ResolvedLayer]) -> Vec<LayerInfo> {
    layers
        .iter()
        .map(|layer| LayerInfo {
            relative_path: layer.relative_path.clone(),
            exists: layer.exists,
        })
        .collect()
}

fn load_file(path: &Path) -> Result<HashMap<String, String>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let vars = EnvFlow::from_file(path).load().map_err(map_error)?;
    Ok(env_vars_to_map(vars))
}

fn load_cascade_at_root(
    root: &Path,
    stage: Option<&str>,
    environment: &str,
) -> Result<(EnvVars, Vec<ResolvedLayer>)> {
    let stage = effective_stage(stage);
    let builder = EnvFlow::from_dir(root)
        .stage_opt(stage)
        .context(context_from_env(environment));

    let layers = builder.layers().map_err(map_error)?;
    let vars = builder.load().map_err(map_error)?;
    Ok((vars, layers))
}

fn load_runtime_cascade(
    working_dir: &Path,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Result<(EnvVars, Vec<ResolvedLayer>)> {
    let (mut vars, mut layers) = load_cascade_at_root(working_dir, stage, environment)?;

    if let Some(parent) = dockerfile_parent_dir(working_dir, dockerfile_path)? {
        let (parent_vars, parent_layers) = load_cascade_at_root(&parent, stage, environment)?;
        vars.merge_from(parent_vars);
        layers = merge_layer_lists(layers, parent_layers);
    }

    Ok((vars, layers))
}

fn merge_layer_lists(
    mut base: Vec<ResolvedLayer>,
    overlay: Vec<ResolvedLayer>,
) -> Vec<ResolvedLayer> {
    for layer in overlay {
        if layer.exists
            && !base
                .iter()
                .any(|existing| existing.relative_path == layer.relative_path)
        {
            base.push(layer);
        }
    }
    base
}

fn existing_layers(layers: &[ResolvedLayer]) -> Vec<&ResolvedLayer> {
    layers.iter().filter(|layer| layer.exists).collect()
}

/// Load env vars for local runtime (cascade, with optional config pin override).
pub fn load_local_runtime(
    working_dir: &Path,
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
) -> Result<Option<EnvLoad>> {
    let mut vars = HashMap::new();
    let mut layers = Vec::new();
    let mut summary = String::new();

    if let Some(path) = resolve_config_env_path(env_files_map, stage, "local")? {
        vars = load_file(&working_dir.join(&path))?;
        summary = path;
    } else {
        let (cascade_vars, cascade_layers) =
            load_runtime_cascade(working_dir, stage, "local", None)?;
        let existing = existing_layers(&cascade_layers);
        if existing.is_empty() {
            return Ok(None);
        }
        vars = env_vars_to_map(cascade_vars);
        layers = layer_infos(&cascade_layers);
        summary = cascade_summary(&existing);
    }

    if vars.is_empty() {
        return Ok(None);
    }

    Ok(Some(EnvLoad {
        vars,
        summary,
        layers,
    }))
}

/// Load env vars for process injection (local, k8s, ci).
pub fn load_process_runtime(
    working_dir: &Path,
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Result<Option<EnvLoad>> {
    if environment == "local" {
        return load_local_runtime(working_dir, env_files_map, stage);
    }

    let mut vars = HashMap::new();
    let mut layers = Vec::new();
    let mut summary = String::new();

    if let Some(path) = resolve_config_env_path(env_files_map, stage, environment)? {
        vars = load_file(&working_dir.join(&path))?;
        summary = path;
    } else {
        let (cascade_vars, cascade_layers) =
            load_runtime_cascade(working_dir, stage, environment, dockerfile_path)?;
        let existing = existing_layers(&cascade_layers);
        if existing.is_empty() {
            return Ok(None);
        }
        vars = env_vars_to_map(cascade_vars);
        layers = layer_infos(&cascade_layers);
        summary = cascade_summary(&existing);
    }

    if vars.is_empty() {
        return Ok(None);
    }

    Ok(Some(EnvLoad {
        vars,
        summary,
        layers,
    }))
}

/// Prepare a container `--env-file` path (single file or merged cascade temp file).
pub fn prepare_container_env_file(
    working_dir: &Path,
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Result<Option<ContainerEnvFile>> {
    if let Some(path) = resolve_config_env_path(env_files_map, stage, environment)? {
        return Ok(Some(ContainerEnvFile {
            path: working_dir.join(&path),
            summary: path,
        }));
    }

    let (vars, layers) =
        load_runtime_cascade(working_dir, stage, environment, dockerfile_path)?;
    let existing = existing_layers(&layers);
    if existing.is_empty() {
        return Ok(None);
    }

    let summary = cascade_summary(&existing);

    if existing.len() == 1 {
        return Ok(Some(ContainerEnvFile {
            path: existing[0].path.clone(),
            summary: if summary.contains("cascade") {
                existing[0].relative_path.clone()
            } else {
                summary
            },
        }));
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_path = std::env::temp_dir().join(format!("devcli-env-{stamp}.env"));
    vars.write_dotenv(&temp_path).map_err(map_error)?;

    Ok(Some(ContainerEnvFile {
        path: temp_path,
        summary,
    }))
}

/// Resolve the env file path for container runtimes (`--env-file`) — highest layer only.
///
/// Prefer [`prepare_container_env_file`] for runtime use appends cascade merge when needed.
pub fn resolve_container_env_file_path(
    working_dir: &Path,
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Result<Option<String>> {
    if let Some(path) = resolve_config_env_path(env_files_map, stage, environment)? {
        return Ok(Some(path));
    }

    resolve_highest_layer_path(working_dir, stage, environment, dockerfile_path)
}

/// Describe which env file(s) would be used at runtime (`config list`, etc.).
pub fn resolve_runtime_env_display(
    working_dir: &Path,
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Result<Option<String>> {
    if let Some(path) = resolve_config_env_path(env_files_map, stage, environment)? {
        return Ok(Some(path));
    }

    let layers = runtime_layers(working_dir, stage, environment, dockerfile_path)?;
    let existing = existing_layers(&layers);
    if existing.is_empty() {
        return Ok(None);
    }

    Ok(Some(format_env_display(&cascade_summary(&existing), &layer_infos(&layers))))
}

/// Layer plan for one runtime (includes non-existent layers).
pub fn resolve_runtime_layers(
    working_dir: &Path,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Result<Vec<LayerInfo>> {
    Ok(layer_infos(&runtime_layers(
        working_dir,
        stage,
        environment,
        dockerfile_path,
    )?))
}

fn runtime_layers(
    working_dir: &Path,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Result<Vec<ResolvedLayer>> {
    let (_, layers) = load_runtime_cascade(working_dir, stage, environment, dockerfile_path)?;
    Ok(layers)
}

pub fn format_env_display(summary: &str, layers: &[LayerInfo]) -> String {
    let loaded: Vec<_> = layers
        .iter()
        .filter(|layer| layer.exists)
        .map(|layer| layer.relative_path.as_str())
        .collect();

    if loaded.len() <= 1 {
        summary.to_string()
    } else {
        format!("{} [{}]", summary, loaded.join(", "))
    }
}

fn resolve_config_env_path(
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
    environment: &str,
) -> Result<Option<String>> {
    let Some(env_files) = env_files_map else {
        return Ok(None);
    };

    if let Some(stage_name) = stage {
        if let Some(stage_map) = env_files.get(stage_name) {
            if let Some(path) = stage_map.get(environment) {
                return Ok(Some(path.clone()));
            }
        }
    }

    if let Some(base_map) = env_files.get("base") {
        if let Some(path) = base_map.get(environment) {
            return Ok(Some(path.clone()));
        }
    }

    Ok(None)
}

fn resolve_highest_layer_path(
    working_dir: &Path,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Result<Option<String>> {
    let context = context_from_env(environment);

    if let Some(path) = highest_layer_relative_path(working_dir, working_dir, stage, &context)? {
        return Ok(Some(path));
    }

    if let Some(parent) = dockerfile_parent_dir(working_dir, dockerfile_path)? {
        if let Some(path) = highest_layer_relative_path(&parent, working_dir, stage, &context)? {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn dockerfile_parent_dir(working_dir: &Path, dockerfile_path: Option<&str>) -> Result<Option<PathBuf>> {
    let Some(dockerfile_rel) = dockerfile_path else {
        return Ok(None);
    };

    let parent = working_dir
        .join(dockerfile_rel)
        .parent()
        .map(|path| path.to_path_buf());

    Ok(parent.filter(|path| path != working_dir))
}

fn highest_layer_relative_path(
    scan_root: &Path,
    rel_base: &Path,
    stage: Option<&str>,
    context: &RuntimeContext,
) -> Result<Option<String>> {
    let layers = EnvFlow::from_dir(scan_root)
        .stage_opt(effective_stage(stage))
        .context(context.clone())
        .layers()
        .map_err(map_error)?;

    let Some(layer) = layers.iter().rev().find(|layer| layer.exists) else {
        return Ok(None);
    };

    Ok(Some(relative_path_from(
        rel_base,
        &layer.path,
        &layer.relative_path,
    )))
}

fn relative_path_from(base: &Path, absolute: &Path, fallback: &str) -> String {
    absolute
        .strip_prefix(base)
        .ok()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| fallback.to_string())
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
    fn load_file_multiline_and_export() {
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

        let vars = load_file(&path).unwrap();
        assert_eq!(vars.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(vars.get("MULTI"), Some(&"line1\nline2".to_string()));
    }

    #[test]
    fn load_file_interpolation() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env");
        fs::write(&path, "BASE=hello\nGREETING=${BASE}_world\n").unwrap();

        let vars = load_file(&path).unwrap();
        assert_eq!(vars.get("GREETING"), Some(&"hello_world".to_string()));
    }

    #[test]
    fn empty_value_becomes_xxx() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env");
        fs::write(&path, "EMPTY=\nFILLED=value\n").unwrap();

        let vars = load_file(&path).unwrap();
        assert_eq!(vars.get("EMPTY"), Some(&"XXX".to_string()));
        assert_eq!(vars.get("FILLED"), Some(&"value".to_string()));
    }

    #[test]
    fn load_cascade_merges_local_overrides() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\nDEBUG=false\n").unwrap();
        fs::write(dir.path().join(".env.local"), "DEBUG=true\n").unwrap();

        let (vars, _) = load_runtime_cascade(dir.path(), None, "local", None).unwrap();
        let map = env_vars_to_map(vars);
        assert_eq!(map.get("PORT"), Some(&"3000".to_string()));
        assert_eq!(map.get("DEBUG"), Some(&"true".to_string()));
    }

    #[test]
    fn load_cascade_supports_reverse_local_env() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();
        fs::write(dir.path().join(".local.env"), "DEBUG=true\n").unwrap();

        let (vars, _) = load_runtime_cascade(dir.path(), None, "local", None).unwrap();
        let map = env_vars_to_map(vars);
        assert_eq!(map.get("DEBUG"), Some(&"true".to_string()));
    }

    #[test]
    fn context_from_env_maps_orbstack_ci_compose() {
        assert_eq!(context_from_env("orbstack"), RuntimeContext::OrbStack);
        assert_eq!(context_from_env("docker"), RuntimeContext::Docker);
        assert_eq!(context_from_env("local"), RuntimeContext::Local);
        assert_eq!(context_from_env("ci"), RuntimeContext::CI);
        assert_eq!(
            context_from_env("docker-compose"),
            RuntimeContext::DockerCompose
        );
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
    fn load_local_runtime_config_pin() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("custom.env"), "FROM_CONFIG=1\n").unwrap();

        let map = HashMap::from([(
            "base".to_string(),
            HashMap::from([("local".to_string(), "custom.env".to_string())]),
        )]);

        let loaded = load_local_runtime(dir.path(), Some(&map), None)
            .unwrap()
            .expect("expected config map load");

        assert_eq!(loaded.vars.get("FROM_CONFIG"), Some(&"1".to_string()));
        assert_eq!(loaded.summary, "custom.env");
    }

    #[test]
    fn load_local_runtime_config_map_no_match_falls_back_to_cascade() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();
        fs::write(dir.path().join(".env.local"), "DEBUG=true\n").unwrap();

        let map = HashMap::from([(
            "dev".to_string(),
            HashMap::from([("docker".to_string(), "missing.env".to_string())]),
        )]);

        let loaded = load_local_runtime(dir.path(), Some(&map), Some("dev"))
            .unwrap()
            .expect("expected cascade fallback");

        assert_eq!(loaded.vars.get("DEBUG"), Some(&"true".to_string()));
        assert_eq!(loaded.summary, "cascade (2 files)");
    }

    #[test]
    fn prepare_container_env_file_merges_cascade_to_temp() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("docker")).unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();
        fs::write(dir.path().join("docker/.env"), "PORT=8080\n").unwrap();

        let prepared = prepare_container_env_file(dir.path(), None, None, "docker", None)
            .unwrap()
            .expect("expected merged env file");

        assert!(prepared.path.exists());
        assert_eq!(prepared.summary, "cascade (2 files)");
        let contents = fs::read_to_string(&prepared.path).unwrap();
        assert!(contents.contains("PORT=8080"));
    }

    #[test]
    fn prepare_container_env_file_single_layer_uses_disk_path() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("docker")).unwrap();
        fs::write(dir.path().join("docker/.env"), "PORT=8080\n").unwrap();

        let prepared = prepare_container_env_file(dir.path(), None, None, "docker", None)
            .unwrap()
            .expect("expected docker env file");

        assert_eq!(prepared.path, dir.path().join("docker/.env"));
        assert_eq!(prepared.summary, "docker/.env");
    }

    #[test]
    fn resolve_container_env_file_prefers_docker_context_layer() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("docker")).unwrap();
        fs::write(dir.path().join(".env"), "ENV=root\n").unwrap();
        fs::write(dir.path().join(".env.dev"), "ENV=root_dev\n").unwrap();
        fs::write(dir.path().join("docker/.env.dev"), "ENV=docker_dev\n").unwrap();

        let path = resolve_container_env_file_path(
            dir.path(),
            None,
            Some("dev"),
            "docker",
            Some("docker/Dockerfile"),
        )
        .unwrap()
        .expect("expected docker env file");

        assert_eq!(path, "docker/.env.dev");
    }

    #[test]
    fn resolve_container_env_file_nested_dockerfile_dir() {
        let dir = TempDir::new().unwrap();
        let docker_dir = dir.path().join("build/docker");
        fs::create_dir_all(&docker_dir).unwrap();
        fs::write(docker_dir.join(".env.prod"), "ENV=prod\n").unwrap();

        let path = resolve_container_env_file_path(
            dir.path(),
            None,
            Some("prod"),
            "docker",
            Some("build/docker/Dockerfile"),
        )
        .unwrap()
        .expect("expected nested docker env file");

        assert_eq!(path, "build/docker/.env.prod");
    }

    #[test]
    fn resolve_container_env_file_strict_config_map() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("pinned.env"), "ENV=pinned\n").unwrap();

        let map = HashMap::from([(
            "base".to_string(),
            HashMap::from([("docker".to_string(), "pinned.env".to_string())]),
        )]);

        let path = resolve_container_env_file_path(dir.path(), Some(&map), None, "docker", None)
            .unwrap()
            .expect("expected config map path");

        assert_eq!(path, "pinned.env");
    }

    #[test]
    fn resolve_container_env_file_orbstack_context() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("orbstack")).unwrap();
        fs::write(dir.path().join("orbstack/.env"), "ENV=orb\n").unwrap();

        let path = resolve_container_env_file_path(dir.path(), None, None, "orbstack", None)
            .unwrap()
            .expect("expected orbstack env file");

        assert_eq!(path, "orbstack/.env");
    }

    #[test]
    fn resolve_runtime_env_display_local_cascade_with_layers() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "A=1\n").unwrap();
        fs::write(dir.path().join(".env.local"), "B=2\n").unwrap();

        let display = resolve_runtime_env_display(dir.path(), None, None, "local", None)
            .unwrap()
            .expect("expected display");

        assert_eq!(display, "cascade (2 files) [.env, .env.local]");
    }

    #[test]
    fn resolve_runtime_env_display_docker_context() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("docker")).unwrap();
        fs::write(dir.path().join("docker/.env"), "PORT=3000\n").unwrap();

        let display = resolve_runtime_env_display(
            dir.path(),
            None,
            None,
            "docker",
            Some("docker/Dockerfile"),
        )
        .unwrap()
        .expect("expected docker display");

        assert_eq!(display, "docker/.env");
    }

    #[test]
    fn load_process_runtime_ci_context() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "APP=1\n").unwrap();
        fs::write(dir.path().join(".env.ci"), "CI=1\n").unwrap();

        let loaded = load_process_runtime(dir.path(), None, None, "ci", None)
            .unwrap()
            .expect("expected ci load");

        assert_eq!(loaded.vars.get("APP"), Some(&"1".to_string()));
        assert_eq!(loaded.vars.get("CI"), Some(&"1".to_string()));
    }
}
