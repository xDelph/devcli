# Changelog

All notable changes to config-manager will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-02-16

### Added - v1.0 Stable Release
- **Stable API**: All public APIs are now stable and follow semver
- Error helper methods: `is_not_found()`, `is_validation_error()`, etc.
- `#[non_exhaustive]` on Error enum for future-proofing
- Comprehensive integration tests (13 new tests covering real-world scenarios)
- Performance optimizations with `#[inline]` hints
- Complete documentation coverage

### Changed
- Error types marked as `#[non_exhaustive]` for API stability
- Optimized hot-path functions with inline hints
- Improved error messages throughout

## [0.3.0] - 2026-02-16

### Added - Production Features
- **Async Support**: Full async/await support with tokio
  - `AsyncConfigLoader` trait
  - `AsyncJsonLoader`, `AsyncTomlLoader`, `AsyncYamlLoader`
- **HTTP Remote Configuration**: Load configs from HTTP/HTTPS endpoints
  - Bearer token authentication
  - Basic authentication
  - Configurable timeouts
- **AWS S3 Support**: Load and save configs to S3
  - AWS SDK integration
  - Automatic credential resolution
- **Encryption**: AES-256-GCM encryption for sensitive configs
  - Argon2 password-based key derivation
  - Transparent encryption/decryption
  - Works with any loader
- **Schema Validation**: JSON Schema generation and validation
  - Auto-generate schemas from Rust types
  - Runtime validation with jsonschema
  - Schemars integration

### Dependencies
- Added: tokio, async-trait, reqwest, aws-sdk-s3, aws-config
- Added: aes-gcm, argon2, rand, schemars, jsonschema

## [0.2.0] - 2026-02-07

### Added - Enhanced Functionality
- **TOML Support**: Full TOML file format support
- **YAML Support**: Full YAML file format support
- **Layered Configuration**: Merge multiple config sources
  - Override strategy (complete replacement)
  - DeepMerge strategy (recursive merging)
- **File Watching**: Hot-reload on configuration changes
  - Event-based callbacks
  - Cross-platform file watching with notify
- **Performance Benchmarks**: Comprehensive benchmarks using criterion
  - Loader benchmarks (JSON/TOML/YAML)
  - Layered merging benchmarks
  - Resolver and fuzzy matching benchmarks

### Tests
- 66+ tests covering all v0.2 features
- All tests passing with all features enabled

## [0.1.0] - 2026-02-06

### Added - Initial Release
- **Core Traits**:
  - `ConfigLoader<T>` for loading/saving configurations
  - `Resolver<C, E>` for entity resolution
  - `Validator<T>` for validation
  - `DependencyProvider<E>` for dependency management
- **JSON Loader**: Built-in JSON file support
- **Dependency Resolution**: Automatic dependency chain resolution
  - Cycle detection using DFS
  - Topological sorting
- **Fuzzy Matching**: Typo-tolerant entity lookup
  - Levenshtein distance calculation
  - Automatic suggestions for typos
- **Validation Framework**: Declarative validation
  - Custom validators
  - Error and warning support
- **Path Expansion**: Tilde (~) and environment variable ($VAR) support
- **ConfigManager**: High-level orchestration
- **Builder Pattern**: Fluent API for configuration

### Tests
- 36 unit tests covering core functionality
- All tests passing

[1.0.0]: https://github.com/xDelph/devcli/releases/tag/config-manager-v1.0.0
[0.3.0]: https://github.com/xDelph/devcli/releases/tag/config-manager-v0.3.0
[0.2.0]: https://github.com/xDelph/devcli/releases/tag/config-manager-v0.2.0
[0.1.0]: https://github.com/xDelph/devcli/releases/tag/config-manager-v0.1.0
