# Monitor-TUI Real-Time Communication

## Overview

The `pm-daemon` monitor and TUI communicate via a lightweight file-based notification mechanism. When process state changes, the TUI picks it up within ~250ms instead of waiting for a full 2-second poll cycle.

## Architecture

### File-Based Notification System

1. **Notification file**: `~/.devcli/processes/.status_changed`
   - Touched (mtime updated) whenever `process-manager` `StateStore` saves or deletes a process
   - Also updated when `pm-daemon` updates runtime metadata (health failures, restarts)

2. **`pm-daemon`** (writer):
   - Runs the `process-manager` monitor loop (3-second cycle)
   - Detects dead or unhealthy processes and triggers restarts
   - Persists state via `StateStore::save()` / `delete()`, which touches `.status_changed`

3. **TUI** (reader):
   - Background task checks `.status_changed` mtime every **250ms**
   - Refreshes app statuses when mtime changes
   - Falls back to a full status scan every **2 seconds** as a safety net

## Implementation

### process-manager (`StateStore`)

```rust
pub fn status_changed_path(&self) -> PathBuf;
pub fn last_status_change(&self) -> Result<Option<SystemTime>>;
```

Called automatically on every `save()` and `delete()`.

### TUI (`devcli-core/src/tui/app.rs`)

Hybrid polling strategy:

```rust
// Fast: check .status_changed mtime every 250ms
// Fallback: full status refresh every 2 seconds
```

Uses `try_lock()` on app state so polling never blocks the render loop.

## Performance

| Scenario | Before | After |
|----------|--------|-------|
| Start/stop via CLI or TUI | up to 2s | ~250ms |
| Process crash (monitor detects) | ~3s + up to 2s | ~3s + ~250ms |

## Requirements

- `pm-daemon` must be installed next to `devcli` or on `PATH` for monitoring
- State directory defaults to `~/.devcli/processes/` (see `process_manager_support::state_dir`)
