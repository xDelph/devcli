# Migration Guide

This guide helps you migrate to config-manager from other configuration systems.

## Table of Contents

- [From devcli-core](#from-devcli-core)
- [From config-rs](#from-config-rs)
- [From figment](#from-figment)
- [From custom JSON loading](#from-custom-json-loading)

## From devcli-core

If you're migrating from devcli-core's built-in config system to config-manager, follow these steps.

### Phase 1: Add config-manager Dependency

Update `Cargo.toml`:

```toml
[dependencies]
config-manager = { path = "../config-manager" }
```

### Phase 2: Keep Old Types (Compatibility Layer)

Keep your existing config types temporarily:

```rust
// devcli-core/src/config/models.rs (keep for now)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub projects: HashMap<String, Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub apps: HashMap<String, App>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    pub app_type: String,
    pub path: String,
    pub commands: Commands,
    pub dependencies: Vec<Dependency>,
    pub defaults: Defaults,
    // ... other fields
}
```

### Phase 3: Create Adapter

Create an adapter to maintain backward compatibility:

```rust
// devcli-core/src/config/adapter.rs

use config_manager::prelude::*;
use crate::config::models::{Config, App, Project};
use crate::process::ProcessTracker;

pub struct DevCliConfigManager {
    manager: ConfigManager<Config>,
}

impl DevCliConfigManager {
    pub fn new(config_path: &str) -> Result<Self> {
        let manager = ConfigManager::<Config>::builder()
            .loader(JsonLoader::new(config_path))
            .build()?;

        Ok(Self { manager })
    }

    // Maintain old API
    pub fn load_config(&self) -> Result<Config> {
        self.manager.load()
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        self.manager.save(config)
    }
}
```

### Phase 4: Update Loaders

Replace old loader functions:

```rust
// OLD
pub fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;
    let contents = fs::read_to_string(&config_path)?;
    let mut config: Config = serde_json::from_str(&contents)?;
    // Path expansion...
    Ok(config)
}

// NEW
pub fn load_config() -> Result<Config> {
    let adapter = DevCliConfigManager::new("~/.devcli/config.json")?;
    adapter.load_config()
}
```

### Phase 5: Implement Custom Resolver

For devcli's hierarchical resolution (project/app lookup):

```rust
use config_manager::resolver::Resolver;
use config_manager::core::{Error, Result};

pub struct ProjectAppResolver;

impl Resolver<Config, ResolvedApp> for ProjectAppResolver {
    fn resolve(&self, config: &Config, id: &str) -> Result<ResolvedApp> {
        // Parse id: can be "app-name" or "project/app-name"
        if let Some((project, app)) = id.split_once('/') {
            self.resolve_in_project(config, project, app)
        } else {
            self.resolve_across_projects(config, id)
        }
    }

    fn list_ids(&self, config: &Config) -> Vec<String> {
        let mut ids = Vec::new();
        for (project_name, project) in &config.projects {
            for app_name in project.apps.keys() {
                ids.push(format!("{}/{}", project_name, app_name));
            }
        }
        ids
    }
}

impl ProjectAppResolver {
    fn resolve_in_project(&self, config: &Config, project: &str, app: &str) -> Result<ResolvedApp> {
        let proj = config.projects.get(project)
            .ok_or_else(|| Error::NotFound {
                id: project.to_string(),
                suggestion: None,
            })?;

        let app_config = proj.apps.get(app)
            .ok_or_else(|| Error::NotFound {
                id: app.to_string(),
                suggestion: None,
            })?;

        Ok(ResolvedApp {
            project: project.to_string(),
            app_name: app.to_string(),
            app: app_config.clone(),
        })
    }

    fn resolve_across_projects(&self, config: &Config, app_name: &str) -> Result<ResolvedApp> {
        let mut matches = Vec::new();

        for (project_name, project) in &config.projects {
            if let Some(app) = project.apps.get(app_name) {
                matches.push((project_name.clone(), app.clone()));
            }
        }

        match matches.len() {
            0 => Err(Error::NotFound {
                id: app_name.to_string(),
                suggestion: self.suggest_app(config, app_name),
            }),
            1 => {
                let (project, app) = matches.into_iter().next().unwrap();
                Ok(ResolvedApp {
                    project,
                    app_name: app_name.to_string(),
                    app,
                })
            }
            _ => {
                let candidates: Vec<String> = matches.iter()
                    .map(|(p, _)| p.clone())
                    .collect();
                Err(Error::Ambiguous {
                    id: app_name.to_string(),
                    candidates,
                })
            }
        }
    }

    fn suggest_app(&self, config: &Config, target: &str) -> Option<String> {
        use config_manager::resolver::fuzzy_match;

        let all_apps: Vec<String> = config.projects.values()
            .flat_map(|p| p.apps.keys().cloned())
            .collect();

        fuzzy_match(target, &all_apps, 2)
    }
}
```

### Phase 6: Implement Dependency Resolution

Adapt devcli's dependency system:

```rust
use config_manager::dependencies::{DependencyGraph, DependencyProvider};

pub struct AppDependencyProvider;

impl DependencyProvider<App> for AppDependencyProvider {
    fn dependencies(&self, entity: &App) -> Vec<String> {
        entity.dependencies.iter()
            .map(|dep| format!("{}/{}", dep.project, dep.app))
            .collect()
    }
}

// Create graph
let graph = DependencyGraph::new(
    Box::new(AppDependencyProvider),
    Box::new(ProjectAppResolver),
    Box::new(|app: &App| format!("{}/{}", /* project */, app.app_type)),
);

// Use for dependency resolution
pub fn resolve_dependency_chain(
    config: &Config,
    resolved_app: &ResolvedApp,
) -> Result<Vec<ResolvedApp>> {
    let graph = /* ... */;
    graph.resolve_chain(config, resolved_app)
}
```

### Phase 7: Update Commands

Update commands to use new API:

```rust
// OLD
use crate::config::loader::load_config;

pub fn start_command(args: StartArgs) -> Result<()> {
    let config = load_config()?;
    // ...
}

// NEW
use crate::config::adapter::DevCliConfigManager;

pub fn start_command(args: StartArgs) -> Result<()> {
    let manager = DevCliConfigManager::new("~/.devcli/config.json")?;
    let config = manager.load_config()?;
    // ...
}
```

### Phase 8: Add Validation

Add validators for devcli config:

```rust
use config_manager::validation::{Validator, ValidationResult, ValidationError};

pub struct DevCliConfigValidator;

impl Validator<Config> for DevCliConfigValidator {
    fn validate(&self, config: &Config) -> ValidationResult {
        let mut result = ValidationResult::new();

        for (project_name, project) in &config.projects {
            for (app_name, app) in &project.apps {
                // Validate app type
                if app.app_type.is_empty() {
                    result.add_error(ValidationError::new(
                        format!("{}.{}.type", project_name, app_name),
                        "App type cannot be empty"
                    ));
                }

                // Validate path exists
                if !std::path::Path::new(&app.path).exists() {
                    result.add_warning(ValidationWarning::new(
                        format!("{}.{}.path", project_name, app_name),
                        format!("Path does not exist: {}", app.path)
                    ));
                }

                // Validate defaults point to existing commands
                // ... more validation
            }
        }

        result
    }
}

// Add to manager
let manager = ConfigManager::<Config>::builder()
    .loader(JsonLoader::new("~/.devcli/config.json"))
    .validator(Box::new(DevCliConfigValidator))
    .build()?;
```

### Phase 9: Remove Old Code

Once everything works:

1. Remove old `loader.rs`
2. Remove old `resolver.rs`
3. Remove old `dependencies.rs`
4. Update imports throughout codebase

### Phase 10: Optimize

Add optimizations now available with config-manager:

```rust
// Cache manager instance
lazy_static! {
    static ref CONFIG_MANAGER: DevCliConfigManager =
        DevCliConfigManager::new("~/.devcli/config.json")
            .expect("Failed to create config manager");
}

// Use singleton
pub fn load_config() -> Result<Config> {
    CONFIG_MANAGER.load_config()
}
```

## From config-rs

If you're using [config-rs](https://github.com/mehcode/config-rs):

### Key Differences

| Feature | config-rs | config-manager |
|---------|-----------|----------------|
| Loading | Builder pattern | ConfigManager + Loader |
| Merging | Built-in | Manual (planned) |
| Environments | Built-in | Manual |
| Resolution | None | Built-in |
| Dependencies | None | Built-in |
| Validation | None | Built-in |

### Migration Steps

```rust
// OLD (config-rs)
use config::{Config, File, Environment};

let config = Config::builder()
    .add_source(File::with_name("config"))
    .add_source(Environment::with_prefix("APP"))
    .build()?;

let port: i64 = config.get("port")?;

// NEW (config-manager)
use config_manager::prelude::*;

#[derive(Deserialize)]
struct AppConfig {
    port: i64,
}

let manager = ConfigManager::<AppConfig>::builder()
    .loader(JsonLoader::new("config.json"))
    .build()?;

let config = manager.load()?;
let port = config.port;
```

### Handling Merged Configs

```rust
// config-rs style merging
// NOT YET SUPPORTED - Coming in v0.2

// Workaround: Manual merge
let base = base_manager.load()?;
let overrides = override_manager.load()?;
let merged = merge_configs(base, overrides);
```

## From figment

If you're using [figment](https://github.com/SergioBenitez/Figment):

### Key Differences

| Feature | figment | config-manager |
|---------|---------|----------------|
| Loading | Provider pattern | ConfigManager + Loader |
| Profiles | Built-in | Manual |
| Extraction | Partial | Full struct |
| Resolution | None | Built-in |
| Dependencies | None | Built-in |

### Migration Steps

```rust
// OLD (figment)
use figment::{Figment, providers::{Format, Json, Env}};

let config: AppConfig = Figment::new()
    .merge(Json::file("config.json"))
    .merge(Env::prefixed("APP_"))
    .extract()?;

// NEW (config-manager)
let manager = ConfigManager::<AppConfig>::builder()
    .loader(JsonLoader::new("config.json"))
    .build()?;

let config = manager.load()?;

// Manual env var overlay
if let Ok(port) = std::env::var("APP_PORT") {
    config.port = port.parse()?;
}
```

## From Custom JSON Loading

If you have custom JSON loading code:

### Before

```rust
use std::fs;
use serde_json;

pub fn load_config(path: &str) -> Result<AppConfig> {
    let contents = fs::read_to_string(path)?;
    let config = serde_json::from_str(&contents)?;
    Ok(config)
}

pub fn save_config(path: &str, config: &AppConfig) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)?;
    Ok(())
}
```

### After

```rust
use config_manager::prelude::*;

pub fn load_config(path: &str) -> Result<AppConfig> {
    let manager = ConfigManager::<AppConfig>::builder()
        .loader(JsonLoader::new(path))
        .build()?;
    manager.load()
}

pub fn save_config(path: &str, config: &AppConfig) -> Result<()> {
    let manager = ConfigManager::<AppConfig>::builder()
        .loader(JsonLoader::new(path))
        .build()?;
    manager.save(config)
}
```

### Or Create Singleton

```rust
use once_cell::sync::Lazy;

static CONFIG_MANAGER: Lazy<ConfigManager<AppConfig>> = Lazy::new(|| {
    ConfigManager::<AppConfig>::builder()
        .loader(JsonLoader::new("config.json"))
        .build()
        .expect("Failed to create config manager")
});

pub fn load_config() -> Result<AppConfig> {
    CONFIG_MANAGER.load()
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    CONFIG_MANAGER.save(config)
}
```

## Common Migration Patterns

### Environment-Specific Configs

```rust
// Load based on environment
let env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
let config_file = format!("config.{}.json", env);

let manager = ConfigManager::<AppConfig>::builder()
    .loader(JsonLoader::new(&config_file))
    .build()?;
```

### Config with Defaults

```rust
// Load or create default
let manager = ConfigManager::<AppConfig>::builder()
    .loader(JsonLoader::new("config.json"))
    .build()?;

let config = if manager.exists() {
    manager.load()?
} else {
    let default = AppConfig::default();
    manager.save(&default)?;
    default
};
```

### Validated Loading

```rust
// Always validate on load
let config = manager.load_and_validate()?;
```

## Troubleshooting

### Error: "Loader is required"

You forgot to set a loader:

```rust
// Wrong
let manager = ConfigManager::<Config>::builder().build()?;

// Correct
let manager = ConfigManager::<Config>::builder()
    .loader(JsonLoader::new("config.json"))
    .build()?;
```

### Error: "Failed to load configuration"

Check the file path and permissions:

```rust
let manager = ConfigManager::<Config>::builder()
    .loader(JsonLoader::new("config.json"))
    .build()?;

if !manager.exists() {
    eprintln!("Config not found at: {}", manager.source_info());
}
```

### Type Mismatch Errors

Make sure your struct matches the JSON structure:

```rust
// JSON
{
    "port": 3000,
    "name": "myapp"
}

// Struct must match
#[derive(Deserialize)]
struct Config {
    port: u16,  // Correct type
    name: String,  // Correct type
}
```

### Path Not Expanding

Enable path expansion:

```rust
let loader = JsonLoader::new("~/config.json")
    .with_path_expansion(true);  // Enable
```

## Getting Help

If you encounter issues during migration:

1. Check the [API documentation](API.md)
2. Look at [examples](EXAMPLES.md)
3. Open an issue on GitHub
4. Ask in discussions

## Migration Checklist

- [ ] Add config-manager dependency
- [ ] Create adapter layer
- [ ] Implement custom resolver (if needed)
- [ ] Implement custom dependency provider (if needed)
- [ ] Add validators
- [ ] Update all load_config calls
- [ ] Update all save_config calls
- [ ] Test thoroughly
- [ ] Remove old code
- [ ] Update documentation
