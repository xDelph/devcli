//! Adapter between devcli-core and the `env-flow` crate.

use crate::Result;
use env_flow::{EnvFlow, EnvVars, ResolvedLayer, RuntimeContext, Stage};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

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

/// The stage whose env files are loaded: the one devcli resolved (`--stage`,
/// the app's `default_stages`, the `default-stage` preference), else none -
/// the base files only.
///
/// Never taken from the caller's shell. It used to fall back to `APP_ENV`,
/// `RUST_ENV`, `NODE_ENV`, `RAILS_ENV` or `FLASK_ENV`: a `NODE_ENV=production`
/// exported for another tool silently loaded `.env.production` into every
/// app, with stages (`production`, `staging`, anything) that `start --stage`
/// refuses, and only for the cascade - a file pinned in `env_files` was still
/// looked up under the stage devcli had resolved.
pub fn effective_stage(stage: Option<&str>) -> Option<Stage> {
    stage.map(Stage::from)
}

pub fn context_from_env(environment: &str) -> RuntimeContext {
    RuntimeContext::from(environment)
}

/// Env vars as handed to the child process. An empty value stays empty:
/// `DEBUG=` means "set to nothing", which many tools treat as false.
fn env_vars_to_map(vars: EnvVars) -> HashMap<String, String> {
    vars.iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
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

/// Load an env file pinned in the app's `env_files`, as `(vars, layers, summary)`.
///
/// A pinned file that does not exist used to yield no variables without a
/// word; it is now flagged in the summary ("Using env file: x (missing)") and
/// in the log, and `config validate` reports it.
fn load_pinned_file(
    working_dir: &Path,
    path: String,
) -> Result<(HashMap<String, String>, Vec<LayerInfo>, String)> {
    let full = working_dir.join(&path);
    if !full.exists() {
        tracing::warn!(path = %full.display(), "env file pinned in config does not exist");
        return Ok((HashMap::new(), Vec::new(), format!("{path} (missing)")));
    }
    Ok((load_file(&full)?, Vec::new(), path))
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
    let (vars, layers, summary) =
        if let Some(path) = resolve_config_env_path(env_files_map, stage, "local")? {
            load_pinned_file(working_dir, path)?
        } else {
            let (cascade_vars, cascade_layers) =
                load_runtime_cascade(working_dir, stage, "local", None)?;
            let existing = existing_layers(&cascade_layers);
            if existing.is_empty() {
                return Ok(None);
            }
            (
                env_vars_to_map(cascade_vars),
                layer_infos(&cascade_layers),
                cascade_summary(&existing),
            )
        };

    // Nothing to apply - unless a file was pinned: its "(missing)" summary is
    // the only sign the user gets that the configured file is not there.
    if vars.is_empty() && !summary.ends_with(" (missing)") {
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

    let (vars, layers, summary) =
        if let Some(path) = resolve_config_env_path(env_files_map, stage, environment)? {
            load_pinned_file(working_dir, path)?
        } else {
            let (cascade_vars, cascade_layers) =
                load_runtime_cascade(working_dir, stage, environment, dockerfile_path)?;
            let existing = existing_layers(&cascade_layers);
            if existing.is_empty() {
                return Ok(None);
            }
            (
                env_vars_to_map(cascade_vars),
                layer_infos(&cascade_layers),
                cascade_summary(&existing),
            )
        };

    // Nothing to apply - unless a file was pinned: its "(missing)" summary is
    // the only sign the user gets that the configured file is not there.
    if vars.is_empty() && !summary.ends_with(" (missing)") {
        return Ok(None);
    }

    Ok(Some(EnvLoad {
        vars,
        summary,
        layers,
    }))
}

/// Prepare a container `--env-file` path: the file pinned in the config, or the
/// env cascade merged into a private file named after `scratch_name`
/// (`<project>_<app>`).
///
/// The merged file used to be `$TMPDIR/devcli-env-<nanos>.env`: created with
/// the default umask (0644 on Linux /tmp, readable by every local user), never
/// deleted, one more per start. It now lives in the devcli directory
/// (`env-files/`, 0700) with mode 0600 and is overwritten on every start.
/// It is also generated when only one layer exists, so values reach the
/// container the same way whatever the number of layers.
pub fn prepare_container_env_file(
    working_dir: &Path,
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
    scratch_name: &str,
) -> Result<Option<ContainerEnvFile>> {
    if let Some(path) = resolve_config_env_path(env_files_map, stage, environment)? {
        let full = working_dir.join(&path);
        if !full.exists() {
            // docker would fail with a bare "no such file"; say which setting.
            anyhow::bail!(
                "Env file '{}' pinned in env_files for {} does not exist",
                path,
                environment
            );
        }
        return Ok(Some(ContainerEnvFile {
            path: full,
            summary: path,
        }));
    }

    let (vars, layers) = load_runtime_cascade(working_dir, stage, environment, dockerfile_path)?;
    let existing = existing_layers(&layers);
    if existing.is_empty() {
        return Ok(None);
    }

    let summary = if existing.len() == 1 {
        existing[0].relative_path.clone()
    } else {
        cascade_summary(&existing)
    };

    let dir = crate::config::loader::config_home().join("env-files");
    let path = dir.join(format!("{scratch_name}_{environment}.env"));
    write_private(
        &dir,
        &path,
        &container_env_file_contents(&vars, environment),
    )?;

    Ok(Some(ContainerEnvFile { path, summary }))
}

/// Render env vars in the dialect of the tool reading `--env-file`.
///
/// `docker run --env-file` takes every line literally - no quotes, escapes or
/// multi-line values - so a dotenv-quoted `"a b"` reached the container with
/// its quote characters. Docker Compose parses dotenv syntax and interpolates
/// `$` in unquoted and double-quoted values, so values are single-quoted there.
fn container_env_file_contents(vars: &EnvVars, environment: &str) -> String {
    let compose = environment == "docker-compose";
    let mut out = String::new();
    for (key, value) in vars.iter() {
        if compose {
            if value.contains('\'') {
                let escaped = value
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('$', "$$");
                out.push_str(&format!("{key}=\"{escaped}\"\n"));
            } else {
                out.push_str(&format!("{key}='{value}'\n"));
            }
        } else if value.contains('\n') {
            tracing::warn!(
                key = %key,
                "multi-line value cannot be passed through `docker --env-file`, skipped"
            );
        } else {
            out.push_str(&format!("{key}={value}\n"));
        }
    }
    out
}

/// Write `contents` to `path` readable by the owner only, in a 0700 `dir`.
fn write_private(dir: &Path, path: &Path, contents: &str) -> Result<()> {
    use std::io::Write;

    fs::create_dir_all(dir)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    // An older devcli may have created it with looser permissions.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(contents.as_bytes())?;
    Ok(())
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

    Ok(Some(format_env_display(
        &cascade_summary(&existing),
        &layer_infos(&layers),
    )))
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

pub(crate) fn resolve_config_env_path(
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
    environment: &str,
) -> Result<Option<String>> {
    let Some(env_files) = env_files_map else {
        return Ok(None);
    };

    if let Some(stage_name) = stage
        && let Some(stage_map) = env_files.get(stage_name)
        && let Some(path) = stage_map.get(environment)
    {
        return Ok(Some(path.clone()));
    }

    if let Some(base_map) = env_files.get("base")
        && let Some(path) = base_map.get(environment)
    {
        return Ok(Some(path.clone()));
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

    if let Some(parent) = dockerfile_parent_dir(working_dir, dockerfile_path)?
        && let Some(path) = highest_layer_relative_path(&parent, working_dir, stage, &context)?
    {
        return Ok(Some(path));
    }

    Ok(None)
}

fn dockerfile_parent_dir(
    working_dir: &Path,
    dockerfile_path: Option<&str>,
) -> Result<Option<PathBuf>> {
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
    fn the_stage_is_never_read_from_the_shell() {
        let _guard = crate::test_utils::env_guard();
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "FROM=base\n").unwrap();
        fs::write(dir.path().join(".env.production"), "FROM=production\n").unwrap();
        let previous = std::env::var_os("NODE_ENV");
        // SAFETY: test code; only std reads the environment here, and std serialises its own accesses.
        unsafe { std::env::set_var("NODE_ENV", "production") };

        let stage = effective_stage(None);
        let loaded = load_local_runtime(dir.path(), None, None);

        match previous {
            // SAFETY: test code; only std reads the environment here, and std serialises its own accesses.
            Some(value) => unsafe { std::env::set_var("NODE_ENV", value) },
            // SAFETY: test code; only std reads the environment here, and std serialises its own accesses.
            None => unsafe { std::env::remove_var("NODE_ENV") },
        }
        assert_eq!(stage, None);
        let loaded = loaded.unwrap().expect("the base .env is loaded");
        assert_eq!(loaded.vars["FROM"], "base");
    }

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
    fn empty_value_stays_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(".env");
        fs::write(&path, "EMPTY=\nFILLED=value\n").unwrap();

        let vars = load_file(&path).unwrap();
        assert_eq!(vars.get("EMPTY"), Some(&String::new()));
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
    fn prepare_container_env_file_merges_cascade_into_a_private_file() {
        let config_dir = crate::test_utils::TempConfigDir::new();
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("docker")).unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();
        fs::write(dir.path().join("docker/.env"), "PORT=8080\n").unwrap();

        let prepared = prepare_container_env_file(dir.path(), None, None, "docker", None, "p_api")
            .unwrap()
            .expect("expected merged env file");

        assert_eq!(
            prepared.path,
            config_dir.path().join("env-files/p_api_docker.env")
        );
        assert_eq!(prepared.summary, "cascade (2 files)");
        let contents = fs::read_to_string(&prepared.path).unwrap();
        assert!(contents.contains("PORT=8080"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&prepared.path), 0o600);
            assert_eq!(mode(&config_dir.path().join("env-files")), 0o700);
        }
    }

    #[test]
    fn prepare_container_env_file_single_layer_is_normalised_too() {
        let _config_dir = crate::test_utils::TempConfigDir::new();
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("docker")).unwrap();
        fs::write(dir.path().join("docker/.env"), "GREETING=\"hello world\"\n").unwrap();

        let prepared = prepare_container_env_file(dir.path(), None, None, "docker", None, "p_api")
            .unwrap()
            .expect("expected docker env file");

        assert_eq!(prepared.summary, "docker/.env");
        // `docker run --env-file` keeps quotes literally: write the bare value.
        let contents = fs::read_to_string(&prepared.path).unwrap();
        assert!(contents.contains("GREETING=hello world\n"), "{contents}");
    }

    #[test]
    fn container_env_file_dialects() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(".env"),
            "PLAIN=a b\nCOST='$5'\nQUOTE=\"it's\"\nMULTI=\"l1\\nl2\"\n",
        )
        .unwrap();
        let ordered = EnvFlow::from_file(dir.path().join(".env")).load().unwrap();

        let docker = container_env_file_contents(&ordered, "docker");
        assert!(docker.contains("PLAIN=a b\n"));
        assert!(docker.contains("COST=$5\n"));
        assert!(
            !docker.contains("MULTI"),
            "multi-line values cannot be passed"
        );

        let compose = container_env_file_contents(&ordered, "docker-compose");
        assert!(compose.contains("PLAIN='a b'\n"));
        assert!(compose.contains("COST='$5'\n"));
        assert!(compose.contains("QUOTE=\"it's\"\n"));
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

    #[test]
    fn missing_pinned_env_file_is_flagged() {
        let dir = TempDir::new().unwrap();
        let mut pins = HashMap::new();
        pins.insert(
            "base".to_string(),
            HashMap::from([("local".to_string(), ".env.missing".to_string())]),
        );

        let loaded = load_local_runtime(dir.path(), Some(&pins), None)
            .unwrap()
            .expect("pinned file reported");
        assert_eq!(loaded.summary, ".env.missing (missing)");
        assert!(loaded.vars.is_empty());
    }
}
