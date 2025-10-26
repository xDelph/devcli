# Development Guidelines

This document provides guidelines for contributing to and understanding the RustyCLI codebase.

## Code Quality Standards

### Senior-Level Code with Junior-Friendly Documentation

The codebase is designed to be:
- **Production-ready**: Well-tested, properly error-handled, performant
- **Educational**: Extensively documented to help developers learn Rust

### Core Principles

1. **Error Handling**
   - Always use `Result<T, E>` types for fallible operations
   - Never use `unwrap()` or `expect()` in production code
   - Provide context with `.context()` from the `anyhow` crate
   - Handle errors gracefully with user-friendly messages

2. **Comments and Documentation**
   - Explain **why** decisions were made, not **what** the code does
   - Document public APIs with `///` doc comments
   - Add inline comments for complex logic or Rust-specific patterns
   - Use examples in documentation where helpful

3. **Module Organization**
   - Keep modules focused and cohesive
   - Use `mod.rs` files to expose public APIs
   - Separate concerns (spawning vs tracking, for example)
   - Make internal implementation details private

4. **Dependencies**
   - Use well-vetted, maintained crates only
   - Document why each dependency is needed
   - Prefer standard library when possible
   - Keep the dependency tree minimal

## Architecture

### Workspace Structure

RustyCLI uses a Cargo workspace with two crates:

#### `rustycli` (Binary Crate)
- Entry point for the CLI application
- Handles argument parsing with `clap`
- Routes commands to appropriate handlers
- Manages user-facing error messages

#### `rustycli-core` (Library Crate)
- Contains all business logic
- Reusable across different interfaces
- Well-tested and independent
- Exposes clean public API

### Module Breakdown

#### `process/` Module
Handles process lifecycle management:

**`spawner.rs`**
- Creates child processes with `tokio::process::Command`
- Handles both attached and detached modes
- Streams stdout/stderr with labels
- Works with the logging system

**`tracker.rs`**
- Persists process information to disk
- Tracks PIDs and metadata
- Checks if processes are still running
- Cleans up stale process files

#### `logging/` Module
Manages log files:

**`file_logger.rs`**
- Creates log files in `~/.rustycli/logs/`
- Writes timestamped log entries
- Provides async API for concurrent writes
- Handles file I/O errors gracefully

#### `commands/` Module
Implements CLI commands:

**`start.rs`**
- Orchestrates process spawning
- Validates arguments
- Creates log files
- Registers processes with tracker
- Handles both attached and detached modes

**`status.rs`**
- Lists tracked processes
- Displays process information in table format
- Automatically cleans up dead processes
- Calculates uptime from start time

## Testing Strategy

### Unit Tests
- Test core logic in isolation
- Mock external dependencies where needed
- Cover edge cases and error conditions
- Place tests in `#[cfg(test)]` modules

### Integration Tests
- Test command execution end-to-end
- Verify process spawning and tracking
- Check log file creation
- Validate cleanup operations

### Running Tests

```bash
cargo test
```

Run tests for a specific crate:

```bash
cargo test -p rustycli-core
```

## Building and Running

### Debug Build
Fast compilation, includes debug symbols:

```bash
cargo build
./target/debug/rustycli --help
```

### Release Build
Optimized for performance:

```bash
cargo build --release
./target/release/rustycli --help
```

### Running Directly
During development:

```bash
cargo run -- start test-app --cmd "echo hello"
cargo run -- status
```

## Code Style

### Formatting
Use `rustfmt` for consistent formatting:

```bash
cargo fmt
```

### Linting
Use `clippy` for Rust best practices:

```bash
cargo clippy -- -D warnings
```

Fix common issues automatically:

```bash
cargo clippy --fix
```

## Common Patterns

### Error Propagation

Use the `?` operator with `Result` types:

```rust
pub fn do_something() -> Result<()> {
    let value = might_fail()?;
    process(value)?;
    Ok(())
}
```

### Adding Context to Errors

```rust
use anyhow::Context;

fs::read_to_string(path)
    .context("Failed to read configuration file")?;
```

### Async Functions

Use `async/await` for I/O operations:

```rust
pub async fn write_log(&mut self, message: &str) -> Result<()> {
    self.file.write_all(message.as_bytes()).await?;
    self.file.flush().await?;
    Ok(())
}
```

### Builder Pattern for Options

```rust
let options = ProcessOptions {
    app_name: "my-app".to_string(),
    working_dir: PathBuf::from("./"),
    command: "npm start".to_string(),
    env_vars: HashMap::new(),
    detached: false,
};
```

## Contributing

### Before Submitting

1. Run tests: `cargo test`
2. Format code: `cargo fmt`
3. Check for warnings: `cargo clippy`
4. Update documentation if needed
5. Add tests for new features

### Pull Request Guidelines

- Keep changes focused and atomic
- Write clear commit messages
- Update README if user-facing changes
- Ensure all tests pass
- Add comments for complex logic

## Resources for Learning Rust

- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Clap Documentation](https://docs.rs/clap/latest/clap/)

## Troubleshooting

### Common Issues

**Build Errors**
- Ensure Rust is up to date: `rustup update`
- Clean build artifacts: `cargo clean && cargo build`

**Permission Errors**
- Check that `~/.rustycli/` directories are writable
- Run with appropriate permissions on macOS/Linux

**Process Not Tracked**
- Verify PID file exists: `ls ~/.rustycli/pids/`
- Check for JSON parsing errors in logs

## Future Development

Planned enhancements:
1. Configuration file support (YAML/TOML)
2. Process control commands (stop, restart)
3. Enhanced monitoring with resource usage
4. Interactive dashboard with TUI
5. Process groups and bulk operations
6. Log streaming and search functionality

