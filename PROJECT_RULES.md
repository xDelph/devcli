# devcli - Project Rules & Guidelines

## Code Quality Standards

### Senior-Level with Educational Focus
- **Production Quality**: All code follows Rust best practices and idioms
- **Learning-Oriented**: Extensive documentation explaining architectural decisions
- **Error Handling**: Proper `Result` types throughout, no `unwrap()` or `expect()` in production
- **Type Safety**: Leverage Rust's type system for compile-time guarantees

### Architectural Principles

#### 1. Modularity First
- Workspace structure separates concerns (binary vs library)
- Each module has a single, well-defined responsibility
- Public APIs are minimal and well-documented
- Implementation details remain private

#### 2. Library-Driven Design
- Core logic lives in `devcli-core` library crate
- Binary crate (`devcli`) is a thin wrapper for CLI interaction
- Enables future reuse (TUI, web interface, etc.)
- Testable components in isolation

#### 3. Async by Default
- Use `tokio` for all I/O operations
- Non-blocking process management
- Efficient log streaming
- Future-proof for network operations

#### 4. Configuration Over Convention
- Everything configurable through CLI arguments (for now)
- Future: config file support planned
- Sensible defaults for common use cases
- No magic behavior

### Code Organization

```
devcli-core/src/
├── lib.rs              # Public API exports
├── process/
│   ├── mod.rs          # Process module exports
│   ├── spawner.rs      # Process spawning logic
│   └── tracker.rs      # PID tracking and persistence
├── logging/
│   ├── mod.rs          # Logging module exports
│   └── file_logger.rs  # File-based logging
└── commands/
    ├── mod.rs          # Command module exports
    ├── start.rs        # Start command implementation
    └── status.rs       # Status command implementation
```

### Development Workflow

#### Before Committing
1. `cargo fmt` - Format code
2. `cargo clippy -- -D warnings` - Check for issues
3. `cargo test` - Run all tests
4. `cargo build --release` - Verify release build

#### Adding Features
1. Design the API in the core library first
2. Implement with proper error handling
3. Add unit tests
4. Expose through CLI if needed
5. Update documentation

### Dependencies Policy

#### Current Dependencies
- **clap**: CLI argument parsing (industry standard)
- **tokio**: Async runtime (most popular, battle-tested)
- **serde/serde_json**: Serialization (essential for config)
- **anyhow**: Error handling (ergonomic for applications)
- **chrono**: Date/time handling (serde support needed)
- **dirs**: Cross-platform directory access
- **libc**: Unix system calls for process daemonization

#### Adding New Dependencies
- Justify why it's needed
- Check maintenance status and popularity
- Prefer smaller, focused crates
- Document in this file

### Error Handling Standards

#### Use Context
```rust
fs::read_to_string(path)
    .context("Failed to read configuration file")?;
```

#### User-Friendly Messages
```rust
anyhow::bail!("Process '{}' is already running with PID {}", name, pid);
```

#### Graceful Degradation
- Clean up resources on errors
- Provide actionable error messages
- Log details for debugging

### Testing Strategy

#### Unit Tests (TODO)
- Test individual functions in isolation
- Mock file system and process operations
- Cover error cases

#### Integration Tests (TODO)
- End-to-end command testing
- Process spawning and tracking
- Log file verification

#### Manual Testing Scenarios
1. Start process in attached mode
2. Start process in detached mode
3. Multiple processes simultaneously
4. Process name conflicts
5. Invalid commands
6. Directory permissions
7. Dead process cleanup

### Process Management Rules

#### Attached Mode
- Parent process waits for child
- Real-time log streaming
- Cleanup on exit
- Ctrl+C handling

#### Detached Mode
- Child becomes session leader (setsid)
- Survives parent termination
- PID tracked for management
- Log file only (no stdout)

#### PID Tracking
- JSON format for extensibility
- Store in `~/.devcli/processes/`
- One file per managed process (`{project}.{app}.{environment}.json`)
- Auto-cleanup dead processes via `process-manager` and `pm-daemon`

#### Log Management
- All logs in `~/.devcli/logs/`
- Timestamped filenames
- One file per process start
- Timestamp each log entry

### Future Feature Guidelines

When implementing future features:

1. **Config Files**: Use TOML or YAML, store in `~/.devcli/config.toml`
2. **Stop/Restart**: Use `kill` syscall, graceful shutdown with timeout
3. **Monitoring**: Consider `sysinfo` crate for resource usage
4. **Dashboard**: Use `ratatui` for terminal UI
5. **Multi-app Groups**: Add group concept to process tracker

### Platform Support

#### Primary: Unix-like (macOS, Linux)
- Full feature support
- Process daemonization with `setsid`
- Signal handling

#### Future: Windows
- Use Windows service APIs
- Different process management approach
- Conditional compilation with `cfg` attributes

### Security Considerations

1. **No Privilege Escalation**: Run as user, spawn as user
2. **Path Safety**: Validate working directory exists
3. **Command Injection**: Already safe (not using shell)
4. **File Permissions**: Respect user umask

### Performance Guidelines

- Async I/O for all operations
- Minimal allocations in hot paths
- Clone only when necessary
- Use references where possible

### Documentation Standards

#### Code Comments
- Explain "why", not "what"
- Complex algorithms get explanations
- Public APIs use `///` doc comments

#### README
- Keep examples current
- Document all CLI flags
- Architecture overview

#### DEVELOPMENT.md
- Contribution guidelines
- Testing instructions
- Design decisions

## Project Maturity Roadmap

### ✅ MVP Complete (Current)
- Process spawning (attached/detached)
- PID tracking and persistence
- Log file management
- Status reporting
- Error handling

### 🔄 Next Phase
- Configuration file support
- Stop/restart commands
- Enhanced monitoring

### 🔮 Future
- Terminal dashboard
- Process groups
- Remote management
- Plugin system

