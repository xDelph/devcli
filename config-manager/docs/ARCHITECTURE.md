# Architecture Guide

This document describes the design decisions, architecture, and implementation details of config-manager.

## Table of Contents

- [Design Philosophy](#design-philosophy)
- [Core Concepts](#core-concepts)
- [Trait System](#trait-system)
- [Module Organization](#module-organization)
- [Data Flow](#data-flow)
- [Design Decisions](#design-decisions)
- [Performance Considerations](#performance-considerations)
- [Extensibility Points](#extensibility-points)

## Design Philosophy

config-manager follows several key principles:

1. **Trait-based composition** - Everything is pluggable through traits
2. **Zero-cost abstractions** - No runtime overhead for features you don't use
3. **Type safety** - Leverage Rust's type system for correctness
4. **Explicit over implicit** - No magic, clear APIs
5. **Generic by default** - Works with any configuration structure

## Core Concepts

### Configuration Manager

The `ConfigManager<T>` is the main orchestrator that ties together:

- **Loader** - How to read/write configs
- **Validators** - Rules for correctness
- **Configuration type** - Your domain model

```rust
pub struct ConfigManager<T> {
    loader: Box<dyn ConfigLoader<T>>,
    validators: Vec<Box<dyn Validator<T>>>,
}
```

The manager is generic over `T`, your configuration type, allowing complete flexibility.

### Separation of Concerns

config-manager separates four distinct concerns:

1. **Loading/Saving** (`loader/`) - How configs are persisted
2. **Resolution** (`resolver/`) - How entities are found
3. **Dependencies** (`dependencies/`) - How entities relate
4. **Validation** (`validation/`) - How correctness is ensured

Each concern is independent and can be used separately.

## Trait System

### ConfigLoader<T>

Responsible for loading and saving configurations.

```rust
pub trait ConfigLoader<T>: Send + Sync + Debug {
    fn load(&self) -> Result<T>;
    fn save(&self, config: &T) -> Result<()>;
    fn exists(&self) -> bool;
    fn source_info(&self) -> String;
}
```

**Implementations:**
- `JsonLoader` - JSON files with path expansion
- `TomlLoader` (planned) - TOML files
- `YamlLoader` (planned) - YAML files
- **User-defined** - HTTP, database, etc.

**Design decision**: The trait is generic over `T` rather than using `dyn Any` or similar. This provides:
- Compile-time type safety
- No runtime type checking overhead
- Clear API contracts

### Resolver<C, E>

Responsible for resolving entities within a configuration.

```rust
pub trait Resolver<C, E>: Send + Sync {
    fn resolve(&self, config: &C, id: &str) -> Result<E>;
    fn list_ids(&self, config: &C) -> Vec<String>;
    fn suggest(&self, config: &C, id: &str) -> Option<String>;
}
```

**Type parameters:**
- `C` - Configuration type to search within
- `E` - Entity type to resolve

**Implementations:**
- `HashMapResolver` - For `HashMap<String, V>` structures
- **User-defined** - Path-based, SQL-like, etc.

**Design decision**: Two generic parameters allow flexibility:
- Config structure can be different from entity type
- Enables nested resolution (project.app)
- Allows multiple resolver types per config

### DependencyProvider<E>

Extracts dependencies from entities.

```rust
pub trait DependencyProvider<E>: Send + Sync {
    fn dependencies(&self, entity: &E) -> Vec<String>;
}
```

**Design decision**: Returns `Vec<String>` (identifiers) rather than entities:
- Decouples dependency extraction from resolution
- Allows lazy resolution
- Simpler implementation

### Validator<T>

Validates configurations.

```rust
pub trait Validator<T>: Send + Sync {
    fn validate(&self, config: &T) -> ValidationResult;
}
```

**Design decision**: Returns `ValidationResult` with errors AND warnings:
- Non-fatal issues can be warnings
- Accumulate all errors at once
- Better UX than fail-fast

## Module Organization

```
src/
├── core/           # Orchestration layer
│   ├── error.rs    # Error types
│   ├── manager.rs  # ConfigManager
│   └── builder.rs  # Builder pattern
│
├── loader/         # Loading/saving
│   ├── traits.rs   # ConfigLoader trait
│   ├── json.rs     # JSON implementation
│   ├── toml.rs     # TOML (feature-gated)
│   └── yaml.rs     # YAML (feature-gated)
│
├── resolver/       # Entity resolution
│   ├── traits.rs   # Resolver trait
│   ├── hashmap.rs  # HashMap implementation
│   └── fuzzy.rs    # Fuzzy matching utilities
│
├── dependencies/   # Dependency management
│   ├── traits.rs   # DependencyProvider trait
│   └── graph.rs    # Dependency graph
│
├── validation/     # Validation
│   ├── traits.rs   # Validator trait
│   └── result.rs   # ValidationResult types
│
└── utils/          # Shared utilities
    ├── path.rs     # Path expansion
    └── string.rs   # String utilities
```

**Design rationale:**

1. **Separation by concern** - Each module is independent
2. **Traits in separate files** - Clear API boundaries
3. **Implementations alongside traits** - Easy to find
4. **Utils are leaf nodes** - No circular dependencies

## Data Flow

### Loading Configuration

```
User Code
   │
   ├─> ConfigManager::load()
   │      │
   │      ├─> ConfigLoader::load()
   │      │      │
   │      │      ├─> Read file/source
   │      │      ├─> Deserialize (serde)
   │      │      └─> Return config
   │      │
   │      └─> Return config
   │
   └─> Config
```

### Saving Configuration

```
User Code
   │
   ├─> ConfigManager::save(config)
   │      │
   │      ├─> For each Validator
   │      │      ├─> Validator::validate(config)
   │      │      └─> Collect errors
   │      │
   │      ├─> If invalid, return error
   │      │
   │      └─> ConfigLoader::save(config)
   │             │
   │             ├─> Serialize (serde)
   │             ├─> Write file/source
   │             └─> Return Ok
   │
   └─> Result
```

### Resolving Dependencies

```
User Code
   │
   ├─> DependencyGraph::resolve_chain(config, entity)
   │      │
   │      ├─> DFS traversal
   │      │      │
   │      │      ├─> DependencyProvider::dependencies(entity)
   │      │      │      └─> Return Vec<String>
   │      │      │
   │      │      ├─> For each dependency ID
   │      │      │      │
   │      │      │      ├─> Resolver::resolve(config, id)
   │      │      │      │      └─> Return entity
   │      │      │      │
   │      │      │      └─> Recurse
   │      │      │
   │      │      └─> Detect cycles
   │      │
   │      └─> Return Vec<Entity> (dependency order)
   │
   └─> Dependency chain
```

## Design Decisions

### 1. Generic Over Configuration Type

**Decision**: Make `ConfigManager<T>` generic rather than using trait objects.

**Rationale:**
- ✅ Type safety at compile time
- ✅ No runtime type checking
- ✅ Better error messages
- ✅ Zero-cost abstraction
- ❌ Slightly more complex API

**Alternatives considered:**
- Use `serde_json::Value` - Too loose, loses type safety
- Use trait objects - Runtime overhead, less ergonomic

### 2. Trait Objects for Plugins

**Decision**: Use `Box<dyn Trait>` for loaders, validators, etc.

**Rationale:**
- ✅ Allows mixing implementations
- ✅ Dynamic dispatch enables plugins
- ✅ Simple API
- ❌ Small runtime cost (vtable lookup)

**Trade-off**: We accept small runtime cost for flexibility. Most operations are I/O bound anyway.

### 3. Synchronous API

**Decision**: Make all operations synchronous (not async).

**Rationale:**
- ✅ Simpler API
- ✅ Most config operations are blocking anyway (file I/O)
- ✅ Can wrap in async if needed
- ❌ Doesn't work with async-only sources

**Future**: Add async support in v0.2 with separate trait.

### 4. Error Handling with anyhow

**Decision**: Use custom `Error` enum, but support anyhow interop.

**Rationale:**
- ✅ Specific error types for library users
- ✅ Still works with anyhow-based apps
- ✅ Better error messages
- ✅ Can pattern match on errors

### 5. Builder Pattern for Construction

**Decision**: Use builder pattern for `ConfigManager`.

**Rationale:**
- ✅ Clear, discoverable API
- ✅ Optional components
- ✅ Type-safe construction
- ✅ Familiar pattern

### 6. Levenshtein for Fuzzy Matching

**Decision**: Use Levenshtein distance (max 2) for suggestions.

**Rationale:**
- ✅ Good for typos (cat → hat, cat → cats)
- ✅ Fast enough for config-sized data
- ✅ No external dependencies
- ❌ Not ideal for very different strings

**Alternatives considered:**
- Jaro-Winkler - Better for names, more complex
- Soundex - Better for phonetic, too loose
- N-grams - Too heavyweight

### 7. DFS for Dependency Resolution

**Decision**: Use depth-first search with cycle detection.

**Rationale:**
- ✅ Natural recursive implementation
- ✅ Detects cycles during traversal
- ✅ O(n) time complexity
- ✅ Simple to understand

**Alternatives considered:**
- BFS - Same complexity, less natural
- Tarjan's algorithm - Overkill for simple graphs

### 8. Kahn's Algorithm for Topological Sort

**Decision**: Use Kahn's algorithm for ordering entities.

**Rationale:**
- ✅ O(n + e) time complexity
- ✅ Simple implementation
- ✅ Detects cycles
- ✅ Stable ordering

## Performance Considerations

### Time Complexity

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Load config | O(n) | n = file size, dominated by I/O |
| Save config | O(n) | n = file size, dominated by I/O |
| Resolve entity | O(k) | k = number of entities to search |
| Fuzzy match | O(k × m²) | k = candidates, m = string length |
| Dependency chain | O(n + e) | n = nodes, e = edges |
| Topological sort | O(n + e) | Kahn's algorithm |
| Cycle detection | O(n + e) | During DFS |

### Memory Usage

- **ConfigManager**: ~100 bytes + validators
- **JsonLoader**: ~200 bytes + path string
- **DependencyGraph**: O(n) for visited sets
- **Fuzzy matching**: O(m²) for distance matrix

### Optimization Strategies

1. **Lazy loading** - Only load config when needed
2. **Caching** - Cache dependency chains (future)
3. **Early termination** - Stop fuzzy search when exact match found
4. **Minimal allocations** - Use references where possible

### Bottlenecks

1. **I/O operations** - Loading/saving files (unavoidable)
2. **Deserialization** - Parsing JSON/TOML/YAML (serde)
3. **Fuzzy matching** - O(k × m²) when searching many entities

**Mitigation:**
- I/O: Can't improve much, already async-ready
- Deserialization: Use efficient formats (bincode for internal)
- Fuzzy matching: Only run when exact match fails

## Extensibility Points

### Custom Loaders

Implement `ConfigLoader<T>` for any source:

```rust
pub struct HttpLoader {
    url: String,
    client: reqwest::Client,
}

impl<T: DeserializeOwned> ConfigLoader<T> for HttpLoader {
    fn load(&self) -> Result<T> {
        // Fetch from HTTP
    }
    // ...
}
```

### Custom Resolvers

Implement `Resolver<C, E>` for any lookup strategy:

```rust
pub struct PathResolver;

impl Resolver<Config, Value> for PathResolver {
    fn resolve(&self, config: &Config, path: &str) -> Result<Value> {
        // Parse path like "projects.web.apps.frontend"
        // Navigate nested structure
    }
}
```

### Custom Validators

Implement `Validator<T>` for any validation logic:

```rust
pub struct SchemaValidator {
    schema: JsonSchema,
}

impl Validator<Config> for SchemaValidator {
    fn validate(&self, config: &Config) -> ValidationResult {
        // Validate against JSON Schema
    }
}
```

### Custom Dependency Providers

Implement `DependencyProvider<E>` for any dependency model:

```rust
pub struct ComputedDependencyProvider;

impl DependencyProvider<Service> for ComputedDependencyProvider {
    fn dependencies(&self, service: &Service) -> Vec<String> {
        // Compute dependencies dynamically
        // Based on service type, configuration, etc.
    }
}
```

## Thread Safety

All traits require `Send + Sync`:

- **ConfigManager** is `Send + Sync` if `T: Send + Sync`
- **Loaders** must be thread-safe
- **Resolvers** must be thread-safe
- **Validators** must be thread-safe

This allows:
- Sharing `ConfigManager` across threads
- Concurrent validation
- Parallel resolution (future)

## Error Handling Strategy

### Error Types

```rust
pub enum Error {
    LoadError(String),
    SaveError(String),
    NotFound { id: String, suggestion: Option<String> },
    Ambiguous { id: String, candidates: Vec<String> },
    CircularDependency { path: Vec<String> },
    ValidationError(String),
    // ...
}
```

### Error Propagation

1. **Library errors** - Use `Error` enum
2. **I/O errors** - Convert to `Error::LoadError`/`SaveError`
3. **Serialization errors** - Convert to `Error::SerdeError`
4. **User errors** - Descriptive messages with context

### Best Practices

- ✅ Include file path in error messages
- ✅ Suggest alternatives for NotFound
- ✅ Show full path for CircularDependency
- ✅ Accumulate all validation errors
- ❌ Don't panic in library code
- ❌ Don't log errors (let user decide)

## Testing Strategy

### Unit Tests

Each module has its own unit tests:

```
src/loader/json.rs       → Tests JSON loading/saving
src/resolver/hashmap.rs  → Tests HashMap resolution
src/dependencies/graph.rs → Tests dependency resolution
src/utils/path.rs        → Tests path expansion
```

### Integration Tests

Complex scenarios combining multiple components:

```
tests/integration/
├── basic_usage.rs    → End-to-end config loading
├── complex_deps.rs   → Multi-level dependencies
└── validation.rs     → Validation pipeline
```

### Property-Based Tests

Invariants that should always hold:

- Levenshtein distance is symmetric
- Topological sort respects dependencies
- Load → Save → Load is idempotent

## Future Architecture

### Implemented in v0.2-v1.0

1. ✅ **Async support** - Full async/await with `AsyncConfigLoader` trait
2. ✅ **Watch support** - File watching with hot-reload via `ConfigWatcher`
3. ✅ **Schema system** - JSON Schema generation via `SchemaValidator`
4. ✅ **Layered configs** - Multi-source merging with `LayeredLoader`
5. ✅ **Remote sources** - HTTP and S3 loaders
6. ✅ **Encryption** - AES-256-GCM via `EncryptedLoader`
7. ✅ **Multiple formats** - TOML and YAML support
8. ✅ **Performance** - Benchmarks and optimizations

### Non-Goals

- ❌ Dynamic typing (use serde_json::Value if needed)
- ❌ Complex query language (keep it simple)
- ❌ Built-in migration system (use external tools)
- ❌ GUI configuration editor (out of scope)

## Conclusion

config-manager's architecture is designed for:

- **Flexibility** - Works with any config structure
- **Extensibility** - All components are pluggable
- **Type Safety** - Leverage Rust's type system
- **Performance** - Zero-cost abstractions where possible
- **Simplicity** - Clear, minimal API

The trait-based design allows the library to grow without breaking existing code, while the generic type system ensures type safety at compile time.
