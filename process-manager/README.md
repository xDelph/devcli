# Process Manager

A high-performance, standalone Rust library for managing local process lifecycles. Designed for developer tools, supervisors, and task orchestrators.

## Features

- 🚀 **Agnostic Execution**: Spawn any shell command as a managed process.
- 👥 **Process Group Tracking**: Robust **PGID** support ensures that child processes are tracked and terminated even if the parent exits.
- 🛡️ **Cross-Process Safety**: File-based locking prevents race conditions between multiple instances (e.g., CLI vs. background Daemon).
- 💚 **Pluggable Health Checks**: Built-in support for HTTP, TCP, and custom command-based health monitoring.
- 🔄 **Self-Healing**: Configurable restart policies with exponential backoff and time-window limits.
- 📝 **Structured Output**: Unified, asynchronous stream for `stdout` and `stderr` with automatic background log file persistence.
- 💾 **Persistent State**: JSON-based state store that survives process exits and system reboots.

## Architecture

The crate is divided into several modular components:

- **Engine**: Low-level process spawning, termination, and group management.
- **State**: Persistent storage and robust liveness checks.
- **Monitor**: Background loop for health monitoring and auto-recovery.
- **Restart**: Coordination logic for safe, synchronized restarts.
- **Health**: Execution engine for various health check protocols.

## Quick Start

### 1. Define a Task
```rust
use process_manager::Task;
use std::collections::HashMap;

let mut env = HashMap::new();
env.insert("NODE_ENV".to_string(), "development".to_string());

let task = Task {
    id: "my-service".to_string(),
    command: "npm run dev".to_string(),
    args: vec!["--port".to_string(), "3000".to_string()],
    working_dir: std::env::current_dir()?,
    env,
    is_detached: true,
    log_file: Some("service.log".into()),
    health_check: process_manager::HealthCheck::Http {
        url: "http://localhost:3000/health".to_string(),
        timeout_secs: 5,
        expected_status: 200,
    },
    restart_policy: Default::default(),
};
```

### 2. Spawn and Listen
```rust
use process_manager::engine;

let mut running = engine::spawn(&task).await?;
println!("Started with PID: {}", running.pid);

// Listen to output in real-time
while let Some(msg) = running.output_rx.recv().await {
    println!("[{}] {}", msg.source, msg.content);
}
```

### 3. Background Monitoring
```rust
use process_manager::{Monitor, StateStore, HealthCheckEngine, RestartCoordinator};
use std::sync::Arc;

let state = Arc::new(StateStore::new("./state")?);
let monitor = Monitor::new(
    state,
    Arc::new(HealthCheckEngine::new()),
    Arc::new(RestartCoordinator::new("./locks".into())),
);

monitor.run().await?;
```

## Process Metadata

The `process-manager` crate uses a flexible metadata system to store arbitrary key-value pairs about processes. This allows integrators like `devcli` to track application-specific information without coupling the crate to specific use cases.

### Standard Metadata Fields

While metadata is completely flexible, the following fields are recommended for `devcli` integration:

| Field | Type | Description | Example |
|-------|------|-------------|---------|
| `project` | String | Project/repository name | `"my-webapp"` |
| `app_config_name` | String | Clean application name from config | `"api-server"` |
| `environment` | String | Deployment environment | `"dev"`, `"qa"`, `"prod"` |
| `stage` | String | Deployment stage (if different from environment) | `"staging"`, `"production"` |
| `command_variant` | String | Which command variant was used | `"start"`, `"dev"`, `"build"` |

### Setting Metadata

```rust
use std::collections::HashMap;
use process_manager::{ManagedProcess, StateStore};

let mut metadata = HashMap::new();
metadata.insert("project".to_string(), "my-webapp".to_string());
metadata.insert("app_config_name".to_string(), "api-server".to_string());
metadata.insert("environment".to_string(), "dev".to_string());

let process = ManagedProcess {
    id: "my-webapp.api-server.dev".to_string(),
    // ... other fields
    metadata,
    // ...
};
```

### Querying by Metadata

```rust
use process_manager::StateStore;

let store = StateStore::new("./state".into())?;

// Find all processes in development environment
let dev_processes = store.find_by_metadata("environment", "dev")?;

// Find all processes for a specific project
let project_processes = store.find_by_metadata("project", "my-webapp")?;

// Find a single process by unique identifier
let process = store.find_one_by_metadata("app_config_name", "api-server")?;
```

### Recommended ID Format

For compatibility with existing `devcli` PID files, use the format:
```
{project}.{app_config_name}.{environment}
```

Example: `"my-webapp.api-server.dev"`

This allows easy migration from the old `ProcessTracker` system while maintaining backward-compatible file naming.

## Documentation

- [Architecture Guide](docs/architecture.md)
- [Usage Examples](docs/usage.md)
- [API Reference](https://docs.rs/process-manager) (Coming soon)

## License

Licensed under PolyForm Noncommercial 1.0.0.
