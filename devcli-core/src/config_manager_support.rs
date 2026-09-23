//! Adapter between devcli-core and the `config-manager` crate.
//!
//! Phase 1: JSON load/save via `ConfigManager` + `JsonLoader`.
//! Phase 2: `ProjectAppResolver` for project/app lookup + fuzzy suggestions.
//! Phase 3: `AppDependencyProvider` + `DependencyGraph` for dep chains.
//! Phase 4: `DevCliConfigValidator` for `config validate`.
//! Phase 5: preferences load/save via `JsonLoader`.
//! Path discovery stays in `config::loader`.

use crate::config::models::{App, Config, Preferences};
use crate::config::resolver::ResolvedApp;
use crate::Result;
use config_manager::dependencies::{DependencyGraph, DependencyProvider};
use config_manager::loader::ConfigLoader;
use config_manager::resolver::{fuzzy_match, Resolver};
use config_manager::utils::{expand_path, expand_tilde};
use config_manager::validation::{
    ValidationError, ValidationResult, ValidationWarning, Validator,
};
use config_manager::{ConfigManager, Error as CmError, JsonLoader};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Thin wrapper around `ConfigManager<Config>` for the main config.json.
pub struct DevCliConfigManager {
    manager: ConfigManager<Config>,
    path: PathBuf,
}

impl DevCliConfigManager {
    /// Build a manager for an already-resolved config file path.
    pub fn for_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let loader = JsonLoader::new(&path)
            .with_path_expansion(false)
            .with_pretty_print(true)
            .with_create_dirs(true);

        let manager = ConfigManager::<Config>::builder()
            .loader(loader)
            .build()
            .map_err(|err| anyhow::anyhow!("{err}"))?;

        Ok(Self { manager, path })
    }

    /// Path this manager reads/writes.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the config file exists.
    pub fn exists(&self) -> bool {
        self.manager.exists()
    }

    /// Load and parse config.json, expanding `~` in app paths.
    pub fn load(&self) -> Result<Config> {
        if !self.exists() {
            anyhow::bail!(
                "Config file not found at {}. Run 'devcli config init' to create one.",
                self.path.display()
            );
        }

        let mut config = self
            .manager
            .load()
            .map_err(|err| anyhow::anyhow!("{err}"))?;

        expand_app_paths(&mut config);
        Ok(config)
    }

    /// Persist config.json (pretty-printed).
    pub fn save(&self, config: &Config) -> Result<()> {
        self.manager
            .save(config)
            .map_err(|err| anyhow::anyhow!("{err}"))
    }
}

/// Load preferences.json via JsonLoader, or return defaults if missing.
pub fn load_preferences(path: impl AsRef<Path>) -> Result<Preferences> {
    let loader = preferences_loader(path.as_ref());
    if !ConfigLoader::<Preferences>::exists(&loader) {
        return Ok(Preferences::default());
    }
    ConfigLoader::<Preferences>::load(&loader).map_err(|err| anyhow::anyhow!("{err}"))
}

/// Save preferences.json via JsonLoader.
pub fn save_preferences(path: impl AsRef<Path>, prefs: &Preferences) -> Result<()> {
    let loader = preferences_loader(path.as_ref());
    ConfigLoader::<Preferences>::save(&loader, prefs).map_err(|err| anyhow::anyhow!("{err}"))
}

fn preferences_loader(path: &Path) -> JsonLoader {
    JsonLoader::new(path)
        .with_path_expansion(false)
        .with_pretty_print(true)
        .with_create_dirs(true)
}

/// Expand tilde paths in all app configurations (product-layer post-process).
fn expand_app_paths(config: &mut Config) {
    for project in config.projects.values_mut() {
        for app in project.apps.values_mut() {
            let expanded_path = expand_tilde(&app.path);
            app.path = expanded_path.to_string_lossy().to_string();
        }
    }
}

/// Hierarchical project/app resolver backed by config-manager's `Resolver` trait.
///
/// Identifiers:
/// - `app` — search all projects (by name or `alternative_name`)
/// - `project/app` — resolve within a specific project
pub struct ProjectAppResolver;

impl ProjectAppResolver {
    pub fn new() -> Self {
        Self
    }

    /// Resolve with an optional project filter (devcli CLI `--project`).
    pub fn resolve_with_filter(
        &self,
        config: &Config,
        app_name: &str,
        project_filter: Option<&str>,
    ) -> std::result::Result<ResolvedApp, CmError> {
        if let Some(project) = project_filter {
            return self.resolve_in_project(config, project, app_name);
        }
        self.resolve(config, app_name)
    }

    fn resolve_in_project(
        &self,
        config: &Config,
        project: &str,
        app_name: &str,
    ) -> std::result::Result<ResolvedApp, CmError> {
        let proj = config
            .projects
            .get(project)
            .ok_or_else(|| CmError::NotFound {
                id: project.to_string(),
                suggestion: None,
            })?;

        // Prefer exact app key, then alternative_name within this project
        if let Some(app) = proj.apps.get(app_name) {
            return Ok(ResolvedApp {
                project: project.to_string(),
                app_name: app_name.to_string(),
                app: app.clone(),
            });
        }

        for (actual_name, app) in &proj.apps {
            if app.alternative_name.as_deref() == Some(app_name) {
                return Ok(ResolvedApp {
                    project: project.to_string(),
                    app_name: actual_name.clone(),
                    app: app.clone(),
                });
            }
        }

        Err(CmError::NotFound {
            id: app_name.to_string(),
            suggestion: self.suggest(config, app_name),
        })
    }

    fn collect_matches(&self, config: &Config, app_name: &str) -> Vec<(String, String, App)> {
        let mut matches = Vec::new();
        for (project_name, project) in &config.projects {
            for (actual_app_name, app) in &project.apps {
                let name_matches = actual_app_name == app_name;
                let alt_matches = app.alternative_name.as_deref() == Some(app_name);
                if name_matches || alt_matches {
                    matches.push((
                        project_name.clone(),
                        actual_app_name.clone(),
                        app.clone(),
                    ));
                }
            }
        }
        matches
    }

    fn all_app_names(&self, config: &Config) -> Vec<String> {
        let mut names: Vec<String> = config
            .projects
            .values()
            .flat_map(|p| {
                p.apps.iter().flat_map(|(name, app)| {
                    let mut ids = vec![name.clone()];
                    if let Some(alt) = &app.alternative_name {
                        ids.push(alt.clone());
                    }
                    ids
                })
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

impl Default for ProjectAppResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver<Config, ResolvedApp> for ProjectAppResolver {
    fn resolve(&self, config: &Config, id: &str) -> config_manager::Result<ResolvedApp> {
        if let Some((project, app)) = id.split_once('/') {
            return self.resolve_in_project(config, project, app);
        }

        let matches = self.collect_matches(config, id);
        match matches.len() {
            0 => Err(CmError::NotFound {
                id: id.to_string(),
                suggestion: self.suggest(config, id),
            }),
            1 => {
                let (project, app_name, app) = matches.into_iter().next().unwrap();
                Ok(ResolvedApp {
                    project,
                    app_name,
                    app,
                })
            }
            _ => {
                let candidates: Vec<String> = matches.iter().map(|(p, _, _)| p.clone()).collect();
                Err(CmError::Ambiguous {
                    id: id.to_string(),
                    candidates,
                })
            }
        }
    }

    fn list_ids(&self, config: &Config) -> Vec<String> {
        let mut ids = Vec::new();
        for (project_name, project) in &config.projects {
            for app_name in project.apps.keys() {
                ids.push(format!("{project_name}/{app_name}"));
            }
        }
        ids.sort();
        ids
    }

    fn suggest(&self, config: &Config, id: &str) -> Option<String> {
        fuzzy_match(id, &self.all_app_names(config), 2)
    }
}

/// Map config-manager resolution errors to the public CLI messages.
pub fn map_resolve_error(err: CmError, app_name: &str) -> anyhow::Error {
    match err {
        CmError::NotFound { suggestion, .. } => {
            if let Some(similar) = suggestion {
                anyhow::anyhow!(
                    "App '{}' not found in config. Did you mean '{}'?",
                    app_name,
                    similar
                )
            } else {
                anyhow::anyhow!("App '{}' not found in config.", app_name)
            }
        }
        CmError::Ambiguous { candidates, .. } => anyhow::anyhow!(
            "App name '{}' is ambiguous. Found in projects: {}. Use --project to specify.",
            app_name,
            candidates.join(", ")
        ),
        other => anyhow::anyhow!("{other}"),
    }
}

/// Extracts `project/app` dependency ids from a resolved app.
pub struct AppDependencyProvider;

impl DependencyProvider<ResolvedApp> for AppDependencyProvider {
    fn dependencies(&self, entity: &ResolvedApp) -> Vec<String> {
        entity
            .app
            .dependencies
            .iter()
            .map(|dep| format!("{}/{}", dep.project, dep.app))
            .collect()
    }
}

/// Build a dependency graph wired to ProjectAppResolver.
pub fn dependency_graph() -> DependencyGraph<Config, ResolvedApp> {
    DependencyGraph::new(
        Box::new(AppDependencyProvider),
        Box::new(ProjectAppResolver::new()),
        Box::new(|resolved: &ResolvedApp| format!("{}/{}", resolved.project, resolved.app_name)),
    )
}

/// Resolve transitive dependencies (excluding the root app), deps-first.
///
/// Returns `CircularDependency` when a cycle is present (stricter than the
/// legacy BFS, which silently truncated cycles).
pub fn resolve_dependency_chain(
    config: &Config,
    resolved_app: &ResolvedApp,
) -> Result<Vec<ResolvedApp>> {
    let graph = dependency_graph();
    let chain = graph
        .resolve_chain(config, resolved_app)
        .map_err(map_dependency_error)?;

    Ok(chain
        .into_iter()
        .filter(|app| {
            !(app.project == resolved_app.project && app.app_name == resolved_app.app_name)
        })
        .collect())
}

fn map_dependency_error(err: CmError) -> anyhow::Error {
    match err {
        CmError::CircularDependency { path } => anyhow::anyhow!(
            "Circular dependency detected: {}. Check app dependencies in config.",
            path.join(" -> ")
        ),
        CmError::NotFound { id, suggestion } => {
            if let Some((project, app)) = id.split_once('/') {
                anyhow::anyhow!("App '{}' not found in project '{}'.", app, project)
            } else if let Some(similar) = suggestion {
                anyhow::anyhow!(
                    "App '{}' not found in config. Did you mean '{}'?",
                    id,
                    similar
                )
            } else {
                anyhow::anyhow!("App '{}' not found in config.", id)
            }
        }
        other => anyhow::anyhow!("{other}"),
    }
}

/// Product validation rules for `devcli config validate`.
pub struct DevCliConfigValidator;

impl DevCliConfigValidator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DevCliConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator<Config> for DevCliConfigValidator {
    fn validate(&self, config: &Config) -> ValidationResult {
        let mut result = ValidationResult::new();
        let mut seen_app_names: HashSet<String> = HashSet::new();

        for (project_name, project) in &config.projects {
            for (app_name, app) in &project.apps {
                let field = format!("{project_name}/{app_name}");

                if seen_app_names.contains(app_name) {
                    result.add_warning(ValidationWarning::new(
                        field.clone(),
                        format!(
                            "App name '{app_name}' is not unique - you'll need to use --project flag"
                        ),
                    ));
                }
                seen_app_names.insert(app_name.clone());

                validate_app(&mut result, &field, config, app);
            }
        }

        // Second pass: dependency chains / cycles
        for (project_name, project) in &config.projects {
            for app_name in project.apps.keys() {
                let field = format!("{project_name}/{app_name}");
                match ProjectAppResolver::new().resolve_with_filter(
                    config,
                    app_name,
                    Some(project_name),
                ) {
                    Ok(resolved) => {
                        if let Err(e) = resolve_dependency_chain(config, &resolved) {
                            result.add_error(ValidationError::new(
                                field,
                                format!("Dependency resolution error: {e}"),
                            ));
                        }
                    }
                    Err(e) => {
                        result.add_error(ValidationError::new(
                            field,
                            format!("Failed to resolve app: {e}"),
                        ));
                    }
                }
            }
        }

        result
    }
}

fn validate_app(result: &mut ValidationResult, field: &str, config: &Config, app: &App) {
    let path = expand_path(&app.path);
    if !path.exists() {
        result.add_error(ValidationError::new(
            field,
            format!("Path does not exist: {}", path.display()),
        ));
    }

    let has_local = app
        .commands
        .local
        .as_ref()
        .map(|m| !m.is_empty())
        .unwrap_or(false);
    let has_docker = app
        .commands
        .docker
        .as_ref()
        .map(|m| !m.is_empty())
        .unwrap_or(false);

    if !has_local && !has_docker {
        result.add_error(ValidationError::new(
            field,
            "No commands defined (need at least one environment)",
        ));
    }

    validate_env_defaults(result, field, "local", &app.commands.local, &app.defaults.local);
    validate_env_defaults(
        result,
        field,
        "docker",
        &app.commands.docker,
        &app.defaults.docker,
    );

    for dep in &app.dependencies {
        if let Err(e) =
            crate::config::resolver::get_app_by_project(config, &dep.project, &dep.app)
        {
            result.add_error(ValidationError::new(
                field,
                format!("Invalid dependency {}/{}: {e}", dep.project, dep.app),
            ));
        }
    }
}

fn validate_env_defaults(
    result: &mut ValidationResult,
    field: &str,
    env: &str,
    commands: &Option<std::collections::HashMap<String, String>>,
    default: &Option<String>,
) {
    let Some(commands) = commands else {
        return;
    };
    if commands.is_empty() {
        return;
    }

    match default {
        Some(default_cmd) if !commands.contains_key(default_cmd) => {
            result.add_error(ValidationError::new(
                field,
                format!("Default {env} command '{default_cmd}' not found in {env} commands"),
            ));
        }
        None => {
            result.add_error(ValidationError::new(
                field,
                format!("{env} commands defined but no defaults.{env} specified"),
            ));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{AppBuilder, ConfigBuilder};
    use config_manager::resolver::Resolver;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn load_and_save_roundtrip() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.json");

        let config = ConfigBuilder::new()
            .with_app(
                "infra",
                "redis",
                AppBuilder::new("redis", "/tmp/redis")
                    .with_local_command("start", "redis-server")
                    .with_local_default("start")
                    .build(),
            )
            .build();

        let manager = DevCliConfigManager::for_path(&path).unwrap();
        manager.save(&config).unwrap();
        assert!(path.exists());

        let loaded = manager.load().unwrap();
        assert!(loaded.projects.contains_key("infra"));
        assert_eq!(loaded.projects["infra"].apps["redis"].path, "/tmp/redis");
    }

    #[test]
    fn load_missing_file_has_helpful_message() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("missing.json");
        let manager = DevCliConfigManager::for_path(&path).unwrap();

        let err = manager.load().unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Config file not found"));
        assert!(msg.contains("devcli config init"));
    }

    #[test]
    fn load_expands_tilde_in_app_paths() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.json");

        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let json = r#"{
              "projects": {
                "p": {
                  "apps": {
                    "api": {
                      "type": "nodejs",
                      "path": "~/my-app",
                      "commands": { "local": { "start": "npm start" } },
                      "defaults": { "local": "start" },
                      "dependencies": []
                    }
                  }
                }
              }
            }"#;
        fs::write(&path, json).unwrap();

        let manager = DevCliConfigManager::for_path(&path).unwrap();
        let config = manager.load().unwrap();
        let app_path = &config.projects["p"].apps["api"].path;
        assert_eq!(app_path, &format!("{home}/my-app"));
    }

    #[test]
    fn resolver_unique_app() {
        let config = ConfigBuilder::new()
            .with_app(
                "infra",
                "redis",
                AppBuilder::new("redis", "/tmp/redis").build(),
            )
            .build();

        let resolved = ProjectAppResolver::new()
            .resolve(&config, "redis")
            .unwrap();
        assert_eq!(resolved.project, "infra");
        assert_eq!(resolved.app_name, "redis");
    }

    #[test]
    fn resolver_project_slash_app() {
        let config = ConfigBuilder::new()
            .with_app(
                "infra",
                "redis",
                AppBuilder::new("redis", "/tmp/redis").build(),
            )
            .build();

        let resolved = ProjectAppResolver::new()
            .resolve(&config, "infra/redis")
            .unwrap();
        assert_eq!(resolved.project, "infra");
    }

    #[test]
    fn resolver_fuzzy_suggestion() {
        let config = ConfigBuilder::new()
            .with_app(
                "infra",
                "redis",
                AppBuilder::new("redis", "/tmp/redis").build(),
            )
            .build();

        let err = ProjectAppResolver::new()
            .resolve(&config, "reds")
            .unwrap_err();
        match err {
            CmError::NotFound {
                suggestion: Some(s),
                ..
            } => assert_eq!(s, "redis"),
            other => panic!("expected NotFound with suggestion, got {other:?}"),
        }
    }

    #[test]
    fn resolver_ambiguous() {
        let config = ConfigBuilder::new()
            .with_app("p1", "api", AppBuilder::new("nodejs", "/tmp/a").build())
            .with_app("p2", "api", AppBuilder::new("nodejs", "/tmp/b").build())
            .build();

        let err = ProjectAppResolver::new()
            .resolve(&config, "api")
            .unwrap_err();
        match err {
            CmError::Ambiguous { candidates, .. } => {
                assert_eq!(candidates.len(), 2);
            }
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn resolver_list_ids() {
        let config = ConfigBuilder::new()
            .with_app("infra", "redis", AppBuilder::new("redis", "/tmp/r").build())
            .with_app("api", "web", AppBuilder::new("nodejs", "/tmp/w").build())
            .build();

        let ids = ProjectAppResolver::new().list_ids(&config);
        assert_eq!(ids, vec!["api/web".to_string(), "infra/redis".to_string()]);
    }

    #[test]
    fn validator_accepts_valid_config() {
        let temp = TempDir::new().unwrap();
        let app_dir = temp.path().join("app");
        fs::create_dir_all(&app_dir).unwrap();

        let config = ConfigBuilder::new()
            .with_app(
                "p",
                "api",
                AppBuilder::new("nodejs", app_dir.to_str().unwrap())
                    .with_local_command("start", "npm start")
                    .with_local_default("start")
                    .build(),
            )
            .build();

        let result = DevCliConfigValidator::new().validate(&config);
        assert!(result.is_valid(), "{result}");
    }

    #[test]
    fn validator_flags_missing_commands() {
        let temp = TempDir::new().unwrap();
        let app_dir = temp.path().join("app");
        fs::create_dir_all(&app_dir).unwrap();

        let config = ConfigBuilder::new()
            .with_app(
                "p",
                "api",
                AppBuilder::new("nodejs", app_dir.to_str().unwrap()).build(),
            )
            .build();

        let result = DevCliConfigValidator::new().validate(&config);
        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.message().contains("No commands defined")));
    }

    #[test]
    fn validator_flags_cycle() {
        let temp = TempDir::new().unwrap();
        let a_dir = temp.path().join("a");
        let b_dir = temp.path().join("b");
        fs::create_dir_all(&a_dir).unwrap();
        fs::create_dir_all(&b_dir).unwrap();

        let config = ConfigBuilder::new()
            .with_app(
                "test",
                "a",
                AppBuilder::new("nodejs", a_dir.to_str().unwrap())
                    .with_local_command("start", "npm start")
                    .with_local_default("start")
                    .with_dependency("test", "b")
                    .build(),
            )
            .with_app(
                "test",
                "b",
                AppBuilder::new("nodejs", b_dir.to_str().unwrap())
                    .with_local_command("start", "npm start")
                    .with_local_default("start")
                    .with_dependency("test", "a")
                    .build(),
            )
            .build();

        let result = DevCliConfigValidator::new().validate(&config);
        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.message().contains("Circular dependency")));
    }

    #[test]
    fn validator_warns_duplicate_names() {
        let temp = TempDir::new().unwrap();
        let a_dir = temp.path().join("a");
        let b_dir = temp.path().join("b");
        fs::create_dir_all(&a_dir).unwrap();
        fs::create_dir_all(&b_dir).unwrap();

        let config = ConfigBuilder::new()
            .with_app(
                "p1",
                "api",
                AppBuilder::new("nodejs", a_dir.to_str().unwrap())
                    .with_local_command("start", "npm start")
                    .with_local_default("start")
                    .build(),
            )
            .with_app(
                "p2",
                "api",
                AppBuilder::new("nodejs", b_dir.to_str().unwrap())
                    .with_local_command("start", "npm start")
                    .with_local_default("start")
                    .build(),
            )
            .build();

        let result = DevCliConfigValidator::new().validate(&config);
        assert!(result.is_valid());
        assert!(result.warning_count() >= 1);
    }
}
