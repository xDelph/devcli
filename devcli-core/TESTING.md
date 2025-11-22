# Testing Guide

## Mock Data Generators

To make tests easier to write and maintain, we provide builder patterns for creating test fixtures.

### AppBuilder

Create App instances with sensible defaults:

```rust
use crate::test_utils::AppBuilder;

// Minimal app
let app = AppBuilder::new("nodejs", "/tmp/app").build();

// App with commands
let app = AppBuilder::new("nodejs", "/tmp/app")
    .with_local_command("start", "npm start")
    .with_local_command("test", "npm test")
    .with_local_default("start")
    .build();

// App with stage and env file
let app = AppBuilder::new("nodejs", "/tmp/app")
    .with_stage("qa")
    .with_env_file_path("config/.env.qa")
    .with_local_command("start", "npm start")
    .build();

// App with dependencies
let app = AppBuilder::new("nodejs", "/tmp/app")
    .with_dependency("project", "redis")
    .with_dependency("project", "postgres")
    .with_local_command("start", "npm start")
    .build();

// App with docker commands
let app = AppBuilder::new("nodejs", "/tmp/app")
    .with_docker_command("build", "docker build -t myapp .")
    .with_docker_command("run", "docker run myapp")
    .with_docker_default("run")
    .build();
```

### ConfigBuilder

Create Config instances with multiple projects and apps:

```rust
use crate::test_utils::{ConfigBuilder, AppBuilder, mock_nodejs_app, mock_redis_app};

// Using builder
let config = ConfigBuilder::new()
    .with_app("project1", "api", AppBuilder::new("nodejs", "/tmp/api")
        .with_local_command("start", "npm start")
        .build())
    .with_app("project1", "redis", mock_redis_app("/tmp/redis"))
    .with_app("project2", "frontend", mock_nodejs_app("/tmp/frontend"))
    .build();
```

### PreferencesBuilder

Create Preferences instances:

```rust
use crate::test_utils::PreferencesBuilder;

let prefs = PreferencesBuilder::new()
    .with_default_env("docker")
    .with_default_stage("qa")
    .with_detached_mode(true)
    .with_auto_start_deps(false)
    .build();
```

### Quick Helpers

For common scenarios, use the quick helper functions:

```rust
use crate::test_utils::{mock_nodejs_app, mock_redis_app, mock_app_with_deps};

// Simple nodejs app
let app = mock_nodejs_app("/tmp/app");

// Simple redis app
let app = mock_redis_app("/tmp/redis");

// App with dependencies
let app = mock_app_with_deps("/tmp/app", vec![
    ("project", "redis"),
    ("project", "postgres"),
]);
```

## Before and After Examples

### Before (Manual Construction)

```rust
#[test]
fn test_something() {
    let mut apps = HashMap::new();
    apps.insert(
        "api".to_string(),
        App {
            app_type: "nodejs".to_string(),
            path: "/tmp/api".to_string(),
            commands: Commands {
                local: Some({
                    let mut cmds = HashMap::new();
                    cmds.insert("start".to_string(), "npm start".to_string());
                    cmds
                }),
                docker: None,
                orbstack: None,
                k8s: None,
            },
            dependencies: Vec::new(),
            defaults: Defaults {
                local: Some("start".to_string()),
                docker: None,
                orbstack: None,
                k8s: None,
            },
            dockerfile_path: None,
            stage: None,
            env_file_path: None,
        },
    );

    let mut projects = HashMap::new();
    projects.insert("test".to_string(), Project { apps });
    let config = Config { projects };

    // ... test code
}
```

### After (Using Builders)

```rust
#[test]
fn test_something() {
    let config = ConfigBuilder::new()
        .with_app("test", "api", AppBuilder::new("nodejs", "/tmp/api")
            .with_local_command("start", "npm start")
            .with_local_default("start")
            .build())
        .build();

    // ... test code
}
```

## Benefits

1. **Less Boilerplate**: Builders handle the verbose struct initialization
2. **Sensible Defaults**: Only specify what matters for your test
3. **Maintainability**: When App struct changes, update the builder once instead of hundreds of tests
4. **Readability**: Test intent is clearer when you only see relevant fields
5. **Type Safety**: Compile-time checks ensure all required fields are set

## Migration Strategy

When updating existing tests:

1. Start with new tests - use builders from the beginning
2. When touching old tests, refactor them to use builders
3. No need to update all tests at once - gradual migration is fine
4. Focus on tests that are hard to read or frequently break when models change
