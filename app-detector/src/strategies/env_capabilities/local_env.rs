//! Local environment capability detection strategy
//! Extracts local commands based on detected app type

use crate::utils::pyproject;
use crate::{context::DetectionContext, strategy::DetectionStrategy, types::*, Result};
use std::collections::HashMap;

/// Detects local environment commands based on app type
#[derive(Default)]
pub struct LocalEnvStrategy;

impl DetectionStrategy for LocalEnvStrategy {
    fn id(&self) -> &str {
        "local-env"
    }

    fn name(&self) -> &str {
        "Local Environment"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::EnvCapability(EnvCapabilityCategory::Local)
    }

    fn priority(&self) -> usize {
        500 // Environment capabilities run after app types
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // Check if we have any supported app type detected
        // This strategy depends on having an app type first
        // Note: For monorepos (nx), we detect at root level with limited commands
        ctx.get_result("nodejs").is_some()
            || ctx.get_result("nx").is_some()
            || ctx.get_result("python").is_some()
            || ctx.get_result("redis").is_some()
            || ctx.get_result("traefik").is_some()
            || ctx.get_result("rust").is_some()
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let mut commands = HashMap::new();
        let mut metadata = HashMap::new();
        let mut detected_app_type = None;

        // Detect commands based on app type results
        if ctx.get_result("nodejs").is_some() {
            detected_app_type = Some("nodejs");
            extract_nodejs_commands(ctx, &mut commands);
        }

        if ctx.get_result("nx").is_some() {
            detected_app_type = Some("nx");
            extract_nx_commands(ctx, &mut commands);
        }

        if ctx.get_result("python").is_some() {
            detected_app_type = Some("python");
            extract_python_commands(ctx, &mut commands);
        }

        if ctx.get_result("redis").is_some() {
            detected_app_type = Some("redis");
            extract_redis_commands(ctx, &mut commands);
        }

        if ctx.get_result("traefik").is_some() {
            detected_app_type = Some("traefik");
            extract_traefik_commands(ctx, &mut commands);
        }

        if ctx.get_result("rust").is_some() {
            detected_app_type = Some("rust");
            extract_rust_commands(ctx, &mut commands);
        }

        if let Some(app_type) = detected_app_type {
            metadata.insert("app_type".to_string(), serde_json::json!(app_type));
        }

        // Suggest a default command
        let suggested_default = suggest_default_command(detected_app_type, &commands);

        metadata.insert(
            "command_count".to_string(),
            serde_json::json!(commands.len()),
        );

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::LocalEnv(LocalEnvInfo {
                commands,
                suggested_default,
                metadata,
            }),
            suggested_strategies: vec![],
        })
    }

    fn depends_on(&self) -> Vec<&str> {
        // Depends on app type strategies
        vec!["nodejs", "nx", "python", "redis", "traefik", "rust"]
    }
}

fn extract_nodejs_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    let pm = node_package_manager(ctx);
    if let Ok(content) = ctx.read_file("package.json")
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
        && let Some(scripts) = json.get("scripts").and_then(|v| v.as_object())
    {
        for (key, _value) in scripts {
            commands.insert(
                key.clone(),
                crate::utils::package_manager::node_script_command(&pm, key),
            );
        }
    }
}

fn node_package_manager(ctx: &DetectionContext) -> String {
    ctx.get_result("nodejs")
        .and_then(|result| match &result.data {
            DetectionData::Language(info) => info
                .metadata
                .get("package_manager")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| crate::utils::package_manager::detect_node_package_manager(ctx))
}

fn extract_nx_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    // Nx uses npm scripts like nodejs
    extract_nodejs_commands(ctx, commands);
}

fn extract_python_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    if let Some(result) = ctx.get_result("python")
        && let DetectionData::Monorepo(info) = &result.data
    {
        extract_python_workspace_commands(ctx, info, commands);
        return;
    }

    extract_python_package_commands(ctx, commands);
}

fn extract_python_workspace_commands(
    ctx: &DetectionContext,
    info: &MonorepoInfo,
    commands: &mut HashMap<String, String>,
) {
    let pm = python_package_manager(ctx);

    commands.insert("install".to_string(), python_install_command(&pm));
    commands.insert("test".to_string(), python_workspace_test_command(&pm));

    for workspace in &info.workspace_info {
        let name = workspace
            .name
            .clone()
            .unwrap_or_else(|| workspace.path.replace('/', "-"));
        let run_key = format!("run-{name}");
        commands.insert(
            run_key,
            python_member_run_command(&pm, &workspace.path, &name),
        );
    }
}

/// Commands for a single Python package, only for files that exist: a
/// pyproject-only project got `pip install -r requirements.txt` and
/// `python main.py` with neither file there.
fn extract_python_package_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    let pm = python_package_manager(ctx);
    let pyproject = ctx.read_file("pyproject.toml").unwrap_or_default();
    let run = python_run_prefix(&pm);

    if let Some(start) = python_start_command(ctx, &pyproject, run) {
        commands.insert("start".to_string(), start);
    }

    if ctx.file_exists("tests") || ctx.file_exists("test") {
        commands.insert("test".to_string(), python_test_command(&pm));
    }

    if let Some(install) = python_package_install_command(ctx, &pm) {
        commands.insert("install".to_string(), install);
    }
}

/// `main.py`, `src/main.py`, a console script from pyproject.toml, or the
/// package's `__main__` module, in that order; nothing when none exists.
fn python_start_command(ctx: &DetectionContext, pyproject: &str, run: &str) -> Option<String> {
    if ctx.file_exists("main.py") {
        return Some(format!("{run}python main.py"));
    }
    if ctx.file_exists("src/main.py") {
        return Some(format!("{run}python -m src.main"));
    }

    let package = pyproject::project_name(pyproject);
    let scripts = pyproject::script_names(pyproject);
    if let Some(script) = package
        .as_ref()
        .and_then(|name| scripts.iter().find(|s| *s == name))
        .or_else(|| scripts.first())
    {
        return Some(format!("{run}{script}"));
    }

    let module = package?.replace(['-', '.'], "_").to_lowercase();
    [
        format!("{module}/__main__.py"),
        format!("src/{module}/__main__.py"),
    ]
    .iter()
    .any(|path| ctx.file_exists(path))
    .then(|| format!("{run}python -m {module}"))
}

/// What a tool prefixes commands with to run them in the project's env.
fn python_run_prefix(pm: &str) -> &'static str {
    match pm {
        "uv" => "uv run ",
        "poetry" => "poetry run ",
        "pipenv" => "pipenv run ",
        "pdm" => "pdm run ",
        "hatch" => "hatch run ",
        _ => "",
    }
}

fn python_package_install_command(ctx: &DetectionContext, pm: &str) -> Option<String> {
    match pm {
        "uv" | "poetry" | "pipenv" | "pdm" => Some(python_install_command(pm)),
        "hatch" => Some("hatch env create".to_string()),
        _ if ctx.file_exists("requirements.txt") => Some(python_install_command(pm)),
        _ if ctx.file_exists("pyproject.toml") || ctx.file_exists("setup.py") => {
            Some("pip install -e .".to_string())
        }
        _ => None,
    }
}

fn python_package_manager(ctx: &DetectionContext) -> String {
    if let Some(result) = ctx.get_result("python") {
        let metadata = match &result.data {
            DetectionData::Language(info) => &info.metadata,
            DetectionData::Monorepo(info) => &info.metadata,
            _ => return "pip".to_string(),
        };
        if let Some(pm) = metadata.get("package_manager").and_then(|v| v.as_str()) {
            return pm.to_string();
        }
    }

    if ctx.file_exists("uv.lock") {
        "uv".to_string()
    } else if ctx.file_exists("poetry.lock") {
        "poetry".to_string()
    } else if ctx.file_exists("Pipfile.lock") {
        "pipenv".to_string()
    } else {
        "pip".to_string()
    }
}

fn python_install_command(pm: &str) -> String {
    match pm {
        "uv" => "uv sync".to_string(),
        "poetry" => "poetry install".to_string(),
        "pipenv" => "pipenv install".to_string(),
        "pdm" => "pdm install".to_string(),
        _ => "pip install -r requirements.txt".to_string(),
    }
}

fn python_test_command(pm: &str) -> String {
    format!("{}pytest", python_run_prefix(pm))
}

fn python_workspace_test_command(pm: &str) -> String {
    match pm {
        "uv" => "uv run pytest".to_string(),
        "poetry" => "poetry run pytest".to_string(),
        _ => "pytest".to_string(),
    }
}

fn python_member_run_command(pm: &str, path: &str, name: &str) -> String {
    match pm {
        "uv" => format!("uv run --package {name} python {path}/main.py"),
        "poetry" => format!("poetry run -C {path} python main.py"),
        _ => format!("python {path}/main.py"),
    }
}

fn extract_redis_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    let mut config_found = false;

    // Check for config files in current directory
    if ctx.file_exists("redis.conf") {
        commands.insert("start".to_string(), "redis-server redis.conf".to_string());
        config_found = true;
    }

    // Check for config files in redis/ subdirectory
    if !config_found && ctx.file_exists("redis") {
        if ctx.file_exists("redis/redis.conf") {
            commands.insert(
                "start".to_string(),
                "redis-server redis/redis.conf".to_string(),
            );
            config_found = true;
        } else if ctx.file_exists("redis/redis.config") {
            commands.insert(
                "start".to_string(),
                "redis-server redis/redis.config".to_string(),
            );
            config_found = true;
        }
    }

    // Check for any redis config files in current directory
    if !config_found && let Ok(entries) = std::fs::read_dir(&ctx.root_path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            if file_name_str.starts_with("redis")
                && (file_name_str.ends_with(".conf") || file_name_str.ends_with(".config"))
            {
                commands.insert(
                    "start".to_string(),
                    format!("redis-server {}", file_name_str),
                );
                config_found = true;
                break;
            }
        }
    }

    // Fallback to generic redis-server if no config found
    if !config_found {
        commands.insert("start".to_string(), "redis-server".to_string());
    }
}

fn extract_traefik_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    let mut config_found = false;

    // Check for config files in current directory
    for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
        if ctx.file_exists(file) {
            commands.insert(
                "start".to_string(),
                format!("traefik --configFile={}", file),
            );
            config_found = true;
            break;
        }
    }

    // Check for config files in traefik/ subdirectory
    if !config_found && ctx.file_exists("traefik") {
        for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
            if ctx.file_exists(format!("traefik/{}", file)) {
                commands.insert(
                    "start".to_string(),
                    format!("traefik --configFile=traefik/{}", file),
                );
                config_found = true;
                break;
            }
        }
    }

    // Check for any traefik config files in current directory
    if !config_found && let Ok(entries) = std::fs::read_dir(&ctx.root_path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            if file_name_str.starts_with("traefik")
                && (file_name_str.ends_with(".yml")
                    || file_name_str.ends_with(".yaml")
                    || file_name_str.ends_with(".toml"))
            {
                commands.insert(
                    "start".to_string(),
                    format!("traefik --configFile={}", file_name_str),
                );
                config_found = true;
                break;
            }
        }
    }

    // Fallback if no config file found
    if !config_found {
        commands.insert("start".to_string(), "traefik".to_string());
    }
}

fn extract_rust_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    if let Some(result) = ctx.get_result("rust")
        && let DetectionData::Monorepo(info) = &result.data
    {
        extract_cargo_workspace_commands(info, commands);
        return;
    }

    extract_cargo_crate_commands(ctx, commands);
}

fn extract_cargo_workspace_commands(info: &MonorepoInfo, commands: &mut HashMap<String, String>) {
    commands.insert("build".to_string(), "cargo build --workspace".to_string());
    commands.insert("test".to_string(), "cargo test --workspace".to_string());
    commands.insert("check".to_string(), "cargo check --workspace".to_string());
    // `cargo run` takes no `--workspace` flag (the command used to be generated
    // and always failed). A plain `cargo run` only works when the workspace
    // root is itself a package; otherwise use the per-member `run-<name>`.
    if info.metadata.contains_key("package_name") {
        commands.insert("run".to_string(), "cargo run".to_string());
    }

    for workspace in &info.workspace_info {
        let name = workspace.name.clone().unwrap_or_else(|| {
            workspace
                .path
                .split('/')
                .next_back()
                .unwrap_or("crate")
                .to_string()
        });
        commands.insert(format!("build-{name}"), format!("cargo build -p {name}"));
        commands.insert(format!("test-{name}"), format!("cargo test -p {name}"));
        commands.insert(format!("run-{name}"), format!("cargo run -p {name}"));
    }
}

fn extract_cargo_crate_commands(ctx: &DetectionContext, commands: &mut HashMap<String, String>) {
    commands.insert("build".to_string(), "cargo build".to_string());
    commands.insert("test".to_string(), "cargo test".to_string());
    commands.insert("check".to_string(), "cargo check".to_string());

    let has_main = ctx.file_exists("src/main.rs");
    let has_lib = ctx.file_exists("src/lib.rs");

    if let Ok(content) = ctx.read_file("Cargo.toml") {
        let has_bin = content.contains("[[bin]]") || content.contains("[bin]");
        let has_lib_section = content.contains("[lib]");

        if has_main || has_bin {
            commands.insert("run".to_string(), "cargo run".to_string());
            commands.insert("start".to_string(), "cargo run --release".to_string());
        } else if has_lib || has_lib_section {
            commands.insert("run".to_string(), "cargo test".to_string());
        } else {
            commands.insert("run".to_string(), "cargo run".to_string());
        }
    } else if has_main {
        commands.insert("run".to_string(), "cargo run".to_string());
        commands.insert("start".to_string(), "cargo run --release".to_string());
    } else if has_lib {
        commands.insert("run".to_string(), "cargo test".to_string());
    } else {
        commands.insert("run".to_string(), "cargo run".to_string());
    }
}

fn suggest_default_command(
    app_type: Option<&str>,
    commands: &HashMap<String, String>,
) -> Option<String> {
    if commands.is_empty() {
        return None;
    }

    match app_type {
        Some("nodejs") | Some("nx") => {
            // For Node/Nx apps, prefer dev server commands
            if commands.contains_key("serve") {
                return Some("serve".to_string());
            }
            if commands.contains_key("start") {
                return Some("start".to_string());
            }
            if commands.contains_key("dev") {
                return Some("dev".to_string());
            }
        }
        Some("python") => {
            if commands.contains_key("start") {
                return Some("start".to_string());
            }
        }
        Some("rust") => {
            if commands.contains_key("run") {
                return Some("run".to_string());
            }
            if commands.contains_key("build") {
                return Some("build".to_string());
            }
        }
        Some("redis") | Some("traefik") if commands.contains_key("start") => {
            return Some("start".to_string());
        }
        _ => {}
    }

    // Fallback: a deterministic, non-destructive choice
    crate::defaults::pick_default_command(Some(commands), &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_local_env_nodejs() {
        let temp_dir = TempDir::new().unwrap();

        // Create package.json with scripts
        let package_json = temp_dir.path().join("package.json");
        let mut file = fs::File::create(&package_json).unwrap();
        file.write_all(
            br#"{
  "name": "test-app",
  "scripts": {
    "start": "node index.js",
    "dev": "nodemon index.js",
    "test": "jest"
  }
}"#,
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        // First detect nodejs
        let nodejs_strategy = crate::strategies::NodeJsStrategy;
        let nodejs_result = nodejs_strategy.detect(&ctx).unwrap();
        ctx.store_result(nodejs_result);

        // Then detect local env
        let strategy = LocalEnvStrategy;
        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "local-env");

        match result.data {
            DetectionData::LocalEnv(info) => {
                assert_eq!(info.commands.len(), 3);
                assert_eq!(
                    info.commands.get("start"),
                    Some(&"npm run start".to_string())
                );
                assert_eq!(info.commands.get("dev"), Some(&"npm run dev".to_string()));
                assert_eq!(info.commands.get("test"), Some(&"npm run test".to_string()));
                // Should suggest "start" as default
                assert_eq!(info.suggested_default, Some("start".to_string()));
            }
            _ => panic!("Expected LocalEnv data"),
        }
    }

    #[test]
    fn test_local_env_python() {
        let temp_dir = TempDir::new().unwrap();

        // Create Python files
        fs::File::create(temp_dir.path().join("main.py")).unwrap();
        fs::File::create(temp_dir.path().join("requirements.txt")).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();

        // First detect python
        let python_strategy = crate::strategies::PythonStrategy;
        let python_result = python_strategy.detect(&ctx).unwrap();
        ctx.store_result(python_result);

        // Then detect local env
        let strategy = LocalEnvStrategy;
        assert!(strategy.can_apply(&ctx));

        let result = strategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::LocalEnv(info) => {
                assert!(info.commands.contains_key("start"));
                assert!(info.commands.contains_key("install"));
                assert_eq!(
                    info.commands.get("start"),
                    Some(&"python main.py".to_string())
                );
                assert_eq!(
                    info.commands.get("install"),
                    Some(&"pip install -r requirements.txt".to_string())
                );
            }
            _ => panic!("Expected LocalEnv data"),
        }
    }

    fn python_commands(dir: &std::path::Path) -> HashMap<String, String> {
        let ctx = DetectionContext::new(dir).unwrap();
        ctx.store_result(crate::strategies::PythonStrategy.detect(&ctx).unwrap());
        match LocalEnvStrategy.detect(&ctx).unwrap().data {
            DetectionData::LocalEnv(info) => info.commands,
            _ => panic!("Expected LocalEnv data"),
        }
    }

    #[test]
    fn pyproject_only_projects_get_commands_for_files_that_exist() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("pyproject.toml"),
            "[project]\nname = \"svc\"\n\n[build-system]\nbuild-backend = \"setuptools.build_meta\"\n",
        )
        .unwrap();
        fs::create_dir(temp_dir.path().join("lib")).unwrap();
        fs::write(temp_dir.path().join("lib/util.py"), "").unwrap();

        let commands = python_commands(temp_dir.path());
        assert_eq!(
            commands.get("install").map(String::as_str),
            Some("pip install -e .")
        );
        assert!(
            !commands.contains_key("start"),
            "no main.py, script or __main__: {commands:?}"
        );
    }

    #[test]
    fn hatch_projects_start_their_console_script() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("pyproject.toml"),
            r#"[project]
name = "svc"

[project.scripts]
svc-admin = "svc.admin:main"
svc = "svc.main:run"

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"
"#,
        )
        .unwrap();

        let commands = python_commands(temp_dir.path());
        assert_eq!(
            commands.get("start").map(String::as_str),
            Some("hatch run svc")
        );
        assert_eq!(
            commands.get("install").map(String::as_str),
            Some("hatch env create")
        );
    }

    #[test]
    fn pdm_projects_run_their_main_module() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("pyproject.toml"),
            "[project]\nname = \"my-svc\"\n\n[tool.pdm]\ndistribution = true\n",
        )
        .unwrap();
        fs::create_dir_all(temp_dir.path().join("src/my_svc")).unwrap();
        fs::write(temp_dir.path().join("src/my_svc/__main__.py"), "").unwrap();

        let commands = python_commands(temp_dir.path());
        assert_eq!(
            commands.get("start").map(String::as_str),
            Some("pdm run python -m my_svc")
        );
        assert_eq!(
            commands.get("install").map(String::as_str),
            Some("pdm install")
        );
    }

    #[test]
    fn test_local_env_nodejs_pnpm() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join("package.json"),
            r#"{"name":"app","packageManager":"pnpm@9.0.0","scripts":{"dev":"vite"}}"#,
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let nodejs_result = crate::strategies::NodeJsStrategy.detect(&ctx).unwrap();
        ctx.store_result(nodejs_result);

        let result = LocalEnvStrategy.detect(&ctx).unwrap();
        match result.data {
            DetectionData::LocalEnv(info) => {
                assert_eq!(info.commands.get("dev"), Some(&"pnpm run dev".to_string()));
            }
            _ => panic!("Expected LocalEnv data"),
        }
    }

    #[test]
    fn test_local_env_rust_workspace() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("crates/api/src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/api\"]\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("crates/api/Cargo.toml"),
            "[package]\nname = \"api\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let rust_result = crate::strategies::RustStrategy.detect(&ctx).unwrap();
        ctx.store_result(rust_result);

        let result = LocalEnvStrategy.detect(&ctx).unwrap();
        match result.data {
            DetectionData::LocalEnv(info) => {
                assert_eq!(
                    info.commands.get("build"),
                    Some(&"cargo build --workspace".to_string())
                );
                assert!(info.commands.contains_key("run-api"));
            }
            _ => panic!("Expected LocalEnv data"),
        }
    }

    #[test]
    fn test_local_env_python_uv_workspace() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("packages/api")).unwrap();
        fs::write(
            temp_dir.path().join("pyproject.toml"),
            "[tool.uv.workspace]\nmembers = [\"packages/api\"]\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("packages/api/pyproject.toml"),
            "name = \"api\"\n",
        )
        .unwrap();
        fs::write(temp_dir.path().join("uv.lock"), "").unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let python_result = crate::strategies::PythonStrategy.detect(&ctx).unwrap();
        ctx.store_result(python_result);

        let result = LocalEnvStrategy.detect(&ctx).unwrap();
        match result.data {
            DetectionData::LocalEnv(info) => {
                assert_eq!(info.commands.get("install"), Some(&"uv sync".to_string()));
                assert!(info.commands.contains_key("run-api"));
            }
            _ => panic!("Expected LocalEnv data"),
        }
    }

    #[test]
    fn test_local_env_no_app_type() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = LocalEnvStrategy;

        // Should not apply without any app type detected
        assert!(!strategy.can_apply(&ctx));
    }
}
