---
inclusion: manual
---

# Test Utils Guide

## Quick Reference for Mock Data Generators

When writing tests, use the builder patterns instead of manual struct construction.

### Import Statement

```rust
use crate::test_utils::{AppBuilder, ConfigBuilder, PreferencesBuilder};
use crate::test_utils::{mock_nodejs_app, mock_redis_app, mock_app_with_deps};
```

### Common Patterns

**Simple App:**

```rust
let app = AppBuilder::new("nodejs", "/tmp/app")
    .with_local_command("start", "npm start")
    .with_local_default("start")
    .build();
```

**App with Stage:**

```rust
let app = AppBuilder::new("nodejs", "/tmp/app")
    .with_stage("qa")
    .with_env_file_path(".env.qa")
    .with_local_command("start", "npm start")
    .build();
```

**App with Dependencies:**

```rust
let app = AppBuilder::new("nodejs", "/tmp/app")
    .with_dependency("project", "redis")
    .with_local_command("start", "npm start")
    .build();
```

**Full Config:**

```rust
let config = ConfigBuilder::new()
    .with_app("project", "api", mock_nodejs_app("/tmp/api"))
    .with_app("project", "redis", mock_redis_app("/tmp/redis"))
    .build();
```

**Preferences:**

```rust
let prefs = PreferencesBuilder::new()
    .with_default_env("docker")
    .with_default_stage("qa")
    .build();
```

### Quick Helpers

- `mock_nodejs_app(path)` - Simple nodejs app with start command
- `mock_redis_app(path)` - Simple redis app with start command
- `mock_app_with_deps(path, deps)` - App with dependencies

### Benefits

- Less boilerplate code
- Only specify what matters for the test
- When models change, update builder once instead of all tests
- Tests are more readable and maintainable

See `devcli-core/TESTING.md` for full documentation.
