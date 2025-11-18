# Monitor-TUI Communication Flow

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         User Actions                             │
└─────────────────────────────────────────────────────────────────┘
                                 │
                    ┌────────────┴────────────┐
                    │                         │
                    ▼                         ▼
        ┌───────────────────┐     ┌───────────────────┐
        │   CLI Commands    │     │    TUI Interface  │
        │                   │     │                   │
        │  rustycli start   │     │  Press 's' to     │
        │  rustycli stop    │     │  start/stop       │
        │  rustycli restart │     │                   │
        └─────────┬─────────┘     └─────────┬─────────┘
                  │                         │
                  │                         │
                  ▼                         ▼
        ┌─────────────────────────────────────────────┐
        │         ProcessTracker                      │
        │  ~/.rustycli/pids/                          │
        │                                             │
        │  • app1.json (PID, start time, etc.)       │
        │  • app2.json                                │
        │  • .monitor.json                            │
        │  • .status_changed (notification file)      │
        └─────────────────────────────────────────────┘
                  │                         │
                  │                         │
        ┌─────────▼─────────┐     ┌─────────▼─────────┐
        │  Monitor Process  │     │   TUI Polling     │
        │                   │     │                   │
        │  Every 3 seconds: │     │  Every 250ms:     │
        │  1. Check PIDs    │     │  1. Check file    │
        │  2. Clean dead    │     │     mtime         │
        │  3. Notify if     │     │  2. Update status │
        │     changed       │     │     if changed    │
        │                   │     │                   │
        │  Writes to:       │     │  Reads from:      │
        │  .status_changed  │────▶│  .status_changed  │
        └───────────────────┘     └───────────────────┘
```

## Sequence Diagram: Process Stops

```
User          CLI/TUI       ProcessTracker    Monitor       TUI Polling
 │              │                │              │               │
 │─stop app────▶│                │              │               │
 │              │─remove_process─▶│              │               │
 │              │                │─delete PID   │               │
 │              │                │─touch .status_changed        │
 │              │                │              │               │
 │              │                │              │◀──check file──│
 │              │                │              │   (250ms)     │
 │              │                │              │               │
 │              │                │              │──file changed!│
 │              │                │              │               │
 │              │                │              │──update status│
 │              │                │              │               │
 │              │                │              │──set flag─────▶
 │              │                │              │               │
 │              │◀───────────────────────────────redraw UI──────│
 │◀─UI updated──│                │              │               │
```

## Sequence Diagram: Process Crashes (Monitor Detects)

```
Process       Monitor         ProcessTracker    TUI Polling
 │              │                  │               │
 │──crashes     │                  │               │
 │              │                  │               │
 │              │──check PIDs      │               │
 │              │  (every 3s)      │               │
 │              │                  │               │
 │              │──cleanup_dead───▶│               │
 │              │                  │─check PID     │
 │              │                  │─PID dead!     │
 │              │                  │─delete file   │
 │              │                  │─touch .status_changed
 │              │                  │               │
 │              │                  │               │◀─check file
 │              │                  │               │  (250ms)
 │              │                  │               │
 │              │                  │               │─file changed!
 │              │                  │               │
 │              │                  │               │─update status
 │              │                  │               │
 │              │                  │               │─redraw UI
```

## Key Timing Characteristics

### Status Update Latency

| Scenario            | Old System  | New System  | Improvement     |
| ------------------- | ----------- | ----------- | --------------- |
| Manual stop via CLI | 0-2000ms    | 0-250ms     | 8x faster       |
| Manual stop via TUI | 0-2000ms    | 0-250ms     | 8x faster       |
| Process crash       | 3000-5000ms | 3000-3250ms | Slightly faster |
| Manual start        | 0-2000ms    | 0-250ms     | 8x faster       |

### Explanation

- **Manual operations**: Instant notification → 250ms detection = ~250ms total
- **Process crashes**: Monitor detects in 3s → 250ms detection = ~3.25s total
- **Old system**: Always waited for next 2s polling cycle

## File System Operations

### Notification File Operations

```rust
// When status changes (any process start/stop/crash)
notify_status_change() {
    touch ~/.rustycli/pids/.status_changed
    // Updates file modification time
}

// TUI checks every 250ms
get_last_status_change() {
    stat ~/.rustycli/pids/.status_changed
    return mtime
}
```

### Performance Impact

- `stat()` system call: ~1-10 microseconds
- File touch: ~10-100 microseconds
- Negligible CPU and I/O impact
- No network or IPC overhead

## Error Handling

The system is designed to be resilient:

1. **Notification file missing**: TUI creates it on first check
2. **Notification fails**: Falls back to 2-second polling
3. **State lock contention**: Skips update cycle, tries again next time
4. **Monitor not running**: TUI still polls normally
5. **File system errors**: Logged but don't crash the app

## Comparison with Alternatives

### Why Not Use...?

1. **Unix Sockets/Named Pipes**:

   - More complex setup
   - Requires connection management
   - Not as portable
   - Overkill for simple notifications

2. **File System Watching (inotify/FSEvents)**:

   - Requires additional dependencies
   - Platform-specific code
   - More complex error handling
   - Current solution is "good enough"

3. **Shared Memory**:

   - Complex synchronization
   - Platform-specific
   - Harder to debug
   - Not needed for this use case

4. **Database/Redis**:
   - External dependency
   - Overkill for local process management
   - Adds complexity and failure modes

The file-based approach is simple, portable, and efficient for this use case.
