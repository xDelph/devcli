# Usage Guide

This guide provides practical examples for common tasks using the `process-manager` library.

## 1. Handling Real-time Output

The `output_rx` channel provides a structured stream of everything the process emits.

```rust
use process_manager::engine;
use process_manager::model::OutputSource;

let running = engine::spawn(&my_task).await?;

// Process logs until the app exits
let mut rx = running.output_rx;
while let Some(msg) = rx.recv().await {
    match msg.source {
        OutputSource::Stdout => println!("[INFO] {}", msg.content),
        OutputSource::Stderr => eprintln!("[ERROR] {}", msg.content),
    }
    
    // Example: Logic-based action
    if msg.content.contains("Database initialized") {
        launch_other_service().await?;
    }
}
```

## 2. Managing Restarts

You can manually trigger a restart using the `engine::restart` function, which automatically handles cleanup of the old process group.

```rust
use process_manager::engine;
use std::time::Duration;

// Assuming you retrieved old_pid and old_pgid from the StateStore
engine::restart(
    &task,
    old_pid,
    old_pgid,
    Duration::from_secs(5) // Wait 5s before spawning new one
).await?;
```

## 3. Querying State

The `StateStore` allows you to audit the entire system.

```rust
use process_manager::StateStore;
use std::path::PathBuf;

let store = StateStore::new(PathBuf::from("./state"))?;

// List all known processes
let all = store.list()?;
for proc in all {
    let status = if store.is_running(&proc) { "ALIVE" } else { "DEAD" };
    println!("App: {} | PID: {} | Status: {}", proc.id, proc.pid, status);
    
    // Access metadata
    if let Some(project) = proc.metadata.get("project") {
        println!("  Project: {}", project);
    }
}
```

## 4. Configuring Health Checks

`process-manager` supports multiple check types.

### HTTP Check
```rust
HealthCheck::Http {
    url: "http://127.0.0.1:8080/health".to_string(),
    timeout_secs: 2,
    expected_status: 200,
}
```

### TCP Check (Databases)
```rust
HealthCheck::Tcp {
    host: "127.0.0.1".to_string(),
    port: 5432,
    timeout_secs: 5,
}
```

### Command Check
```rust
HealthCheck::Command {
    command: "grep -q 'ready' /tmp/status.txt".to_string(),
    timeout_secs: 5,
    expected_exit_code: 0,
}
```
