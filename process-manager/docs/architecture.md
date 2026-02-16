# Process Manager Architecture

This document describes the core architectural decisions that make `process-manager` robust and reliable.

## 1. Process Group Management (PGID)

One of the biggest challenges in process management is "orphan" processes. Many commands (like shell scripts or package managers) spawn sub-processes and then exit. If a supervisor only tracks the initial PID, it loses control over the actual application.

### How it works:
- When a task is spawned as **detached**, the engine uses `setsid()` to create a new session.
- The process becomes the **Leader** of a new **Process Group**.
- The **PGID** is identical to the initial **PID**.
- Even if the parent PID exits, the children remain in the same group.
- **Liveness Check**: `StateStore::is_running` checks the PID first, then falls back to checking the entire PGID group using `kill(0, -pgid)`.
- **Termination**: When `terminate` is called, the signal is sent to the negative PGID (`-pgid`), which Unix interprets as "send to every process in this group."

## 2. Cross-Process Synchronization

`process-manager` is built for environments where a CLI tool and a background Daemon coexist.

### The Challenge:
If the background Monitor detects a health failure at the exact same millisecond that a user runs a manual `restart` command via the CLI, both might try to spawn the process, leading to port conflicts and duplicate tasks.

### The Solution:
The `RestartCoordinator` implements **File-Based Locking** using the `fs2` crate.
- Every task has a corresponding `.lock` file in the coordination directory.
- Before any restart (manual or automatic), the process must acquire an **exclusive advisory lock** on this file.
- If the lock is held by another process, the second caller will immediately receive a `None` or `WouldBlock` result and skip the operation.

## 3. Persistent State Machine

The `StateStore` acts as the source of truth. Every process is serialized into a JSON file named `{id}.json`.

### State Lifecycle:
1. **Registered**: The file is created when `spawn` is successful.
2. **Active**: The monitor periodically updates the `runtime` fields (last health check, failure count).
3. **Crashed**: The PID is gone, but the file remains. The monitor will detect this and trigger the restart logic.
4. **Clean Exit**: If a process exits with code 0 and no restart policy is set, or if it's explicitly stopped, the state file is deleted.

## 4. Structured Output Streaming

Unlike standard `std::process::Child` which can only have its output consumed once, `process-manager` implements a unified handler.

- **Background Logger**: A dedicated task always reads `stdout`/`stderr` and appends to the configured `log_file`.
- **Event Bus**: Simultaneously, every line is wrapped in an `OutputMessage` (with source and timestamp) and sent to an asynchronous channel.
- **Subscription**: The `RunningProcess` struct returns the `Receiver` end of this channel. If the caller doesn't need it, they can simply drop it; the background logging continues regardless.
