# Contributing to config-manager

Thank you for your interest in contributing to config-manager! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Documentation](#documentation)
- [Submitting Changes](#submitting-changes)
- [Code Style](#code-style)
- [Architecture](#architecture)

## Code of Conduct

Be respectful, constructive, and professional in all interactions.

## Getting Started

### Prerequisites

- Rust 1.70 or later
- Cargo
- Git

### Fork and Clone

```bash
# Fork the repository on GitHub
# Then clone your fork
git clone https://github.com/YOUR_USERNAME/devcli.git
cd devcli/config-manager
```

### Build and Test

```bash
# Build the project
cargo build

# Run tests
cargo test --package config-manager

# Run tests with all features
cargo test --package config-manager --all-features
```

## Development Setup

### Project Structure

```
config-manager/
├── src/
│   ├── core/          # ConfigManager and orchestration
│   ├── loader/        # Configuration loaders
│   ├── resolver/      # Entity resolution
│   ├── dependencies/  # Dependency management
│   ├── validation/    # Configuration validation
│   └── utils/         # Shared utilities
├── examples/          # Usage examples
├── docs/              # Documentation
└── tests/             # Integration tests
```

### Running Examples

```bash
# Run a specific example
cargo run --example simple
cargo run --example hierarchical
cargo run --example dependencies
```

### Continuous Testing

```bash
# Watch for changes and run tests
cargo watch -x "test --package config-manager"
```

## Making Changes

### Branching Strategy

Create a feature branch from `develop`:

```bash
git checkout develop
git pull origin develop
git checkout -b feature/your-feature-name
```

Branch naming conventions:
- `feature/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation updates
- `refactor/` - Code refactoring
- `test/` - Test additions/improvements

### Commit Messages

Follow conventional commits format:

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types:
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation changes
- `style` - Code style changes (formatting, etc.)
- `refactor` - Code refactoring
- `test` - Adding/updating tests
- `chore` - Build process, dependencies, etc.

Examples:

```bash
git commit -m "feat(loader): add TOML file support"
git commit -m "fix(resolver): handle empty HashMap gracefully"
git commit -m "docs(api): update ConfigManager examples"
```

## Testing

### Writing Tests

All new features must include tests.

#### Unit Tests

Add tests in the same file as your code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_feature() {
        // Arrange
        let input = create_test_data();

        // Act
        let result = my_function(input);

        // Assert
        assert_eq!(result, expected_value);
    }
}
```

#### Integration Tests

Add tests in `tests/integration/`:

```rust
// tests/integration/my_feature_test.rs

use config_manager::prelude::*;

#[test]
fn test_end_to_end_workflow() {
    // Test complete workflows
}
```

### Running Tests

```bash
# Run all tests
cargo test --package config-manager

# Run specific test
cargo test --package config-manager test_name

# Run with output
cargo test --package config-manager -- --nocapture

# Run with all features
cargo test --package config-manager --all-features
```

### Test Coverage

Aim for high test coverage:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --package config-manager --out Html
```

## Documentation

### Code Documentation

All public APIs must have rustdoc comments:

```rust
/// Loads configuration from a source.
///
/// # Examples
///
/// ```rust
/// use config_manager::prelude::*;
///
/// let manager = ConfigManager::<MyConfig>::builder()
///     .loader(JsonLoader::new("config.json"))
///     .build()?;
///
/// let config = manager.load()?;
/// ```
///
/// # Errors
///
/// Returns `Error::LoadError` if the configuration cannot be loaded.
pub fn load(&self) -> Result<T> {
    // ...
}
```

### Documentation Comments

- Use `///` for public items
- Use `//!` for module-level documentation
- Include examples where applicable
- Document errors, panics, and safety requirements
- Keep examples up-to-date

### Building Documentation

```bash
# Build documentation
cargo doc --package config-manager --no-deps --open

# Build with all features
cargo doc --package config-manager --all-features --no-deps --open
```

## Submitting Changes

### Before Submitting

Checklist:

- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] New features have tests
- [ ] Documentation is updated
- [ ] Examples work
- [ ] Code follows style guidelines
- [ ] Commit messages follow conventions

### Pull Request Process

1. **Update your branch**

```bash
git checkout develop
git pull origin develop
git checkout your-feature-branch
git rebase develop
```

2. **Push to your fork**

```bash
git push origin your-feature-branch
```

3. **Create Pull Request**

- Go to GitHub
- Click "New Pull Request"
- Select your branch
- Fill in the PR template

### Pull Request Template

```markdown
## Description

Brief description of what this PR does.

## Motivation

Why is this change needed?

## Changes

- Change 1
- Change 2
- Change 3

## Testing

How was this tested?

## Checklist

- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] Examples updated (if applicable)
- [ ] All tests pass
- [ ] No compiler warnings
```

## Code Style

### Rust Style

Follow the [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/):

```bash
# Format code
cargo fmt --package config-manager

# Check formatting
cargo fmt --package config-manager -- --check

# Run clippy
cargo clippy --package config-manager -- -D warnings
```

### Naming Conventions

- **Types**: `PascalCase` (e.g., `ConfigManager`, `JsonLoader`)
- **Functions**: `snake_case` (e.g., `load_config`, `resolve_chain`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `MAX_RETRIES`)
- **Modules**: `snake_case` (e.g., `config_loader`, `dependency_graph`)

### Error Handling

- Use `Result<T>` for fallible operations
- Use `Option<T>` for optional values
- Don't panic in library code
- Provide descriptive error messages

```rust
// Good
pub fn resolve(&self, id: &str) -> Result<Entity> {
    self.entities.get(id)
        .cloned()
        .ok_or_else(|| Error::NotFound {
            id: id.to_string(),
            suggestion: self.suggest(id),
        })
}

// Bad
pub fn resolve(&self, id: &str) -> Entity {
    self.entities.get(id).unwrap()  // DON'T PANIC!
}
```

### Trait Implementations

- All public traits should have documentation
- Provide examples in trait documentation
- Consider default implementations
- Make traits object-safe when possible

```rust
/// Trait for loading configuration.
///
/// # Examples
///
/// ```rust
/// use config_manager::loader::ConfigLoader;
///
/// struct MyLoader;
///
/// impl<T> ConfigLoader<T> for MyLoader {
///     fn load(&self) -> Result<T> {
///         // Implementation
///     }
///     // ...
/// }
/// ```
pub trait ConfigLoader<T>: Send + Sync + Debug {
    fn load(&self) -> Result<T>;
    // ...
}
```

## Architecture

### Design Principles

When contributing, follow these principles:

1. **Trait-based composition** - New features should be trait-based
2. **Zero-cost abstractions** - Avoid runtime overhead
3. **Type safety** - Leverage Rust's type system
4. **Explicit over implicit** - Clear, predictable APIs
5. **Generic by default** - Support any configuration structure

### Adding New Features

#### Adding a New Loader

1. Implement the `ConfigLoader<T>` trait
2. Add tests
3. Add example
4. Document in API.md

```rust
// src/loader/my_loader.rs

use crate::core::Result;
use crate::loader::ConfigLoader;

pub struct MyLoader {
    // Fields
}

impl<T> ConfigLoader<T> for MyLoader
where
    T: DeserializeOwned + Serialize,
{
    fn load(&self) -> Result<T> {
        // Implementation
    }

    fn save(&self, config: &T) -> Result<()> {
        // Implementation
    }

    fn exists(&self) -> bool {
        // Implementation
    }

    fn source_info(&self) -> String {
        // Implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_loader() {
        // Tests
    }
}
```

#### Adding a New Resolver

Similar process - implement `Resolver<C, E>` trait.

#### Adding a New Validator

Implement `Validator<T>` trait.

### Performance Considerations

- Profile before optimizing
- Use `#[inline]` judiciously
- Avoid unnecessary allocations
- Consider async for I/O-heavy operations (future)
- Document performance characteristics

### Breaking Changes

Avoid breaking changes. If necessary:

1. Document in CHANGELOG
2. Provide migration guide
3. Consider deprecation period
4. Bump major version (semver)

## Feature Flags

When adding optional features:

```toml
# Cargo.toml

[features]
my-feature = ["dep:my-dependency"]
```

Gate code appropriately:

```rust
#[cfg(feature = "my-feature")]
pub mod my_feature;

#[cfg(feature = "my-feature")]
pub use my_feature::MyFeature;
```

## Questions?

- Open an issue for questions
- Start a discussion on GitHub
- Check existing documentation
- Look at examples

## Recognition

Contributors will be acknowledged in:
- CHANGELOG.md
- GitHub contributors page
- README.md (for significant contributions)

Thank you for contributing to config-manager!
