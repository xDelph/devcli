# devcli

A powerful command-line interface for managing spawned processes with config-based management, dependency resolution, and advanced process tracking.

## Overview

devcli is a modular process management tool that allows you to:
- Manage processes through a centralized configuration file
- Define and resolve process dependencies automatically
- Run processes in local or Docker environments
- Track running processes with persistent state
- Access detailed logs for all spawned processes
- Monitor process status and uptime grouped by project

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
cd devcli
cargo build --release
```

The binary will be available at `target/release/devcli`.

Optionally, install it globally:

```bash
cargo install --path devcli
```

## Getting Started

### 1. Initialize Configuration

Create a configuration file in `~/.devcli/config.json`:

```bash
devcli config init
```

This creates a template configuration that you can edit.

### 2. Edit Configuration

Edit your config file:

```bash
devcli config edit
```

Example configuration structure:

```json
{
  "projects": {
    "my-project": {
      "apps": {
        "api": {
          "type": "nodejs",
          "path": "~/Projects/my-project/api",
          "commands": {
            "local": {
              "start": "npm start",
              "test": "npm test",
              "build": "npm run build"
            },
            "docker": {
              "build": "docker build -t my-api .",
              "run": "docker run --name my-api -p 3000:3000 --rm my-api"
            }
          },
          "dependencies": [],
          "defaults": {
            "local": "start",
            "docker": "run"
          }
        }
      }
    }
  }
}
```

### 3. Validate Configuration

Check your configuration is valid:

```bash
devcli config validate
```

### 4. Set Preferences

Set your default environment (local or docker):

```bash
devcli pref set default-env local
```

## Usage

### Starting Processes

Start a process using its default command:

```bash
devcli start api
```

Start with a specific environment:

```bash
devcli start api --env docker
```

If app names are ambiguous across projects, specify the project:

```bash
devcli start api --project my-project
```

### Running Specific Commands

Run a specific command variant:

```bash
devcli run api build
devcli run api test
```

With environment override:

```bash
devcli run api build --env docker
```

### Checking Status

View all running processes grouped by project:

```bash
devcli status
```

Filter by project:

```bash
devcli status --project my-project
```

Show a specific app with its dependencies:

```bash
devcli status api --deps
```

Example output:

```
PROJECT: my-project
  [api] (local)  PID: 12345  running  2h 15m  npm start
  [worker] (local)  PID: 12346  running  1h 30m  npm run worker

PROJECT: other-project
  [frontend] (local)  PID: 12347  running  45m  npm run dev
```

### Configuration Management

List all projects and apps:

```bash
devcli config list
```

List apps only:

```bash
devcli config list --apps-only
```

Show details of a specific app:

```bash
devcli config show api
```

### Preferences Management

Show current preferences:

```bash
devcli pref show
```

Set default environment:

```bash
devcli pref set default-env docker
```

Reset preferences to defaults:

```bash
devcli pref reset
```

## Features

### Config-Based Management

All processes are defined in `~/.devcli/config.json`:
- Centralized configuration for all projects and apps
- Support for multiple environments (local, docker)
- Default commands per environment
- Path expansion for home directory (~) and environment variables

### Dependency Resolution

Define dependencies between apps:

```json
{
  "dependencies": [
    {"project": "infrastructure", "app": "redis"},
    {"project": "infrastructure", "app": "traefik"}
  ]
}
```

devcli will:
- Check dependencies are running before starting an app
- Detect circular dependencies
- Provide clear error messages for missing dependencies

### Multi-Environment Support

Define separate commands for different environments:
- `local`: Commands for local development
- `docker`: Commands for Docker containers

Set your preferred environment once, or override per command.

### Project Grouping

Status command groups processes by project for better organization. Processes without config metadata are shown in "UNGROUPED" section.

### Log Management

All process logs are automatically saved to:
```
~/.devcli/logs/<app-name>_<timestamp>.log
```

Logs include timestamps and are preserved for review.

### Process Tracking

Process information is stored in:
```
~/.devcli/pids/<app-name>.json
```

This includes:
- Process ID (PID)
- Command and working directory
- Environment variables
- Start time
- Project and app metadata
- Environment type (local/docker)
- Command variant used

## Architecture

devcli uses a modular workspace structure:

### Workspace Structure

```
devcli/
├── devcli/           # Binary crate (CLI entry point)
│   └── src/main.rs     # Command routing and argument parsing
└── devcli-core/      # Library crate (core functionality)
    └── src/
        ├── config/     # Configuration management
        ├── process/    # Process spawning and tracking
        ├── logging/    # Log file management
        ├── commands/   # Command implementations
        └── utils/      # Utility functions (path expansion)
```

### Core Modules

- **Config Manager**: Loads and validates configuration files
- **Dependency Resolver**: Resolves and checks app dependencies
- **Process Spawner**: Handles process creation with configurable options
- **Process Tracker**: Manages PID tracking and process state persistence
- **File Logger**: Manages log files with timestamps
- **Commands**: Implements start, run, status, config, and pref commands

## Future Roadmap

Upcoming features include:
- **Process Control**: `stop`, `restart`, and `kill` commands
- **Enhanced Monitoring**: CPU/memory usage tracking
- **Log Visualization**: Interactive log viewer and search
- **Terminal Dashboard**: Real-time process monitoring UI
- **Multi-app Management**: Start/stop multiple apps as groups with one command

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for development guidelines and architecture details.

## License

MIT
