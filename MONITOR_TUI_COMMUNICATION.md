# Monitor-TUI Real-Time Communication

## Overview

The monitor process and TUI now communicate in real-time to provide instant status updates when processes start or stop. This eliminates the previous 2-second delay and provides a much more responsive user experience.

## Architecture

### File-Based Notification System

The system uses a lightweight file-based notification mechanism:

1. **Notification File**: `~/.devcli/pids/.status_changed`

   - A simple marker file that gets "touched" whenever process status changes
   - The file's modification timestamp is used as the signal

2. **Monitor Process** (Writer):

   - Detects when processes die during its 3-second polling cycle
   - Calls `notify_status_change()` after cleaning up dead processes
   - Also notifies when new processes are registered or removed

3. **TUI Process** (Reader):
   - Checks the notification file every 250ms for changes
   - Compares the file's modification time to detect updates
   - Updates all app statuses immediately when a change is detected
   - Falls back to full status check every 2 seconds as a safety net

## Implementation Details

### ProcessTracker Changes

Added three new methods to `devcli-core/src/process/tracker.rs`:

```rust
// Get path to the notification file
fn status_notification_path(&self) -> PathBuf

// Notify watchers that status has changed (touch the file)
pub fn notify_status_change(&self) -> Result<()>

// Get the last modification time of the notification file
pub fn get_last_status_change(&self) -> Result<Option<SystemTime>>
```

These methods are automatically called by:

- `register_process()` - when a new process starts
- `remove_process()` - when a process is manually stopped
- `cleanup_dead()` - when the monitor detects dead processes

### TUI Changes

Modified `devcli-core/src/tui/app.rs`:

1. **Hybrid Polling Strategy**:

   ```rust
   // Fast check every 250ms - looks for notification file changes
   fast_check_interval.tick() => check notification file

   // Full check every 2 seconds - safety net
   full_check_interval.tick() => update all statuses
   ```

2. **Status Update Flag**:

   - Added `status_updated` flag to `AppState`
   - Background polling sets this flag when status changes
   - Main event loop checks this flag and triggers redraw

3. **Non-Blocking Updates**:
   - Uses `try_lock()` to avoid blocking the main thread
   - Skips update cycle if state is locked
   - Ensures smooth UI responsiveness

## Performance Characteristics

### Before (Old System)

- Status updates: Every 2 seconds
- Worst-case delay: 2 seconds to see a status change
- CPU usage: Moderate (constant polling)

### After (New System)

- Status updates: Within 250ms of actual change
- Worst-case delay: 250ms to see a status change
- CPU usage: Similar (file stat is very cheap)
- Responsiveness: 8x improvement in update latency

## Benefits

1. **Instant Feedback**: Users see status changes almost immediately
2. **Better UX**: No more waiting 2 seconds to see if a command worked
3. **Efficient**: File modification checks are extremely lightweight
4. **Reliable**: Falls back to periodic polling if notification system fails
5. **Cross-Process**: Works across independent processes without IPC complexity

## Testing

To test the real-time communication:

1. Start the TUI:

   ```bash
   devcli ui
   ```

2. In another terminal, start an app:

   ```bash
   devcli start <app-name>
   ```

3. Observe the TUI updates within 250ms (instead of up to 2 seconds)

4. Stop the app:

   ```bash
   devcli stop <app-name>
   ```

5. Again, observe the instant update in the TUI

## Future Enhancements

Possible improvements:

1. **File System Watching**: Use `notify` crate for true event-driven updates (0ms latency)
2. **Unix Sockets**: For even more efficient IPC on Unix systems
3. **Batch Notifications**: Group multiple status changes to reduce file I/O
4. **Status Details**: Include more information in notifications (which app changed, etc.)

## Dependencies

- `filetime = "0.2"` - For updating file modification times
