# Examples

Comprehensive examples demonstrating config-manager usage.

## Table of Contents

- [Basic Usage](#basic-usage)
- [Hierarchical Configuration](#hierarchical-configuration)
- [Validation](#validation)
- [Dependency Resolution](#dependency-resolution)
- [Fuzzy Matching](#fuzzy-matching)
- [Path Expansion](#path-expansion)
- [Custom Implementations](#custom-implementations)
- [Real-World Scenarios](#real-world-scenarios)

## Basic Usage

### Simple Configuration

```rust
use config_manager::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    name: String,
    version: String,
    port: u16,
    debug: bool,
}

fn main() -> Result<()> {
    // Create manager
    let manager = ConfigManager::<AppConfig>::builder()
        .loader(JsonLoader::new("config.json"))
        .build()?;

    // Create config
    let config = AppConfig {
        name: "myapp".to_string(),
        version: "1.0.0".to_string(),
        port: 3000,
        debug: true,
    };

    // Save
    manager.save(&config)?;
    println!("✓ Saved configuration");

    // Load
    let loaded = manager.load()?;
    println!("✓ Loaded: {} v{}", loaded.name, loaded.version);
    println!("  Port: {}", loaded.port);
    println!("  Debug: {}", loaded.debug);

    Ok(())
}
```

### With Pretty Printing

```rust
let manager = ConfigManager::<AppConfig>::builder()
    .loader(
        JsonLoader::new("config.json")
            .with_pretty_print(true)  // Human-readable JSON
    )
    .build()?;
```

### Compact JSON

```rust
let manager = ConfigManager::<AppConfig>::builder()
    .loader(
        JsonLoader::new("config.json")
            .with_pretty_print(false)  // Single-line JSON
    )
    .build()?;
```

## Hierarchical Configuration

### Projects and Apps

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    projects: HashMap<String, Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
    name: String,
    apps: HashMap<String, App>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct App {
    name: String,
    path: String,
    port: u16,
    environment: String,
}

fn main() -> Result<()> {
    let manager = ConfigManager::<Config>::builder()
        .loader(JsonLoader::new("config.json"))
        .build()?;

    // Build config
    let mut config = Config {
        projects: HashMap::new(),
    };

    let mut web_apps = HashMap::new();
    web_apps.insert("frontend".to_string(), App {
        name: "Frontend".to_string(),
        path: "~/projects/web/frontend".to_string(),
        port: 3000,
        environment: "development".to_string(),
    });

    web_apps.insert("backend".to_string(), App {
        name: "Backend API".to_string(),
        path: "~/projects/web/backend".to_string(),
        port: 8080,
        environment: "development".to_string(),
    });

    config.projects.insert("web".to_string(), Project {
        name: "Web Project".to_string(),
        apps: web_apps,
    });

    // Save
    manager.save(&config)?;

    // Load and navigate
    let loaded = manager.load()?;

    for (project_name, project) in &loaded.projects {
        println!("\nProject: {} ({})", project.name, project_name);
        for (app_name, app) in &project.apps {
            println!("  - {} ({}:{})", app.name, app_name, app.port);
        }
    }

    Ok(())
}
```

### Resolving Entities

```rust
use config_manager::resolver::HashMapResolver;

// Resolve an app from a project
if let Some(project) = config.projects.get("web") {
    let resolver = HashMapResolver::new(true);  // Enable fuzzy matching

    match resolver.resolve(&project.apps, "frontend") {
        Ok(app) => println!("Found: {} on port {}", app.name, app.port),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

## Validation

### Single Validator

```rust
use config_manager::validation::{Validator, ValidationResult, ValidationError};

struct PortValidator;

impl Validator<AppConfig> for PortValidator {
    fn validate(&self, config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();

        if config.port == 0 {
            result.add_error(ValidationError::new(
                "port",
                "Port cannot be 0"
            ));
        }

        if config.port > 65535 {
            result.add_error(ValidationError::new(
                "port",
                "Port must be <= 65535"
            ));
        }

        result
    }
}

// Use validator
let manager = ConfigManager::<AppConfig>::builder()
    .loader(JsonLoader::new("config.json"))
    .validator(Box::new(PortValidator))
    .build()?;

// Validation happens on save
let invalid_config = AppConfig {
    port: 0,  // Invalid!
    // ...
};

match manager.save(&invalid_config) {
    Ok(()) => println!("Saved"),
    Err(Error::ValidationError(msg)) => {
        eprintln!("Validation failed:\n{}", msg);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

### Multiple Validators

```rust
struct NameValidator;

impl Validator<AppConfig> for NameValidator {
    fn validate(&self, config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();

        if config.name.is_empty() {
            result.add_error(ValidationError::new(
                "name",
                "Name cannot be empty"
            ));
        }

        if config.name.len() > 50 {
            result.add_error(ValidationError::new(
                "name",
                "Name too long (max 50 chars)"
            ));
        }

        result
    }
}

// Add multiple validators
let manager = ConfigManager::<AppConfig>::builder()
    .loader(JsonLoader::new("config.json"))
    .validator(Box::new(PortValidator))
    .validator(Box::new(NameValidator))
    .build()?;
```

### Validation with Warnings

```rust
use config_manager::validation::ValidationWarning;

struct SecurityValidator;

impl Validator<AppConfig> for SecurityValidator {
    fn validate(&self, config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();

        if config.debug && config.environment == "production" {
            result.add_warning(ValidationWarning::new(
                "debug",
                "Debug mode enabled in production"
            ));
        }

        if config.port < 1024 {
            result.add_warning(ValidationWarning::new(
                "port",
                "Using privileged port (< 1024), requires root"
            ));
        }

        result
    }
}
```

## Dependency Resolution

### Basic Dependencies

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Service {
    name: String,
    depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    services: HashMap<String, Service>,
}

use config_manager::dependencies::{DependencyGraph, DependencyProvider};
use config_manager::resolver::Resolver;

// Implement dependency provider
struct ServiceDependencyProvider;

impl DependencyProvider<Service> for ServiceDependencyProvider {
    fn dependencies(&self, entity: &Service) -> Vec<String> {
        entity.depends_on.clone()
    }
}

// Implement resolver
struct ServiceResolver;

impl Resolver<Config, Service> for ServiceResolver {
    fn resolve(&self, config: &Config, id: &str) -> config_manager::core::Result<Service> {
        config.services.get(id).cloned()
            .ok_or_else(|| config_manager::core::Error::NotFound {
                id: id.to_string(),
                suggestion: None,
            })
    }

    fn list_ids(&self, config: &Config) -> Vec<String> {
        config.services.keys().cloned().collect()
    }
}

// Create dependency graph
let graph = DependencyGraph::new(
    Box::new(ServiceDependencyProvider),
    Box::new(ServiceResolver),
    Box::new(|service: &Service| service.name.clone()),
);
```

### Resolve Dependency Chain

```rust
// Build config with dependencies
let mut config = Config {
    services: HashMap::new(),
};

config.services.insert("database".to_string(), Service {
    name: "database".to_string(),
    depends_on: vec![],
});

config.services.insert("cache".to_string(), Service {
    name: "cache".to_string(),
    depends_on: vec![],
});

config.services.insert("api".to_string(), Service {
    name: "api".to_string(),
    depends_on: vec!["database".to_string(), "cache".to_string()],
});

config.services.insert("worker".to_string(), Service {
    name: "worker".to_string(),
    depends_on: vec!["api".to_string()],
});

// Resolve dependencies for worker
let worker = config.services.get("worker").unwrap();
let chain = graph.resolve_chain(&config, worker)?;

println!("Dependencies for 'worker':");
for service in &chain {
    println!("  - {}", service.name);
}
// Output:
//   - database
//   - cache
//   - api
//   - worker
```

### Topological Sort

```rust
// Sort all services in dependency order
let all_services: Vec<Service> = config.services.values().cloned().collect();
let sorted = graph.topological_sort(&config, &all_services)?;

println!("Start order:");
for service in sorted {
    println!("  - {}", service.name);
}
// Output:
//   - database
//   - cache
//   - api
//   - worker
```

### Detect Circular Dependencies

```rust
// Add circular dependency
config.services.insert("cycle_a".to_string(), Service {
    name: "cycle_a".to_string(),
    depends_on: vec!["cycle_b".to_string()],
});

config.services.insert("cycle_b".to_string(), Service {
    name: "cycle_b".to_string(),
    depends_on: vec!["cycle_a".to_string()],  // Circular!
});

let all_services: Vec<Service> = config.services.values().cloned().collect();

match graph.check_cycles(&config, &all_services) {
    Ok(()) => println!("No cycles"),
    Err(Error::CircularDependency { path }) => {
        eprintln!("Circular dependency detected:");
        eprintln!("  {}", path.join(" -> "));
        // Output: cycle_a -> cycle_b -> cycle_a
    }
    _ => {}
}
```

## Fuzzy Matching

### Basic Fuzzy Matching

```rust
use config_manager::resolver::HashMapResolver;

let mut apps = HashMap::new();
apps.insert("frontend".to_string(), app1);
apps.insert("backend".to_string(), app2);
apps.insert("database".to_string(), app3);

let resolver = HashMapResolver::new(true);  // Enable fuzzy matching

// Typo: "fronted" instead of "frontend"
match resolver.resolve(&apps, "fronted") {
    Ok(app) => println!("Found: {}", app.name),
    Err(Error::NotFound { id, suggestion: Some(s) }) => {
        println!("'{}' not found. Did you mean '{}'?", id, s);
        // Output: 'fronted' not found. Did you mean 'frontend'?
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

### Adjusting Tolerance

```rust
// More tolerant of typos
let resolver = HashMapResolver::new(true)
    .with_max_distance(3);  // Allow up to 3 character differences

// Less tolerant
let resolver = HashMapResolver::new(true)
    .with_max_distance(1);  // Only single-character typos
```

### Manual Suggestions

```rust
if let Some(suggestion) = resolver.suggest(&apps, "bacend") {
    println!("Did you mean '{}'?", suggestion);
    // Output: Did you mean 'backend'?
}
```

## Path Expansion

### Tilde Expansion

```rust
use config_manager::utils::expand_tilde;

// Expands to /home/username/config.json
let path = expand_tilde("~/config.json");

// Use in loader
let loader = JsonLoader::new("~/my-app/config.json");
```

### Environment Variable Expansion

```rust
use config_manager::utils::expand_env_vars;
use std::env;

env::set_var("CONFIG_DIR", "/etc/my-app");

// $VAR syntax
let path = expand_env_vars("$CONFIG_DIR/config.json");
// Result: /etc/my-app/config.json

// ${VAR} syntax
let path = expand_env_vars("${CONFIG_DIR}/config.json");
// Result: /etc/my-app/config.json
```

### Combined Expansion

```rust
use config_manager::utils::path::expand_all;

env::set_var("APP_NAME", "my-app");

// Expands both ~ and $VAR
let path = expand_all("~/$APP_NAME/config.json");
// Result: /home/username/my-app/config.json

// Automatic in JsonLoader
let loader = JsonLoader::new("~/$CONFIG_DIR/config.json")
    .with_path_expansion(true);  // Default
```

## Custom Implementations

### Custom Loader (HTTP)

```rust
use config_manager::loader::ConfigLoader;
use config_manager::core::Result;
use serde::{Deserialize, Serialize};

struct HttpLoader {
    url: String,
}

impl HttpLoader {
    fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

impl<T> ConfigLoader<T> for HttpLoader
where
    T: DeserializeOwned + Serialize,
{
    fn load(&self) -> Result<T> {
        let response = reqwest::blocking::get(&self.url)
            .map_err(|e| config_manager::core::Error::LoadError(e.to_string()))?;

        let config = response.json()
            .map_err(|e| config_manager::core::Error::LoadError(e.to_string()))?;

        Ok(config)
    }

    fn save(&self, config: &T) -> Result<()> {
        let client = reqwest::blocking::Client::new();
        client.put(&self.url)
            .json(config)
            .send()
            .map_err(|e| config_manager::core::Error::SaveError(e.to_string()))?;

        Ok(())
    }

    fn exists(&self) -> bool {
        reqwest::blocking::get(&self.url).is_ok()
    }

    fn source_info(&self) -> String {
        format!("HTTP: {}", self.url)
    }
}

// Use custom loader
let manager = ConfigManager::<AppConfig>::builder()
    .loader(HttpLoader::new("https://api.example.com/config"))
    .build()?;
```

### Custom Resolver (Path-based)

```rust
use config_manager::resolver::Resolver;

struct PathResolver;

impl Resolver<Config, serde_json::Value> for PathResolver {
    fn resolve(&self, config: &Config, path: &str) -> config_manager::core::Result<serde_json::Value> {
        // Parse path like "projects.web.apps.frontend"
        let parts: Vec<&str> = path.split('.').collect();

        let config_value = serde_json::to_value(config).unwrap();
        let mut current = &config_value;

        for part in parts {
            current = current.get(part)
                .ok_or_else(|| config_manager::core::Error::NotFound {
                    id: path.to_string(),
                    suggestion: None,
                })?;
        }

        Ok(current.clone())
    }

    fn list_ids(&self, _config: &Config) -> Vec<String> {
        vec![]  // Would need to traverse structure
    }
}

// Use path-based resolution
let resolver = PathResolver;
let value = resolver.resolve(&config, "projects.web.apps.frontend")?;
```

## Real-World Scenarios

### Multi-Environment Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    environment: String,
    database: DatabaseConfig,
    redis: RedisConfig,
    logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    database: String,
    pool_size: u32,
}

// Load environment-specific config
let env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
let config_file = format!("config.{}.json", env);

let manager = ConfigManager::<Config>::builder()
    .loader(JsonLoader::new(&config_file))
    .validator(Box::new(DatabaseValidator))
    .validator(Box::new(RedisValidator))
    .build()?;

let config = manager.load_and_validate()?;
```

### Configuration with Secrets

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(skip_serializing)]  // Don't save to file
    api_key: String,

    database_url: String,
    redis_url: String,
}

// Load config, then inject secrets
let mut config = manager.load()?;
config.api_key = std::env::var("API_KEY")?;
```

### Validation Before Deployment

```rust
struct DeploymentValidator;

impl Validator<Config> for DeploymentValidator {
    fn validate(&self, config: &Config) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Production checks
        if config.environment == "production" {
            if config.logging.level == "debug" {
                result.add_error(ValidationError::new(
                    "logging.level",
                    "Debug logging not allowed in production"
                ));
            }

            if config.database.pool_size < 10 {
                result.add_warning(ValidationWarning::new(
                    "database.pool_size",
                    "Low pool size for production"
                ));
            }
        }

        result
    }
}
```

### Dynamic Configuration Reload

```rust
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

// Shared config
let config = Arc::new(RwLock::new(manager.load()?));

// Clone for reload thread
let config_clone = config.clone();
let manager_clone = Arc::new(manager);

thread::spawn(move || {
    loop {
        thread::sleep(Duration::from_secs(60));

        if let Ok(new_config) = manager_clone.reload() {
            *config_clone.write().unwrap() = new_config;
            println!("Config reloaded");
        }
    }
});

// Use config
let current_config = config.read().unwrap();
println!("Port: {}", current_config.port);
```

### Merging Configurations

```rust
// Load base config
let base = base_manager.load()?;

// Load environment-specific overrides
let overrides = override_manager.load()?;

// Merge (manual implementation)
let mut merged = base;
merged.port = overrides.port;  // Override port
merged.debug = overrides.debug;  // Override debug

// Save merged
final_manager.save(&merged)?;
```

## Async Configuration (v0.3+)

### Async JSON Loading

```rust
use config_manager::loader::{AsyncJsonLoader, AsyncConfigLoader};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    name: String,
    port: u16,
}

#[tokio::main]
async fn main() -> config_manager::core::Result<()> {
    let loader = AsyncJsonLoader::new("config.json");
    
    // Async load
    let config: Config = loader.load().await?;
    println!("Loaded: {}", config.name);
    
    // Async save
    loader.save(&config).await?;
    
    Ok(())
}
```

### Concurrent Config Loading

```rust
#[tokio::main]
async fn main() -> config_manager::core::Result<()> {
    let configs = vec!["app1.json", "app2.json", "app3.json"];
    
    // Load all configs concurrently
    let handles: Vec<_> = configs
        .into_iter()
        .map(|path| {
            tokio::spawn(async move {
                let loader = AsyncJsonLoader::new(path);
                loader.load().await
            })
        })
        .collect();
    
    // Wait for all
    for handle in handles {
        let config: Config = handle.await.unwrap()?;
        println!("Loaded config: {}", config.name);
    }
    
    Ok(())
}
```

## Remote Configuration (v0.3+)

### HTTP Configuration

```rust
use config_manager::loader::{HttpLoader, AsyncConfigLoader, HttpAuth};
use std::time::Duration;

#[tokio::main]
async fn main() -> config_manager::core::Result<()> {
    // No authentication
    let loader = HttpLoader::new("https://config.example.com/app.json");
    
    // With Bearer token
    let loader = HttpLoader::new("https://api.example.com/config")
        .with_auth(HttpAuth::Bearer("secret-token".to_string()))
        .with_timeout(Duration::from_secs(30));
    
    // With Basic auth
    let loader = HttpLoader::new("https://api.example.com/config")
        .with_auth(HttpAuth::Basic("user".to_string(), "pass".to_string()));
    
    let config: Config = loader.load().await?;
    Ok(())
}
```

### S3 Configuration

```rust
use config_manager::loader::{S3Loader, AsyncConfigLoader};

#[tokio::main]
async fn main() -> config_manager::core::Result<()> {
    // Uses AWS SDK default credential chain
    let loader = S3Loader::new("my-config-bucket", "configs/production.json")
        .with_region("us-west-2");
    
    // Load from S3
    let config: Config = loader.load().await?;
    
    // Save to S3
    loader.save(&config).await?;
    
    Ok(())
}
```

## Encrypted Secrets (v0.3+)

### Basic Encryption

```rust
use config_manager::loader::{EncryptedLoader, JsonLoader, ConfigLoader};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Secrets {
    api_key: String,
    db_password: String,
}

fn main() -> config_manager::core::Result<()> {
    // Wrap any loader with encryption
    let base_loader = JsonLoader::new("secrets.json");
    let loader = EncryptedLoader::new(base_loader, "my-secret-password");
    
    let secrets = Secrets {
        api_key: "sk-1234567890".to_string(),
        db_password: "super-secret".to_string(),
    };
    
    // Saves encrypted data to disk
    loader.save(&secrets)?;
    
    // Verify encryption - file should not contain plaintext
    let raw = std::fs::read_to_string("secrets.json")?;
    assert!(!raw.contains("super-secret"));
    
    // Load and decrypt
    let loaded: Secrets = loader.load()?;
    assert_eq!(loaded.api_key, secrets.api_key);
    
    Ok(())
}
```

### Layered Config with Encrypted Secrets

```rust
use config_manager::loader::{
    LayeredLoader, MergeStrategy, JsonLoader, EncryptedLoader
};

fn main() -> config_manager::core::Result<()> {
    // Base config (public)
    let base = JsonLoader::new("config/base.json");
    
    // Secrets (encrypted)
    let secrets_base = JsonLoader::new("config/secrets.json");
    let secrets = EncryptedLoader::new(secrets_base, "production-password");
    
    // Merge public base with encrypted secrets
    let loader = LayeredLoader::new(MergeStrategy::DeepMerge)
        .add_layer(base)
        .add_layer(secrets);
    
    let config: AppConfig = loader.load()?;
    Ok(())
}
```

## Schema Validation (v0.3+)

### Generate and Validate

```rust
use config_manager::schema::SchemaValidator;
use config_manager::validation::Validator;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ServerConfig {
    #[schemars(range(min = 1024, max = 65535))]
    port: u16,
    
    #[schemars(regex(pattern = r"^[a-z0-9-]+$"))]
    hostname: String,
    
    #[schemars(length(min = 1, max = 100))]
    name: String,
}

fn main() -> config_manager::core::Result<()> {
    let validator = SchemaValidator::<ServerConfig>::new();
    
    // Generate JSON schema
    let schema = validator.schema_json()?;
    println!("Schema:\n{}", schema);
    
    // Valid config
    let valid = ServerConfig {
        port: 8080,
        hostname: "api-server".to_string(),
        name: "Production API".to_string(),
    };
    
    let result = validator.validate(&valid);
    assert!(result.is_valid());
    
    // Invalid config
    let invalid = ServerConfig {
        port: 80,  // Below minimum (1024)
        hostname: "Invalid Hostname!".to_string(),  // Invalid regex
        name: "".to_string(),  // Too short
    };
    
    let result = validator.validate(&invalid);
    assert!(!result.is_valid());
    for error in result.errors() {
        println!("Validation error: {}", error);
    }
    
    Ok(())
}
```

## File Watching (v0.2+)

### Hot Reload

```rust
use config_manager::loader::{JsonLoader, ConfigLoader};
use config_manager::watch::{ConfigWatcher, ConfigEvent};
use std::sync::{Arc, Mutex};

fn main() -> config_manager::core::Result<()> {
    let loader = JsonLoader::new("config.json");
    
    // Initial load
    let config: Config = loader.load()?;
    let current_config = Arc::new(Mutex::new(config));
    
    // Set up watcher
    let mut watcher: ConfigWatcher<Config> = ConfigWatcher::new(loader)?;
    
    let config_clone = Arc::clone(&current_config);
    watcher.start(move |event| {
        match event {
            ConfigEvent::Modified => {
                println!("Config file changed, reloading...");
                // Reload in the callback
                // In real app, you'd use channels to notify main thread
            }
            ConfigEvent::Error(e) => {
                eprintln!("Watch error: {}", e);
            }
            _ => {}
        }
    })?;
    
    // Keep running...
    std::thread::sleep(std::time::Duration::from_secs(60));
    
    watcher.stop()?;
    Ok(())
}
```

## Production Patterns

### Multi-Environment Setup

```rust
use config_manager::loader::{LayeredLoader, MergeStrategy, JsonLoader};

fn load_config(environment: &str) -> config_manager::core::Result<AppConfig> {
    let mut loader = LayeredLoader::new(MergeStrategy::DeepMerge)
        // 1. Defaults
        .add_layer(JsonLoader::new("config/defaults.json"));
    
    // 2. Environment-specific
    if environment != "development" {
        let env_path = format!("config/{}.json", environment);
        loader = loader.add_layer(JsonLoader::new(env_path));
    }
    
    // 3. Local overrides (optional, gitignored)
    let local_loader = JsonLoader::new("config/local.json");
    if local_loader.exists() {
        loader = loader.add_layer(local_loader);
    }
    
    loader.load()
}

fn main() -> config_manager::core::Result<()> {
    let env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string());
    let config = load_config(&env)?;
    
    println!("Loaded config for environment: {}", env);
    Ok(())
}
```

### Error Handling Best Practices

```rust
use config_manager::core::Error;

fn handle_config_load() -> config_manager::core::Result<()> {
    let loader = JsonLoader::new("config.json");
    
    match loader.load::<Config>() {
        Ok(config) => {
            println!("Config loaded successfully");
            Ok(())
        }
        Err(e) => {
            // Use error helpers (v1.0+)
            if e.is_load_error() {
                eprintln!("Failed to load config file: {}", e);
                // Maybe create default config
            } else if e.is_validation_error() {
                eprintln!("Config validation failed: {}", e);
                // User needs to fix config
            }
            
            Err(e)
        }
    }
}
```

