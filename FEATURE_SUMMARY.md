# Real-Time Status Updates — Feature Summary

## Components

### 1. process-manager `StateStore`

- Persists managed processes under `~/.devcli/processes/`
- Touches `~/.devcli/processes/.status_changed` on every save/delete
- Exposes `last_status_change()` for cheap mtime polling

### 2. pm-daemon

- Background singleton spawned automatically when the first managed process starts
- Runs health checks and auto-restart via `process-manager::Monitor`
- Updates state files (and `.status_changed`) when processes crash or fail health checks

### 3. TUI hybrid polling (`devcli-core/src/tui/app.rs`)

- Fast check every **250ms** on `.status_changed` mtime
- Full status refresh every **2 seconds** as fallback
- Sets `status_updated` flag to trigger redraw when any app status changes

## Data Flow

```
Process event → StateStore.save/delete → touch .status_changed → TUI detects (250ms) → update UI
```

## Expected Latency

| Event | Typical delay |
|-------|----------------|
| Start/stop via CLI | ~250ms in TUI |
| Start/stop via TUI | ~250ms |
| Crash detected by monitor | monitor interval (~3s) + ~250ms |
