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

let task = Task {
    id: "my-service".to_string(),
    command: "npm".to_string(),
    args: vec!["run".to_string(), "dev".to_string()],
    working_dir: std::env::current_dir()?,
    env: HashMap::new(),
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

## Documentation

- [Architecture Guide](docs/architecture.md)
- [Usage Examples](docs/usage.md)
- [API Reference](https://docs.rs/process-manager) (Coming soon)

## License

Licensed under PolyForm Noncommercial 1.0.0.
