# devcli
[![CI](https://github.com/xdelph/devcli-private/workflows/CI/badge.svg)](https://github.com/xdelph/devcli-private/actions)
[![Release](https://github.com/xdelph/devcli-private/workflows/Release/badge.svg)](https://github.com/xdelph/devcli-private/actions)
[![License: PolyForm](https://img.shields.io/badge/License-PolyForm%20Noncommercial-blue.svg)](LICENSE)
[![Commercial License](https://img.shields.io/badge/Commercial-License%20Available-green.svg)](LICENSE-COMMERCIAL.md)

A powerful command-line interface for managing spawned processes with config-based management, dependency resolution, and advanced process tracking.

## Overview

devcli is a powerful process management tool that allows you to:

- **Manage processes** through a centralized YAML configuration
- **Auto-start dependencies** in the correct order
- **Run in multiple environments** (local, Docker, Kubernetes)
- **Monitor health** and automatically restart crashed processes
- **View logs** in a beautiful terminal UI
- **Track metrics** for all your running processes
- **Scale from single app to complex microservices**

Perfect for microservices development, monorepo projects, and complex local development setups.

## Features

- ✅ **Config-based management** - Define once, run anywhere
- ✅ **Smart dependency resolution** - Automatic topological sorting
- ✅ **Multi-environment support** - Local, Docker, K8s, OrbStack
- ✅ **Health monitoring** - HTTP, TCP, and command-based checks
- ✅ **Auto-restart** - Configurable restart policies with exponential backoff
- ✅ **Real-time metrics** - HTTP API + CLI for insights
- ✅ **Structured logging** - JSON logs with spans and context
- ✅ **Beautiful TUI** - Interactive log viewer with syntax highlighting
- ✅ **Auto-detection** - Discover and configure apps automatically
- ✅ **Environment management** - Stage-based env file support

## Installation

### Quick Install (Recommended)

**macOS and Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/develop/install.sh | sh
```

This script will:
- Detect your platform automatically
- Download the latest version
- Install to `~/.devcli/bin`
- Add to your PATH
- No sudo required

### Homebrew (macOS/Linux)

```bash
brew tap xDelph/devcli
brew install devcli
```

**Update:**
```bash
brew upgrade devcli
```

### Manual Installation

Download the binary for your platform from the [Releases page](https://github.com/xDelph/devcli/releases).

**macOS (Apple Silicon M1/M2/M3):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-aarch64-apple-darwin.tar.gz | tar xz
chmod +x devcli
mv devcli ~/.devcli/bin/  # or /usr/local/bin with sudo
```

**macOS (Intel):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-x86_64-apple-darwin.tar.gz | tar xz
chmod +x devcli
mv devcli ~/.devcli/bin/
```

**Linux (x64):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-x86_64-unknown-linux-gnu.tar.gz | tar xz
chmod +x devcli
mv devcli ~/.devcli/bin/
```

**Linux (ARM64):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-aarch64-unknown-linux-gnu.tar.gz | tar xz
chmod +x devcli
mv devcli ~/.devcli/bin/
```

### Verify Installation

```bash
devcli --version
devcli --help
```

### Building from Source

If you prefer to build from source:

**Prerequisites:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

**Build:**
```bash
# Note: Source code is in private repository
# Contact thomas.delalonde@example.com for source access
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

## Documentation

Comprehensive documentation is available in the `docs/` directory:

### 📚 Documentation

- **[Getting Started](./docs/getting-started.md)** - Quick start guide with examples (5-minute setup)
- **[Configuration Reference](./docs/configuration-reference.md)** - Complete config file documentation
- **[Commands Reference](./docs/commands-reference.md)** - All CLI commands with examples
- **[Advanced Features](./docs/advanced-features.md)** - Health checks, metrics, logging, dependencies
- **[Troubleshooting](./docs/troubleshooting.md)** - Common issues and solutions

### Quick Links

- **Installation**: See [Getting Started](./docs/getting-started.md#installation)
- **Config Format**: See [Configuration Reference](./docs/configuration-reference.md#file-structure)
- **All Commands**: See [Commands Reference](./docs/commands-reference.md)
- **Health Checks**: See [Advanced Features](./docs/advanced-features.md#health-checks)
- **Metrics**: See [Advanced Features](./docs/advanced-features.md#metrics--monitoring)
- **Help**: See [Troubleshooting](./docs/troubleshooting.md)

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
