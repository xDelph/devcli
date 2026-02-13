# DEVCLI

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

![DevCLI Terminal Demo](./assets/hero-terminal-demo.gif)

## Showcase

### Interactive TUI Dashboard
Monitor all your processes in real-time.
![TUI Dashboard](./assets/tui-dashboard.png)

### Multi-App Log Streaming
Watch logs from multiple apps simultaneously.
![TUI Logs](./assets/tui-logs.png)

### Colorful CLI Status
Get quick insights into your process health.
![CLI Status](./assets/terminal-status.png)

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
```bash
curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/main/install.sh | sh
```

### Homebrew (macOS/Linux)
```bash
brew tap xDelph/devcli
brew install devcli
```

## License

See [LICENSE](LICENSE) and [LICENSE-COMMERCIAL](LICENSE-COMMERCIAL) for details.


<!-- BEGIN_SYNC_FROM_PRIVATE -->
<!-- Content from private repo README will be inserted here -->
<!-- END_SYNC_FROM_PRIVATE -->
