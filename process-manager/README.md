# Process Manager

A high-performance, standalone Rust library for managing local process lifecycles. Designed for developer tools, supervisors, and task orchestrators.

## Features

- **Agnostic Execution**: Spawn any shell command as a managed process.
- **Process Group Tracking**: Robust PGID support ensures child processes are tracked and terminated even if the parent exits.
- **Self-Managing Daemon**: `pm-daemon` starts automatically when the first process is launched and exits when none remain — no user interaction required.
- **Cross-Process Safety**: File-based locking prevents race conditions between multiple clients and the daemon.
- **Pluggable Health Checks**: Built-in HTTP, TCP, and command-based health monitoring.
- **Self-Healing**: Configurable restart policies with exponential backoff and time-window limits.
- **Structured Output**: Unified async stream for `stdout`/`stderr` with automatic log file persistence.
- **Persistent State**: JSON-based state store that survives process exits and reboots.

## Architecture

```
process-manager/
├── engine      — Process spawning, termination, group management, liveness probe
├── state       — JSON persistence, daemon lifecycle (ensure_daemon_running)
├── monitor     — Health check + auto-restart loop (runs inside pm-daemon)
├── restart     — File-lock coordination preventing thundering-herd restarts
├── health      — HTTP / TCP / Command health check execution
└── bin/
    └── pm-daemon — Background singleton daemon (spawned automatically)
```

### Directory layout (state_dir)

```
~/.devcli/processes/          ← default state_dir for devcli
  <id>.json                   ← one file per managed process
  .daemon.lock                ← exclusive lock held by pm-daemon while alive
  .status_changed             ← touched on every save/delete (TUI poll target)
  locks/
    <id>.lock                 ← per-process restart coordination lock
```

All daemon infrastructure lives **inside** `state_dir`. Callers choose where `state_dir` points; the library manages everything beneath it.

## Quick Start

### 1. Define a Task

```rust
use process_manager::Task;
use std::collections::HashMap;

let task = Task {
    id: "my-service".to_string(),
    command: "npm run dev".to_string(),
    args: vec![],
    working_dir: std::env::current_dir()?,
    env: HashMap::new(),
    is_detached: true,
    log_file: Some("~/.devcli/logs/my-service.log".into()),
    health_check: process_manager::HealthCheck::Http {
        url: "http://localhost:3000/health".to_string(),
        timeout_secs: 5,
        expected_status: 200,
    },
    restart_policy: Default::default(),
};
```

### 2. Spawn, Persist, and Enable Monitoring

```rust
use process_manager::{engine, ManagedProcess, StateStore};
use std::sync::Arc;

let store = StateStore::new("/path/to/state_dir".into())?;

// Spawn the process
let running = engine::spawn(&task).await?;

// Verify it started successfully — no raw PID needed
assert!(running.is_alive());

// Save to state store
let managed = ManagedProcess { /* ... */ };
store.save(&managed)?;

// Ensure pm-daemon is running (no-op if already alive).
// The daemon handles health checks and auto-restarts from here on.
store.ensure_daemon_running()?;

// Forward output (or just drop the receiver for detached mode)
while let Some(msg) = running.output_rx.recv().await {
    println!("[{}] {}", msg.source, msg.content);
}
```

### 3. The Daemon Lifecycle

`ensure_daemon_running()` is the only call needed to activate monitoring:

```
first start call
  └── store.save()
  └── store.ensure_daemon_running()
        ├── checks state_dir/.daemon.lock
        ├── lock free? → spawns pm-daemon --state-dir <path>
        └── lock held? → daemon already running, returns Ok(())

pm-daemon
  ├── acquires .daemon.lock (exclusive, held until exit)
  ├── runs Monitor loop every 3 s:
  │     ├── liveness check (kill -0) → restart on crash
  │     └── health check → restart after 3 consecutive failures
  └── exits when StateStore is empty → lock released

next start call
  └── ensure_daemon_running() → lock held → no-op
```

The daemon is a **singleton per state directory** — it is impossible to spawn two daemons for the same store regardless of how many callers call `ensure_daemon_running()` concurrently.

## API Reference

### `StateStore`

| Method | Description |
|--------|-------------|
| `new(base_dir)` | Open or create a state store at `base_dir` |
| `save(&proc)` | Persist a `ManagedProcess`; touches `.status_changed` |
| `load(id)` | Load a single process by ID |
| `list()` | List all persisted processes |
| `delete(id)` | Remove a process; touches `.status_changed` |
| `is_running(&proc)` | Signal-0 liveness probe (checks PID and PGID) |
| `cleanup_dead()` | Remove all dead processes; returns deleted IDs |
| `find_by_metadata(key, value)` | Find all processes matching a metadata field |
| `find_one_by_metadata(key, value)` | Find the first matching process |
| `ensure_daemon_running()` | Start `pm-daemon` if not already alive |

### `RunningProcess`

| Method / Field | Description |
|----------------|-------------|
| `is_alive()` | Returns `true` if the spawned process is still running |
| `output_rx` | Async receiver for stdout/stderr lines |
| `pid` | Raw PID (use `is_alive()` for liveness; avoid raw PID probes) |
| `pgid` | Process Group ID for group-kill support |

### `engine` functions

| Function | Description |
|----------|-------------|
| `spawn(task)` | Spawn a process; returns `RunningProcess` |
| `terminate(pid, pgid, force)` | SIGTERM / SIGKILL with process-group support |
| `restart(task, old_pid, old_pgid, backoff)` | Terminate old, wait, spawn new |

### Health Check types

```rust
HealthCheck::Process {}                               // always passes
HealthCheck::Http { url, timeout_secs, expected_status }
HealthCheck::Tcp  { host, port, timeout_secs }
HealthCheck::Command { command, timeout_secs, expected_exit_code }
```

### Restart Policy

```rust
RestartPolicy {
    enabled: true,
    max_restarts: 3,            // within the window
    restart_window_secs: 300,   // 5-minute window
    initial_backoff_secs: 1,
    max_backoff_secs: 60,
    backoff_multiplier: 2.0,    // exponential: 1s, 2s, 4s, …, 60s
    restart_on_exit_codes: None, // None = restart on any non-zero exit
}
```

## Process Metadata

The metadata system lets callers store arbitrary key-value pairs alongside a process without coupling the library to any specific domain.

### Recommended fields (devcli convention)

| Field | Example |
|-------|---------|
| `project` | `"my-webapp"` |
| `app` | `"api-server"` |
| `env` | `"local"`, `"docker"` |
| `stage` | `"dev"`, `"prod"` |
| `command_variant` | `"start"`, `"build"` |
| `alternative_name` | `"Backend"` |

### Querying by metadata

```rust
let store = StateStore::new(state_dir)?;

let dev_processes   = store.find_by_metadata("env", "local")?;
let project_procs   = store.find_by_metadata("project", "my-webapp")?;
let single          = store.find_one_by_metadata("app", "api-server")?;
```

## License

Licensed under PolyForm Noncommercial 1.0.0.
