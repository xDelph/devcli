# config-manager

[![License](https://img.shields.io/badge/License-PolyForm%20Noncommercial-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.91%2B-orange.svg)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-1.0.0-green.svg)](CHANGELOG.md)
[![Production Ready](https://img.shields.io/badge/status-production%20ready-brightgreen.svg)](CHANGELOG.md)

A production-ready, flexible, trait-based configuration management library for Rust applications.

## Overview

`config-manager` provides a powerful, extensible system for managing application configurations with built-in support for:

- **Multiple formats** (JSON, TOML, YAML)
- **Dependency resolution** with cycle detection
- **Fuzzy matching** for typo-tolerant lookups
- **Validation** with custom rules
- **Path expansion** (~/ and $ENV support)

Unlike traditional config libraries that force you into a specific structure, `config-manager` is **completely generic** and works with any configuration schema you define.

## Features

### 🎯 Core Features

- ✅ **Generic Configuration** - Works with any `Serialize + Deserialize` type
- ✅ **Multiple Formats** - JSON (built-in), TOML, YAML (feature-gated)
- ✅ **Dependency Resolution** - Automatic dependency chain resolution with cycle detection
- ✅ **Fuzzy Matching** - Typo-tolerant entity resolution using Levenshtein distance
- ✅ **Validation** - Declarative validation with custom validators
- ✅ **Path Expansion** - Automatic tilde (~) and environment variable ($VAR) expansion
- ✅ **Type Safe** - Full compile-time type checking
- ✅ **Zero Panics** - All errors returned as `Result` types
- ✅ **No Unsafe Code** - 100% safe Rust

### 🔌 Extensibility

Everything is trait-based and pluggable:

- Custom loaders (HTTP, S3, database, etc.)
- Custom resolvers (path-based, SQL-like queries, etc.)
- Custom validators (schema validation, business rules, etc.)
- Custom dependency providers

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
config-manager = "1.0"
serde = { version = "1.0", features = ["derive"] }
```

### Basic Example

```rust
use config_manager::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    name: String,
    version: String,
    port: u16,
}

fn main() -> Result<()> {
    // Create a configuration manager
    let manager = ConfigManager::<AppConfig>::builder()
        .loader(JsonLoader::new("config.json"))
        .build()?;

    // Create and save config
    let config = AppConfig {
        name: "myapp".to_string(),
        version: "1.0.0".to_string(),
        port: 3000,
    };

    manager.save(&config)?;

    // Load config
    let loaded = manager.load()?;
    println!("Loaded: {} v{}", loaded.name, loaded.version);

    Ok(())
}
```

## Documentation

- **[Architecture Guide](docs/ARCHITECTURE.md)** - Design decisions and internals
- **[API Reference](docs/API.md)** - Detailed API documentation
- **[Examples](docs/EXAMPLES.md)** - More usage examples
- **[Migration Guide](docs/MIGRATION.md)** - Migrating from other config systems
- **[Contributing](docs/CONTRIBUTING.md)** - How to contribute

## Common Use Cases

### Hierarchical Configuration

Perfect for applications with nested configuration structures:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    projects: HashMap<String, Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Project {
    apps: HashMap<String, App>,
}

let manager = ConfigManager::<Config>::builder()
    .loader(JsonLoader::new("config.json"))
    .build()?;
```

### With Validation

Ensure your configuration is always valid:

```rust
use config_manager::validation::{Validator, ValidationResult, ValidationError};

struct PortValidator;

impl Validator<AppConfig> for PortValidator {
    fn validate(&self, config: &AppConfig) -> ValidationResult {
        let mut result = ValidationResult::new();
        if config.port == 0 || config.port > 65535 {
            result.add_error(ValidationError::new(
                "port",
                "Port must be between 1 and 65535"
            ));
        }
        result
    }
}

let manager = ConfigManager::<AppConfig>::builder()
    .loader(JsonLoader::new("config.json"))
    .validator(Box::new(PortValidator))
    .build()?;

// Validation runs automatically on save
manager.save(&config)?; // Returns error if invalid
```

### Dependency Resolution

Manage dependencies between configuration entities:

```rust
use config_manager::dependencies::{DependencyGraph, DependencyProvider};

#[derive(Debug, Clone)]
struct App {
    name: String,
    depends_on: Vec<String>,
}

struct AppDependencyProvider;

impl DependencyProvider<App> for AppDependencyProvider {
    fn dependencies(&self, entity: &App) -> Vec<String> {
        entity.depends_on.clone()
    }
}

let graph = DependencyGraph::new(
    Box::new(AppDependencyProvider),
    Box::new(AppResolver),
    Box::new(|app: &App| app.name.clone()),
);

// Resolve dependency chain (returns dependencies in order)
let chain = graph.resolve_chain(&config, &app)?;

// Detect circular dependencies
graph.check_cycles(&config, &all_apps)?;
```

### Fuzzy Resolution

Typo-tolerant entity lookup:

```rust
use config_manager::resolver::HashMapResolver;

let resolver = HashMapResolver::new(true); // Enable fuzzy matching

// Typo: "fronted" instead of "frontend"
match resolver.resolve(&apps, "fronted") {
    Err(Error::NotFound { suggestion: Some(s), .. }) => {
        println!("Did you mean '{}'?", s); // "Did you mean 'frontend'?"
    }
    _ => {}
}
```

### Layered Configuration

Merge multiple configuration sources with different strategies:

```rust
use config_manager::loader::{LayeredLoader, MergeStrategy, JsonLoader};

// Load base config + environment-specific overrides
let loader = LayeredLoader::new(MergeStrategy::DeepMerge)
    .add_layer(JsonLoader::new("config/base.json"))
    .add_layer(JsonLoader::new("config/production.json"));

let manager = ConfigManager::<AppConfig>::builder()
    .loader(loader)
    .build()?;

let config = manager.load()?; // Merged config
```

**Merge Strategies:**
- `Override`: Later layers completely replace earlier ones
- `DeepMerge`: Recursively merge nested structures

### Config Watching (Hot Reload)

Watch for configuration file changes and automatically reload:

```rust
use config_manager::watch::{ConfigWatcher, ConfigEvent};

let loader = JsonLoader::new("config.json");
let mut watcher: ConfigWatcher<AppConfig> = ConfigWatcher::new(loader)?;

// Start watching with callback
watcher.start(|event| {
    match event {
        ConfigEvent::Modified => {
            println!("Config was modified - reloading!");
            // Reload your application state here
        }
        ConfigEvent::Created => println!("Config file created"),
        ConfigEvent::Removed => println!("Config file removed"),
        ConfigEvent::Error(e) => eprintln!("Watch error: {}", e),
    }
})?;

// Watcher runs in background
// Call watcher.stop() when done
```

### Performance Benchmarks

Run benchmarks to measure performance:

```bash
# Run all benchmarks
cargo bench --package config-manager

# Run specific benchmark suite
cargo bench --package config-manager --bench loader_benchmarks
cargo bench --package config-manager --bench resolver_benchmarks
```

Benchmarks cover:
- JSON/TOML/YAML loading and saving (small to large configs)
- Layered config merging (2, 5, 10 layers)
- Fuzzy matching and Levenshtein distance calculation
- Resolver operations (exact match, fuzzy suggestions, listing)

### Async Configuration Loading

Load configurations asynchronously using tokio:

```rust
use config_manager::loader::{AsyncJsonLoader, AsyncConfigLoader};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    name: String,
    port: u16,
}

#[tokio::main]
async fn main() -> config_manager::core::Result<()> {
    let loader = AsyncJsonLoader::new("config.json");

    let config: AppConfig = loader.load().await?;
    println!("Loaded: {}", config.name);

    Ok(())
}
```

### HTTP Remote Configuration

Fetch configuration from HTTP endpoints:

```rust
use config_manager::loader::{HttpLoader, AsyncConfigLoader, HttpAuth};
use std::time::Duration;

#[tokio::main]
async fn main() -> config_manager::core::Result<()> {
    let loader = HttpLoader::new("https://api.example.com/config")
        .with_auth(HttpAuth::Bearer("token123".to_string()))
        .with_timeout(Duration::from_secs(30));

    let config: AppConfig = loader.load().await?;
    Ok(())
}
```

### S3 Configuration Storage

Load configuration from AWS S3:

```rust
use config_manager::loader::{S3Loader, AsyncConfigLoader};

#[tokio::main]
async fn main() -> config_manager::core::Result<()> {
    // Uses AWS SDK's default credential chain
    let loader = S3Loader::new("my-config-bucket", "config/app.json")
        .with_region("us-east-1");

    let config: AppConfig = loader.load().await?;

    // Also supports saving
    loader.save(&config).await?;

    Ok(())
}
```

### Encrypted Configuration

Store sensitive configs with encryption:

```rust
use config_manager::loader::{EncryptedLoader, JsonLoader, ConfigLoader};

fn main() -> config_manager::core::Result<()> {
    // Wrap any loader with encryption
    let base_loader = JsonLoader::new("secrets.json");
    let loader = EncryptedLoader::new(base_loader, "my-password");

    let config = SecretConfig {
        api_key: "secret123".to_string(),
    };

    // Saves encrypted data
    loader.save(&config)?;

    // Loads and decrypts
    let loaded: SecretConfig = loader.load()?;
    Ok(())
}
```

### Schema Validation

Generate and validate against JSON schemas:

```rust
use config_manager::schema::SchemaValidator;
use config_manager::validation::Validator;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct MyConfig {
    #[schemars(range(min = 1, max = 65535))]
    port: u16,

    #[schemars(regex(pattern = r"^[a-z0-9-]+$"))]
    name: String,
}

fn main() -> config_manager::core::Result<()> {
    let validator = SchemaValidator::<MyConfig>::new();

    // Generate JSON schema
    let schema_json = validator.schema_json()?;
    println!("Schema: {}", schema_json);

    // Validate config
    let config = MyConfig {
        port: 8080,
        name: "my-app".to_string(),
    };

    let result = validator.validate(&config);
    assert!(result.is_valid());

    Ok(())
}
```

## Feature Flags

Enable additional functionality through feature flags:

```toml
[dependencies.config-manager]
version = "0.3"
features = ["toml", "yaml", "watch", "async", "http", "s3", "encryption", "schema", "full"]
```

| Feature | Description |
|---------|-------------|
| `toml` | Enable TOML file format support |
| `yaml` | Enable YAML file format support |
| `watch` | Enable configuration file watching (hot reload) |
| `async` | Enable async loader support with tokio |
| `http` | Enable HTTP/HTTPS remote configuration loading |
| `s3` | Enable AWS S3 configuration loading |
| `encryption` | Enable AES-256-GCM encryption/decryption |
| `schema` | Enable JSON Schema generation and validation |
| `interactive` | Enable interactive prompts (using `inquire`) |
| `full` | Enable all features |

## Architecture

config-manager is built on four core traits:

```
┌─────────────────┐
│ ConfigManager   │  Orchestrates everything
└────────┬────────┘
         │
    ┌────┴────┬──────────┬────────────┐
    │         │          │            │
┌───▼──┐  ┌──▼───┐  ┌───▼────┐  ┌────▼─────┐
│Loader│  │Resol-│  │Depend- │  │Validator │
│      │  │ver   │  │ency    │  │          │
└──────┘  └──────┘  └────────┘  └──────────┘
```

1. **ConfigLoader<T>** - Loads/saves configurations from various sources
2. **Resolver<C, E>** - Resolves entities by identifier with fuzzy matching
3. **DependencyProvider<E>** - Extracts dependencies from entities
4. **Validator<T>** - Validates configurations with custom rules

Each trait has multiple implementations, and you can provide your own!

## Examples

Run the included examples:

```bash
# Basic configuration loading and saving
cargo run --example simple

# Hierarchical configuration (projects → apps)
cargo run --example hierarchical

# Dependency resolution and cycle detection
cargo run --example dependencies
```

## Testing

Run the test suite:

```bash
# Run all tests
cargo test --package config-manager

# Run tests with all features
cargo test --package config-manager --all-features

# Run with verbose output
cargo test --package config-manager -- --nocapture
```

**Test Coverage**: 66+ tests covering loaders (JSON, TOML, YAML, layered), resolvers, validators, dependency graphs, file watching, and utilities.

## Performance

config-manager is designed for performance:

- **Lazy evaluation** - Only loads what you need
- **Efficient algorithms** - O(n) dependency resolution, O(n²) Levenshtein distance
- **Minimal allocations** - Uses references where possible
- **Zero-copy deserialization** - When possible with serde

## Comparison with Other Libraries

| Feature | config-manager | config-rs | figment |
|---------|---------------|-----------|---------|
| Generic types | ✅ | ✅ | ✅ |
| Dependency resolution | ✅ | ❌ | ❌ |
| Fuzzy matching | ✅ | ❌ | ❌ |
| Custom validators | ✅ | ❌ | ✅ |
| Cycle detection | ✅ | ❌ | ❌ |
| Path expansion | ✅ | ❌ | ✅ |
| Trait-based | ✅ | ❌ | ✅ |

## Real-World Usage

config-manager is used in production by:

- **devcli** - Development environment manager
- *(Add your project here!)*

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.

## License

Licensed under the PolyForm Noncommercial 1.0.0 license. See [LICENSE](../LICENSE) for details.

## Support

- **Issues**: [GitHub Issues](https://github.com/xDelph/devcli/issues)
- **Discussions**: [GitHub Discussions](https://github.com/xDelph/devcli/discussions)

## Roadmap

### Version 0.2 ✅

- [x] TOML loader implementation
- [x] YAML loader implementation
- [x] Layered config support (base + overrides)
- [x] Config watching (hot reload)
- [x] Performance benchmarks

### Version 0.3 ✅

- [x] Async loader support (AsyncConfigLoader trait)
- [x] Remote config sources (HTTP with authentication, S3)
- [x] Config encryption/decryption (AES-256-GCM)
- [x] Schema generation from types (JSON Schema)

### Version 1.0 ✅ - Production Ready!

- [x] Stable API with semver guarantees
- [x] Complete documentation and examples
- [x] Production-tested with comprehensive integration tests
- [x] Performance optimizations

## FAQ

**Q: Why another config library?**
A: Most config libraries are focused on loading and merging configs. config-manager adds dependency resolution, fuzzy matching, and validation - features needed for complex applications.

**Q: Can I use this with async/await?**
A: Currently the loaders are synchronous. Async support is planned for v0.2.

**Q: Does it support hot reloading?**
A: Not yet, but it's on the roadmap for v0.2.

**Q: How do I migrate from config-rs?**
A: See the [Migration Guide](docs/MIGRATION.md) for detailed instructions.

**Q: Can I use custom serialization formats?**
A: Yes! Implement the `ConfigLoader<T>` trait for your format.

## Acknowledgments

- Inspired by configuration management needs in the [devcli](https://github.com/xDelph/devcli) project
- Built with ♥️ using Rust

---

**Made with 🦀 Rust**
