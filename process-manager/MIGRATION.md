# Migration Guide: ProcessTracker → process-manager

This guide explains how to migrate from the old `devcli-core` ProcessTracker to the new standalone `process-manager` crate.

## Overview

The `process-manager` crate is now a standalone, reusable library extracted from `devcli-core`. It provides the same functionality but with a more generic API suitable for any process management tool.

## Breaking Changes

### 1. Process Identification

**Before (ProcessTracker):**
```rust
tracker.get_process(project, app_name, environment)
tracker.remove_process(project, app_name, environment)
```

**After (process-manager):**
```rust
state.load(id)  // id = "{project}.{app_name}.{environment}"
state.delete(id)
```

### 2. Process Information Structure

**Before:** Flat `ProcessInfo` with specific fields
```rust
struct ProcessInfo {
    app_name: String,
    project: Option<String>,
    environment: Option<String>,
    stage: Option<String>,
    app_config_name: Option<String>,
    command_variant: Option<String>,
    // ...
}
```

**After:** Generic `ManagedProcess` with metadata HashMap
```rust
struct ManagedProcess {
    id: String,
    task: Task,
    metadata: HashMap<String, String>,  // ← project, environment, etc. go here
    runtime: ProcessRuntime,
    // ...
}
```

### 3. Command Parsing

The new engine uses `shell-words` for proper argument parsing:
```rust
// Now handles quoted arguments correctly
let task = Task {
    command: r#"echo "hello world""#,  // ✅ Correctly parsed
    // ...
};
```

## Migration Steps

### Step 1: Stop All Running Applications

Run the migration script to gracefully stop all processes managed by the old system:

```bash
# Dry run first to see what will happen
./scripts/migrate_to_process_manager.sh --dry-run

# Actually perform migration
./scripts/migrate_to_process_manager.sh
```

This will:
- Stop all running processes
- Create a backup of old PID files at `~/.devcli/pids_backup_{timestamp}/`
- Clean up the PID directory

### Step 2: Update devcli-core Dependencies

Add `process-manager` to `devcli-core/Cargo.toml`:

```toml
[dependencies]
process-manager = { path = "../process-manager" }
# ... other dependencies
```

### Step 3: Update Process Registration

**Before:**
```rust
let info = ProcessInfo {
    app_name: "my-app".to_string(),
    project: Some("my-project".to_string()),
    environment: Some("dev".to_string()),
    // ...
};
tracker.register_process(info)?;
```

**After:**
```rust
use process_manager::{ManagedProcess, Task};

let mut metadata = HashMap::new();
metadata.insert("project".to_string(), "my-project".to_string());
metadata.insert("app_config_name".to_string(), "my-app".to_string());
metadata.insert("environment".to_string(), "dev".to_string());

let process = ManagedProcess {
    id: "my-project.my-app.dev".to_string(),
    pid: running.pid,
    pgid: running.pgid,
    task: task.clone(),
    start_time: Utc::now(),
    metadata,
    runtime: ProcessRuntime::default(),
};

state.save(&process)?;
```

### Step 4: Update Process Queries

**Before:**
```rust
let process = tracker.get_process("my-project", "my-app", Some("dev"))?;
let all = tracker.list_processes()?;
```

**After:**
```rust
// By ID
let process = state.load("my-project.my-app.dev")?;

// By metadata
let dev_processes = state.find_by_metadata("environment", "dev")?;
let app = state.find_one_by_metadata("app_config_name", "my-app")?;

// All processes
let all = state.list()?;
```

### Step 5: Update Spawning Logic

**Before:**
```rust
use devcli_core::process::spawn_process;

let spawned = spawn_process(ProcessOptions {
    command: "npm run dev",
    // ...
})?;
```

**After:**
```rust
use process_manager::{Task, engine};

let task = Task {
    id: "my-project.my-app.dev".to_string(),
    command: "npm run dev".to_string(),
    args: vec![],
    working_dir: PathBuf::from("/path/to/app"),
    env: HashMap::new(),
    is_detached: true,
    log_file: Some(PathBuf::from("app.log")),
    health_check: HealthCheck::Http { /* ... */ },
    restart_policy: RestartPolicy::default(),
};

let running = engine::spawn(&task).await?;
```

## Recommended Metadata Fields

Use these standard metadata fields for compatibility:

| Field | Example | Usage |
|-------|---------|-------|
| `project` | `"my-webapp"` | Repository/project name |
| `app_config_name` | `"api-server"` | Clean app name from config |
| `environment` | `"dev"` | Environment (dev/qa/prod) |
| `stage` | `"staging"` | Deployment stage if needed |
| `command_variant` | `"start"` | Which command was used |

## Testing the Migration

After updating your code:

1. **Build the new system:**
   ```bash
   cargo build
   ```

2. **Start a test application:**
   ```bash
   devcli start my-app
   ```

3. **Verify state persistence:**
   ```bash
   # Check the state directory
   ls ~/.devcli/state/
   # Should see: my-project.my-app.dev.json
   ```

4. **Test health checks:**
   ```bash
   devcli status
   ```

## Rollback Plan

If you need to rollback:

1. Stop all processes managed by the new system
2. Restore old PID files from backup:
   ```bash
   cp -r ~/.devcli/pids_backup_*/* ~/.devcli/pids/
   ```
3. Checkout the previous version of devcli-core
4. Rebuild and start your applications

## New Features Available

The new `process-manager` crate provides:

- ✅ Proper shell command parsing with quoted arguments
- ✅ Metadata-based querying (`find_by_metadata`)
- ✅ Better separation of concerns
- ✅ Comprehensive unit tests (32 tests)
- ✅ Process group (PGID) tracking for child processes
- ✅ File-based restart coordination
- ✅ Pluggable health checks (HTTP, TCP, Command, Process)
- ✅ Configurable restart policies with exponential backoff

## Support

If you encounter issues during migration:

1. Check the backup directory for old PID files
2. Review the migration script output for errors
3. Check logs in `~/.devcli/logs/`
4. File an issue with details about the error

## Next Steps After Migration

1. Remove old `ProcessTracker` code from `devcli-core/src/process/tracker.rs`
2. Update all commands to use the new `process-manager` API
3. Run full integration tests
4. Update user documentation
