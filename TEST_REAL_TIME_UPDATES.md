# Testing Real-Time Monitor-TUI Communication

## Quick Test Guide

### Test 1: Start App from CLI, Watch TUI Update

1. **Terminal 1**: Start the TUI

   ```bash
   cargo run --release -- ui
   ```

2. **Terminal 2**: Start an app

   ```bash
   cargo run --release -- start <app-name>
   ```

3. **Expected Result**:
   - TUI shows status change from "Stopped" to "Running" within 250ms
   - Previously took up to 2 seconds

### Test 2: Stop App from CLI, Watch TUI Update

1. **Terminal 1**: TUI is running (from Test 1)

2. **Terminal 2**: Stop the app

   ```bash
   cargo run --release -- stop <app-name>
   ```

3. **Expected Result**:
   - TUI shows status change from "Running" to "Stopped" within 250ms
   - Previously took up to 2 seconds

### Test 3: Start/Stop from TUI

1. **Terminal 1**: TUI is running

   - Navigate to an app
   - Press `s` to start
   - Observe instant status update

2. **Stop the app**:

   - Press `x` to stop
   - Observe instant status update

3. **Expected Result**:
   - Status updates appear within 250ms
   - Much more responsive than before

### Test 4: Monitor Detects Crashed Process

1. **Terminal 1**: Start the TUI

   ```bash
   cargo run --release -- ui
   ```

2. **Terminal 2**: Start an app

   ```bash
   cargo run --release -- start <app-name>
   ```

3. **Terminal 3**: Kill the process directly

   ```bash
   # Find the PID
   ps aux | grep <app-name>
   # Kill it
   kill -9 <PID>
   ```

4. **Expected Result**:
   - Monitor detects dead process within 3 seconds
   - TUI updates within 250ms after monitor detection
   - Total time: ~3.25 seconds (vs 3-5 seconds before)

### Test 5: Verify Notification File

1. **Check the notification file exists**:

   ```bash
   ls -la ~/.devcli/processes/.status_changed
   ```

2. **Watch it change in real-time**:

   ```bash
   # Terminal 1
   watch -n 0.1 'stat ~/.devcli/processes/.status_changed'

   # Terminal 2
   cargo run --release -- start <app-name>
   ```

3. **Expected Result**:
   - File modification time updates when you start/stop apps
   - File is touched (mtime changes) on every status change

## Performance Measurement

### Measure Update Latency

```bash
# Terminal 1: Start TUI with timestamp logging
cargo run --release -- ui

# Terminal 2: Time the status update
time cargo run --release -- start <app-name>

# Observe in TUI how quickly status changes
# Should be < 250ms after command completes
```

### Compare Before/After

**Before this feature**:

```
Start command completes → Wait 0-2000ms → TUI updates
Average latency: ~1000ms
```

**After this feature**:

```
Start command completes → Wait 0-250ms → TUI updates
Average latency: ~125ms
```

**Improvement: 8x faster!**

## Debugging

### Check if Monitor is Running

```bash
ps aux | grep "devcli monitor"
```

If not running, start it:

```bash
cargo run --release -- monitor --daemon &
```

### Check Notification File

```bash
# See when it was last modified
stat ~/.devcli/processes/.status_changed

# Watch for changes
watch -n 0.1 'stat ~/.devcli/processes/.status_changed | grep Modify'
```

### Check PID Files

```bash
# List all tracked processes
ls -la ~/.devcli/processes/

# View a specific process
cat ~/.devcli/processes/<app-name>.json
```

### Enable Debug Logging

The TUI already has debug logging enabled. Check:

```bash
tail -f tui-debug.log
```

## Expected Behavior

### Normal Operation

1. **App starts**:

   - PID file created
   - Notification file touched
   - TUI detects within 250ms
   - Status shows "Running"

2. **App stops**:

   - PID file removed
   - Notification file touched
   - TUI detects within 250ms
   - Status shows "Stopped"

3. **App crashes**:
   - Monitor detects within 3s
   - PID file removed
   - Notification file touched
   - TUI detects within 250ms
   - Status shows "Stopped"

### Edge Cases

1. **Notification file missing**:

   - TUI creates it on first check
   - System continues normally

2. **Monitor not running**:

   - TUI still polls every 2s
   - Manual start/stop still work
   - Crashed processes not detected until next TUI poll

3. **Multiple TUI instances**:
   - All instances see the same notifications
   - All update within 250ms
   - No conflicts or race conditions

## Success Criteria

✅ Status updates appear within 250ms of actual change  
✅ No errors or warnings in logs  
✅ All 263 tests pass  
✅ CPU usage remains low  
✅ UI remains responsive  
✅ Works with multiple TUI instances  
✅ Gracefully handles missing notification file  
✅ Falls back to polling if notification fails

## Troubleshooting

### TUI not updating quickly

1. Check if notification file exists:

   ```bash
   ls ~/.devcli/processes/.status_changed
   ```

2. Check if file is being touched:

   ```bash
   watch -n 0.1 'stat ~/.devcli/processes/.status_changed'
   ```

3. Check TUI debug log:
   ```bash
   tail -f tui-debug.log
   ```

### Monitor not detecting crashes

1. Verify monitor is running:

   ```bash
   ps aux | grep "devcli monitor"
   ```

2. Check monitor PID file:

   ```bash
   cat ~/.devcli/processes/.monitor.json
   ```

3. Restart monitor:
   ```bash
   cargo run --release -- monitor --daemon &
   ```

## Conclusion

The real-time communication system provides instant feedback to users, making the TUI feel much more responsive and professional. The 8x improvement in update latency is immediately noticeable and significantly enhances the user experience.
