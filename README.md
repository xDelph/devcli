# RustyCLI

A powerful command-line interface for managing spawned processes with advanced logging and process tracking capabilities.

## Overview

RustyCLI is a modular process management tool that allows you to:
- Spawn and manage child processes with ease
- Run processes in attached or detached mode
- Track running processes with persistent state
- Access detailed logs for all spawned processes
- Monitor process status and uptime

## Installation

### Prerequisites

Ensure you have Rust installed on your system. On macOS with Homebrew:

```bash
brew install rustup-init
rustup-init
source ~/.cargo/env
```

Verify the installation:

```bash
rustc --version
cargo --version
```

### Building from Source

Clone this repository and build the project:

```bash
cd rusty_cli
cargo build --release
```

The binary will be available at `target/release/rustycli`.

Optionally, install it globally:

```bash
cargo install --path rustycli
```

## Usage

### Starting a Process

Start a process in attached mode (displays logs in real-time):

```bash
rustycli start my-app --cmd "npm start" --dir ./my-project
```

Start a process in detached mode (runs in background):

```bash
rustycli start my-api --cmd "node server.js" --dir ./api --detach
```

With environment variables:

```bash
rustycli start my-service \
  --cmd "python app.py" \
  --dir ./service \
  --env PORT=8080 \
  --env ENV=production \
  --detach
```

### Checking Process Status

View all running processes:

```bash
rustycli status
```

Check a specific process:

```bash
rustycli status my-app
```

Example output:

```
APP NAME             PID        STATUS       UPTIME          COMMAND
====================================================================================================
my-app               12345      running      2h 15m          npm start
my-api               12346      running      5m              node server.js
```

## Known Limitations

### Command Parsing
Currently, the CLI uses simple whitespace-based command parsing. Complex shell commands with nested quotes may not work as expected. 

**Workaround**: Create a shell script for complex commands and call it directly:
```bash
rustycli start my-app --cmd "./scripts/start.sh"
```

This will be improved in a future version with proper shell argument parsing.

## Features

### Attached Mode

When starting a process without `--detach`, the CLI will:
- Display real-time logs with labels `[app-name][stdout/stderr]`
- Wait for the process to complete
- Allow you to stop it with Ctrl+C

### Detached Mode

With the `--detach` flag:
- Process runs independently in the background
- Parent CLI exits immediately after spawning
- Process continues even if terminal is closed
- PID is tracked for later management

### Log Management

All process logs are automatically saved to:
```
~/.rustycli/logs/<app-name>_<timestamp>.log
```

Logs include timestamps and are preserved for review.

### Process Tracking

Process information is stored in:
```
~/.rustycli/pids/<app-name>.json
```

This includes:
- Process ID (PID)
- Command and working directory
- Environment variables
- Start time

## Architecture

RustyCLI uses a modular workspace structure:

### Workspace Structure

```
rusty_cli/
├── rustycli/           # Binary crate (CLI entry point)
│   └── src/main.rs     # Command routing and argument parsing
└── rustycli-core/      # Library crate (core functionality)
    └── src/
        ├── process/    # Process spawning and tracking
        ├── logging/    # Log file management
        └── commands/   # Command implementations
```

### Core Modules

- **Process Spawner**: Handles process creation with configurable options
- **Process Tracker**: Manages PID tracking and process state persistence
- **File Logger**: Manages log files with timestamps
- **Commands**: Implements `start` and `status` commands

## Future Roadmap

Upcoming features include:
- **Config Management**: Store app configurations in files
- **Process Control**: `stop`, `restart`, and `kill` commands
- **Enhanced Monitoring**: CPU/memory usage tracking
- **Log Visualization**: Interactive log viewer and search
- **Terminal Dashboard**: Real-time process monitoring UI
- **Multi-app Management**: Start/stop multiple apps as groups

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for development guidelines and architecture details.

## License

MIT

