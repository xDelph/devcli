# API Reference

Complete API documentation for config-manager.

## Table of Contents

- [Core Types](#core-types)
- [Loaders](#loaders)
- [Resolvers](#resolvers)
- [Dependencies](#dependencies)
- [Validation](#validation)
- [Utilities](#utilities)
- [Error Types](#error-types)

## Core Types

### ConfigManager<T>

Main configuration manager that orchestrates loading, validation, and saving.

```rust
pub struct ConfigManager<T> { /* private fields */ }
```

#### Methods

##### `builder() -> ConfigManagerBuilder<T>`

Create a new builder for constructing a ConfigManager.

```rust
let manager = ConfigManager::<MyConfig>::builder()
    .loader(JsonLoader::new("config.json"))
    .validator(Box::new(MyValidator))
    .build()?;
```

##### `with_loader(loader: Box<dyn ConfigLoader<T>>) -> Self`

Create a ConfigManager with just a loader (no validation).

```rust
let manager = ConfigManager::with_loader(
    Box::new(JsonLoader::new("config.json"))
);
```

##### `load(&self) -> Result<T>`

Load configuration from the source.

```rust
let config = manager.load()?;
```

**Errors:**
- `Error::LoadError` - Failed to load or deserialize

##### `save(&self, config: &T) -> Result<()>`

Save configuration to the source.

Validates before saving if validators are configured.

```rust
manager.save(&config)?;
```

**Errors:**
- `Error::ValidationError` - Configuration is invalid
- `Error::SaveError` - Failed to serialize or write

##### `reload(&self) -> Result<T>`

Reload configuration from the source (alias for `load()`).

```rust
let config = manager.reload()?;
```

##### `exists(&self) -> bool`

Check if the configuration source exists.

```rust
if !manager.exists() {
    println!("Config not found");
}
```

##### `source_info(&self) -> String`

Get information about the configuration source.

```rust
println!("Config source: {}", manager.source_info());
```

##### `validate(&self, config: &T) -> ValidationResult`

Validate configuration.

Runs all registered validators and returns a combined result.

```rust
let result = manager.validate(&config);
if !result.is_valid() {
    for error in result.errors() {
        eprintln!("Error: {}", error);
    }
}
```

##### `is_valid(&self, config: &T) -> bool`

Check if configuration is valid (no errors).

```rust
if manager.is_valid(&config) {
    manager.save(&config)?;
}
```

##### `load_and_validate(&self) -> Result<T>`

Load and validate configuration in one step.

```rust
let config = manager.load_and_validate()?;
```

**Errors:**
- `Error::LoadError` - Failed to load
- `Error::ValidationError` - Configuration is invalid

### ConfigManagerBuilder<T>

Builder for constructing a ConfigManager.

```rust
pub struct ConfigManagerBuilder<T> { /* private fields */ }
```

#### Methods

##### `new() -> Self`

Create a new builder.

```rust
let builder = ConfigManagerBuilder::<MyConfig>::new();
```

##### `loader(self, loader: impl ConfigLoader<T> + 'static) -> Self`

Set the configuration loader (required).

```rust
let builder = builder.loader(JsonLoader::new("config.json"));
```

##### `loader_boxed(self, loader: Box<dyn ConfigLoader<T>>) -> Self`

Set the configuration loader (boxed version).

```rust
let loader: Box<dyn ConfigLoader<MyConfig>> = Box::new(JsonLoader::new("config.json"));
let builder = builder.loader_boxed(loader);
```

##### `validator(self, validator: Box<dyn Validator<T>>) -> Self`

Add a validator.

Multiple validators can be added and will run in order.

```rust
let builder = builder.validator(Box::new(MyValidator));
```

##### `validators(self, validators: Vec<Box<dyn Validator<T>>>) -> Self`

Add multiple validators at once.

```rust
let validators = vec![
    Box::new(PortValidator) as Box<dyn Validator<Config>>,
    Box::new(NameValidator) as Box<dyn Validator<Config>>,
];
let builder = builder.validators(validators);
```

##### `build(self) -> Result<ConfigManager<T>>`

Build the ConfigManager.

```rust
let manager = builder.build()?;
```

**Errors:**
- `Error::Custom` - Required components (loader) are not set

## Loaders

### ConfigLoader<T>

Trait for loading and saving configuration from various sources.

```rust
pub trait ConfigLoader<T>: Send + Sync + Debug {
    fn load(&self) -> Result<T>;
    fn save(&self, config: &T) -> Result<()>;
    fn exists(&self) -> bool;
    fn source_info(&self) -> String;
    fn reload(&self) -> Result<T> { self.load() }
}
```

### JsonLoader

JSON file-based configuration loader.

```rust
pub struct JsonLoader { /* private fields */ }
```

#### Methods

##### `new<P: AsRef<Path>>(path: P) -> Self`

Create a new JSON loader for the specified path.

Supports ~ and $ENV expansion in the path.

```rust
let loader = JsonLoader::new("~/config.json");
let loader = JsonLoader::new("$CONFIG_DIR/config.json");
```

##### `with_path_expansion(self, enabled: bool) -> Self`

Enable or disable path expansion (tilde and env vars).

Default: `true`

```rust
let loader = JsonLoader::new("config.json")
    .with_path_expansion(false);
```

##### `with_pretty_print(self, enabled: bool) -> Self`

Enable or disable pretty-printing when saving.

Default: `true` (indented, readable JSON)

```rust
let loader = JsonLoader::new("config.json")
    .with_pretty_print(false);  // Compact JSON
```

##### `with_create_dirs(self, enabled: bool) -> Self`

Enable or disable automatic directory creation when saving.

Default: `true`

```rust
let loader = JsonLoader::new("deep/nested/config.json")
    .with_create_dirs(true);  // Creates deep/nested/ automatically
```

## Resolvers

### Resolver<C, E>

Trait for resolving entities by identifier.

```rust
pub trait Resolver<C, E>: Send + Sync {
    fn resolve(&self, config: &C, id: &str) -> Result<E>;
    fn list_ids(&self, config: &C) -> Vec<String>;
    fn suggest(&self, config: &C, id: &str) -> Option<String>;
    fn exists(&self, config: &C, id: &str) -> bool;
    fn resolve_many(&self, config: &C, ids: &[&str]) -> Result<Vec<E>>;
}
```

**Type Parameters:**
- `C` - Configuration type to search within
- `E` - Entity type to resolve

### HashMapResolver

A resolver for HashMap-based configurations.

```rust
pub struct HashMapResolver { /* private fields */ }
```

#### Methods

##### `new(fuzzy_matching: bool) -> Self`

Create a new HashMap resolver.

```rust
let resolver = HashMapResolver::new(true);  // Enable fuzzy matching
```

##### `with_max_distance(self, distance: usize) -> Self`

Set the maximum Levenshtein distance for fuzzy matching.

Default: `2`

```rust
let resolver = HashMapResolver::new(true)
    .with_max_distance(3);  // More tolerant of typos
```

#### Implementation

Implements `Resolver<HashMap<K, V>, V>` where:
- `K: Eq + Hash + AsRef<str> + ToString`
- `V: Clone`

```rust
let mut apps = HashMap::new();
apps.insert("frontend".to_string(), app1);
apps.insert("backend".to_string(), app2);

let resolver = HashMapResolver::new(true);

// Exact match
let app = resolver.resolve(&apps, "frontend")?;

// Fuzzy match (typo)
match resolver.resolve(&apps, "fronted") {
    Err(Error::NotFound { suggestion: Some(s), .. }) => {
        println!("Did you mean '{}'?", s);  // "frontend"
    }
    _ => {}
}
```

## Dependencies

### DependencyProvider<E>

Trait for extracting dependencies from entities.

```rust
pub trait DependencyProvider<E>: Send + Sync {
    fn dependencies(&self, entity: &E) -> Vec<String>;
    fn has_dependencies(&self, entity: &E) -> bool;
    fn dependency_count(&self, entity: &E) -> usize;
}
```

**Type Parameters:**
- `E` - Entity type

#### Example Implementation

```rust
struct MyApp {
    name: String,
    depends_on: Vec<String>,
}

struct AppDependencyProvider;

impl DependencyProvider<MyApp> for AppDependencyProvider {
    fn dependencies(&self, entity: &MyApp) -> Vec<String> {
        entity.depends_on.clone()
    }
}
```

### DependencyGraph<C, E, I>

A dependency graph that can resolve dependency chains and detect cycles.

```rust
pub struct DependencyGraph<C, E, I = String> { /* private fields */ }
```

**Type Parameters:**
- `C` - Configuration type
- `E` - Entity type
- `I` - Identifier type (default: String)

#### Methods

##### `new(...) -> Self`

Create a new dependency graph.

```rust
let graph = DependencyGraph::new(
    Box::new(MyDependencyProvider),
    Box::new(MyResolver),
    Box::new(|entity: &MyEntity| entity.id.clone()),
);
```

**Parameters:**
- `provider: Box<dyn DependencyProvider<E>>` - Extracts dependencies
- `resolver: Box<dyn Resolver<C, E>>` - Resolves dependency IDs to entities
- `id_extractor: Box<dyn Fn(&E) -> I + Send + Sync>` - Extracts ID from entity

##### `resolve_chain(&self, config: &C, entity: &E) -> Result<Vec<E>>`

Resolve the full dependency chain for an entity.

Returns all transitive dependencies in dependency order (dependencies before dependents).

```rust
let chain = graph.resolve_chain(&config, &app)?;
for dep in chain {
    println!("Dependency: {}", dep.name);
}
```

**Errors:**
- `Error::CircularDependency` - Circular dependency detected
- `Error::NotFound` - Dependency not found

##### `topological_sort(&self, config: &C, entities: &[E]) -> Result<Vec<E>>`

Topologically sort entities based on their dependencies.

Returns entities in an order where all dependencies come before dependents.

Uses Kahn's algorithm.

```rust
let sorted = graph.topological_sort(&config, &all_apps)?;
// Start apps in dependency order
for app in sorted {
    start_app(&app);
}
```

**Errors:**
- `Error::CircularDependency` - Circular dependency detected
- `Error::DependencyError` - Dependency not in entity list

##### `check_cycles(&self, config: &C, entities: &[E]) -> Result<()>`

Check if there are any circular dependencies.

```rust
match graph.check_cycles(&config, &all_apps) {
    Ok(()) => println!("No cycles detected"),
    Err(Error::CircularDependency { path }) => {
        eprintln!("Cycle: {}", path.join(" -> "));
    }
    _ => {}
}
```

**Errors:**
- `Error::CircularDependency` - If cycles exist

## Validation

### Validator<T>

Trait for validating configuration.

```rust
pub trait Validator<T>: Send + Sync {
    fn validate(&self, config: &T) -> ValidationResult;
    fn validate_field(&self, config: &T, field: &str) -> ValidationResult;
    fn is_valid(&self, config: &T) -> bool;
}
```

#### Example Implementation

```rust
struct PortValidator;

impl Validator<Config> for PortValidator {
    fn validate(&self, config: &Config) -> ValidationResult {
        let mut result = ValidationResult::new();

        if config.port == 0 {
            result.add_error(ValidationError::new(
                "port",
                "Port cannot be 0"
            ));
        }

        if config.port > 65535 {
            result.add_warning(ValidationWarning::new(
                "port",
                "Port is very high, might not be valid"
            ));
        }

        result
    }
}
```

### ValidationResult

Result of validation, containing errors and warnings.

```rust
pub struct ValidationResult { /* private fields */ }
```

#### Methods

##### `new() -> Self`

Create a new empty validation result.

```rust
let mut result = ValidationResult::new();
```

##### `add_error(&mut self, error: ValidationError)`

Add an error to the result.

```rust
result.add_error(ValidationError::new("port", "Invalid port"));
```

##### `add_warning(&mut self, warning: ValidationWarning)`

Add a warning to the result.

```rust
result.add_warning(ValidationWarning::new("port", "Unusual port"));
```

##### `is_valid(&self) -> bool`

Check if validation passed (no errors).

```rust
if result.is_valid() {
    println!("Config is valid");
}
```

##### `errors(&self) -> &[ValidationError]`

Get all errors.

```rust
for error in result.errors() {
    eprintln!("Error: {}", error);
}
```

##### `warnings(&self) -> &[ValidationWarning]`

Get all warnings.

```rust
for warning in result.warnings() {
    println!("Warning: {}", warning);
}
```

##### `error_count(&self) -> usize`

Get the number of errors.

##### `warning_count(&self) -> usize`

Get the number of warnings.

##### `merge(&mut self, other: ValidationResult)`

Merge another validation result into this one.

```rust
let result1 = validator1.validate(&config);
let result2 = validator2.validate(&config);
result1.merge(result2);
```

### ValidationError

A validation error.

```rust
pub struct ValidationError { /* private fields */ }
```

#### Methods

##### `new(field: impl Into<String>, message: impl Into<String>) -> Self`

Create a new validation error.

```rust
let error = ValidationError::new("port", "Port must be > 0");
```

##### `field(&self) -> &str`

Get the field that failed validation.

##### `message(&self) -> &str`

Get the error message.

### ValidationWarning

A validation warning.

```rust
pub struct ValidationWarning { /* private fields */ }
```

Same methods as `ValidationError`.

## Utilities

### Path Utilities

#### `expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf`

Expand tilde (~) in a path to the user's home directory.

```rust
use config_manager::utils::expand_tilde;

let expanded = expand_tilde("~/config.json");
// /home/username/config.json
```

#### `expand_env_vars<P: AsRef<Path>>(path: P) -> PathBuf`

Expand environment variables in a path.

Supports both `$VAR` and `${VAR}` syntax.

```rust
use config_manager::utils::expand_env_vars;
use std::env;

env::set_var("CONFIG_DIR", "/etc/myapp");
let expanded = expand_env_vars("$CONFIG_DIR/config.json");
// /etc/myapp/config.json

let expanded = expand_env_vars("${HOME}/config.json");
// /home/username/config.json
```

#### `expand_all<P: AsRef<Path>>(path: P) -> PathBuf`

Expand both tilde and environment variables in a path.

First expands environment variables, then expands tilde.

```rust
use config_manager::utils::path::expand_all;

env::set_var("APP", "myapp");
let expanded = expand_all("~/$APP/config.json");
// /home/username/myapp/config.json
```

#### `expand_path<P: AsRef<Path>>(path: P) -> PathBuf`

Alias for `expand_all`.

#### `contract_tilde<P: AsRef<Path>>(path: P) -> String`

Contract an absolute path under `$HOME` back to `~/…` for portable config storage.

```rust
use config_manager::utils::contract_tilde;

let contracted = contract_tilde("/home/username/Projects/app");
// ~/Projects/app
```

### String Utilities

#### `levenshtein_distance(s1: &str, s2: &str) -> usize`

Calculate the Levenshtein distance between two strings.

```rust
use config_manager::utils::levenshtein_distance;

assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
assert_eq!(levenshtein_distance("api", "api"), 0);
```

#### `find_closest_match(target: &str, candidates: &[&str], max_distance: usize) -> Option<String>`

Find the closest match from a list of candidates.

Returns the candidate with the smallest Levenshtein distance if it's within the max_distance threshold.

```rust
use config_manager::utils::string::find_closest_match;

let candidates = vec!["apple", "banana", "cherry"];
let result = find_closest_match("aple", &candidates, 2);
assert_eq!(result, Some("apple".to_string()));
```

## Error Types

### Error

Main error type for config-manager operations.

```rust
pub enum Error {
    LoadError(String),
    SaveError(String),
    NotFound { id: String, suggestion: Option<String> },
    Ambiguous { id: String, candidates: Vec<String> },
    DependencyError(String),
    CircularDependency { path: Vec<String> },
    ValidationError(String),
    IoError(std::io::Error),
    SerdeError(String),
    Custom(String),
}
```

#### Variants

##### `LoadError(String)`

Error loading configuration.

##### `SaveError(String)`

Error saving configuration.

##### `NotFound { id: String, suggestion: Option<String> }`

Entity not found during resolution.

May include a suggestion for typo correction.

##### `Ambiguous { id: String, candidates: Vec<String> }`

Multiple entities found (ambiguous identifier).

##### `DependencyError(String)`

Dependency resolution error.

##### `CircularDependency { path: Vec<String> }`

Circular dependency detected.

##### `ValidationError(String)`

Validation error.

##### `IoError(std::io::Error)`

I/O error.

##### `SerdeError(String)`

Serialization/deserialization error.

##### `Custom(String)`

Custom error.

#### Conversions

- `From<std::io::Error>`
- `From<serde_json::Error>`
- `From<anyhow::Error>`
- `Into<anyhow::Error>`

### Result<T>

Result type alias using config-manager's Error type.

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

## Prelude

The prelude module contains commonly used imports:

```rust
use config_manager::prelude::*;
```

Includes:
- `ConfigManager`
- `ConfigManagerBuilder`
- `ConfigLoader`
- `JsonLoader`
- `Resolver`
- `DependencyProvider`
- `DependencyGraph`
- `Validator`
- `Error`
- `Result`
- `Serialize` (from serde)
- `Deserialize` (from serde)

## Additional Loaders (v0.2+)

### TomlLoader

TOML file-based configuration loader (requires `toml` feature).

```rust
let loader = TomlLoader::new("config.toml")
    .with_path_expansion(true)
    .with_pretty_print(true);
```

### YamlLoader

YAML file-based configuration loader (requires `yaml` feature).

```rust
let loader = YamlLoader::new("config.yaml")
    .with_path_expansion(true);
```

### LayeredLoader<T>

Merge multiple configuration sources with different strategies.

```rust
let loader = LayeredLoader::new(MergeStrategy::DeepMerge)
    .add_layer(JsonLoader::new("base.json"))
    .add_layer(JsonLoader::new("override.json"));
```

#### MergeStrategy

- `Override` - Later layers completely replace earlier ones
- `DeepMerge` - Recursively merge nested structures

## Async Loaders (v0.3+)

### AsyncConfigLoader<T>

Trait for async configuration loading (requires `async` feature).

```rust
#[async_trait]
pub trait AsyncConfigLoader<T>: Send + Sync + Debug {
    async fn load(&self) -> Result<T>;
    async fn save(&self, config: &T) -> Result<()>;
    fn exists(&self) -> bool;
    fn source_info(&self) -> String;
}
```

### AsyncJsonLoader

Async JSON loader with tokio.

```rust
let loader = AsyncJsonLoader::new("config.json");
let config: MyConfig = loader.load().await?;
```

### AsyncTomlLoader

Async TOML loader (requires `async` and `toml` features).

### AsyncYamlLoader

Async YAML loader (requires `async` and `yaml` features).

### HttpLoader

HTTP/HTTPS remote configuration loader (requires `http` feature).

```rust
let loader = HttpLoader::new("https://api.example.com/config")
    .with_auth(HttpAuth::Bearer("token".to_string()))
    .with_timeout(Duration::from_secs(30));
```

#### HttpAuth

- `None` - No authentication
- `Bearer(String)` - Bearer token authentication
- `Basic(String, String)` - Basic authentication (username, password)

### S3Loader

AWS S3 configuration loader (requires `s3` feature).

```rust
let loader = S3Loader::new("my-bucket", "config/app.json")
    .with_region("us-east-1");
```

### EncryptedLoader<L>

Wrapper for encrypting/decrypting configurations (requires `encryption` feature).

```rust
let base = JsonLoader::new("secrets.json");
let loader = EncryptedLoader::new(base, "my-password");

// Transparent encryption on save, decryption on load
loader.save(&config)?;
let loaded = loader.load()?;
```

Uses AES-256-GCM with Argon2 key derivation.

## File Watching (v0.2+)

### ConfigWatcher<T>

Watch configuration files for changes (requires `watch` feature).

```rust
let loader = JsonLoader::new("config.json");
let mut watcher = ConfigWatcher::new(loader)?;

watcher.start(|event| {
    match event {
        ConfigEvent::Modified => println!("Config changed!"),
        _ => {}
    }
})?;
```

#### ConfigEvent

- `Modified` - Configuration was modified
- `Created` - Configuration was created
- `Removed` - Configuration was deleted
- `Error(String)` - Watch error occurred

## Schema Validation (v0.3+)

### SchemaValidator<T>

Generate and validate JSON schemas (requires `schema` feature).

```rust
use schemars::JsonSchema;

#[derive(Serialize, Deserialize, JsonSchema)]
struct MyConfig {
    #[schemars(range(min = 1, max = 65535))]
    port: u16,
}

let validator = SchemaValidator::<MyConfig>::new();

// Generate schema
let schema_json = validator.schema_json()?;

// Validate
let result = validator.validate(&config);
assert!(result.is_valid());
```

## Error Helpers (v1.0+)

### Error Methods

```rust
impl Error {
    pub fn is_not_found(&self) -> bool;
    pub fn is_validation_error(&self) -> bool;
    pub fn is_circular_dependency(&self) -> bool;
    pub fn is_load_error(&self) -> bool;
    pub fn is_save_error(&self) -> bool;
}
```

All methods are marked with `#[must_use]` and `#[inline]` for safety and performance.

## Version History

- **v1.0.0** - Stable API, integration tests, performance optimizations
- **v0.3.0** - Async support, HTTP/S3 loaders, encryption, schema validation
- **v0.2.0** - TOML/YAML, layered configs, file watching, benchmarks
- **v0.1.0** - Core traits, JSON loader, dependency resolution, validation
