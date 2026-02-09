# Getting Started with devcli

A comprehensive guide to get you up and running with devcli in minutes.

## What is devcli?

devcli is a powerful process management tool that helps you:

- **Manage processes** through a centralized YAML configuration
- **Auto-start dependencies** in the correct order
- **Run in multiple environments** (local, Docker, Kubernetes)
- **Monitor health** and automatically restart crashed processes
- **View logs** in a beautiful terminal UI
- **Track metrics** for all your running processes

Perfect for microservices development, monorepo projects, and complex local development setups.

## Installation

### Prerequisites

- **macOS** (10.15+) or **Linux** (Ubuntu 20.04+, or equivalent)
- No other dependencies required!

### Quick Install

**macOS (Apple Silicon M1/M2/M3):**
```bash
curl -L https://github.com/YOUR_USERNAME/devcli/releases/latest/download/devcli-aarch64-apple-darwin.tar.gz | tar xz
sudo mv devcli /usr/local/bin/
devcli --version
```

**macOS (Intel):**
```bash
curl -L https://github.com/YOUR_USERNAME/devcli/releases/latest/download/devcli-x86_64-apple-darwin.tar.gz | tar xz
sudo mv devcli /usr/local/bin/
devcli --version
```

**Linux (x64):**
```bash
curl -L https://github.com/YOUR_USERNAME/devcli/releases/latest/download/devcli-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv devcli /usr/local/bin/
devcli --version
```

### Build from Source

```bash
git clone https://github.com/YOUR_USERNAME/devcli.git
cd devcli
cargo build --release
sudo cp target/release/devcli /usr/local/bin/
```

## Quick Start (5 minutes)

### 1. Initialize Configuration

```bash
# Create default config file at ~/.devcli/config.yaml
devcli config init

# Verify it was created
cat ~/.devcli/config.yaml
```

### 2. Auto-Detect Your First App

Navigate to any Node.js, Docker, or Kubernetes project:

```bash
cd ~/my-awesome-api
devcli auto-add

# This detects your app type and adds it to config
# ✓ Detected: nodejs app "my-awesome-api"
# ✓ Found commands: start, test, build
# ✓ Added to config
```

### 3. Start Your App

```bash
# Start in local environment (default)
devcli start my-awesome-api

# Or start in Docker
devcli start my-awesome-api --env docker

# Or with dependencies auto-started
devcli start frontend  # Also starts api, database, etc.
```

### 4. Check Status

```bash
devcli status

# Output:
# ┌─────────────────────────────────────────┐
# │ Running Processes                       │
# ├─────────────────────────────────────────┤
# │ my-awesome-api (PID 12345) ✓ healthy   │
# │   Uptime: 2m 34s                        │
# │   Project: awesome-project              │
# └─────────────────────────────────────────┘
```

### 5. View Logs

```bash
# Launch interactive TUI to view logs
devcli ui

# Or get current status
devcli status my-awesome-api
```

### 6. Stop

```bash
# Stop specific app
devcli stop my-awesome-api

# Stop all apps
devcli stop --all
```

## Core Concepts

### Configuration File

Located at `~/.devcli/config.yaml`, this file defines:

```yaml
projects:
  awesome-project:
    apps:
      api:
        app_type: nodejs
        path: ~/code/awesome-project/api
        commands:
          local:
            start: npm run dev
            test: npm test
          docker:
            start: docker compose up api
        dependencies:
          - database
        health_check:
          http:
            url: http://localhost:3000/health
            expected_status: 200
        restart_policy:
          max_restarts: 5
          restart_window_secs: 300
```

### Projects

A **project** is a logical grouping of related apps (e.g., your microservices).

### Apps

An **app** is a single process/service you want to manage. Each app has:
- **Commands** for different environments (local, docker, k8s)
- **Dependencies** on other apps
- **Health checks** to monitor status
- **Restart policies** for automatic recovery

### Environments

Run the same app in different environments:
- **local**: Native execution (npm, cargo, go, etc.)
- **docker**: Docker Compose or standalone containers
- **k8s**: Kubernetes manifests
- **orbstack**: OrbStack optimized commands

## Common Workflows

### Workflow 1: Starting a Full Stack

```bash
# Start frontend (also starts api, database, redis)
devcli start frontend

# Check what's running
devcli status --deps

# View logs in TUI
devcli ui
```

### Workflow 2: Development with Hot Reload

```bash
# Start in local mode with hot reload
devcli start api --env local

# Make code changes... app reloads automatically

# Run tests in separate terminal
devcli run api test
```

### Workflow 3: Docker Development

```bash
# Start everything in Docker
devcli start --all --env docker

# Health checks ensure containers are ready
devcli health-check api

# View container logs
devcli ui
```

### Workflow 4: Managing Long-Running Services

```bash
# Start with monitor daemon (auto-restarts on crash)
devcli start database

# Monitor starts automatically in background
# Check metrics
devcli metrics

# View metrics API
curl http://localhost:9090/metrics | jq
```

## Next Steps

### Explore Commands

```bash
# List all apps in config
devcli config list

# Show details of an app
devcli config show api

# Run custom command
devcli run api build:production

# Check health manually
devcli health-check api
```

### Configure Health Checks

Add health checks to detect when apps need restart:

```yaml
health_check:
  http:
    url: http://localhost:3000/health
    timeout_secs: 5
    expected_status: 200
```

### Set Up Restart Policies

Automatically restart crashed processes:

```yaml
restart_policy:
  max_restarts: 5          # Max 5 restarts
  restart_window_secs: 300 # Within 5 minutes
  backoff_secs: 5          # Wait 5s between restarts
```

### Add Environment Files

Manage environment-specific variables:

```bash
# Add .env file for different stages
devcli env add api --stage qa --context docker --file .env.qa
devcli env add api --stage prod --context k8s --file .env.prod

# List env files
devcli env list api

# Start with specific stage
devcli start api --stage qa
```

### Set Preferences

```bash
# Set default environment
devcli pref set default-env docker

# Enable auto-start dependencies
devcli pref set auto-start-deps true

# View current preferences
devcli pref show
```

## Detailed Guides

- **[Configuration Reference](./configuration-reference.md)** - Complete config file documentation
- **[Commands Reference](./commands-reference.md)** - All CLI commands with examples
- **[Advanced Features](./advanced-features.md)** - Health checks, metrics, logging
- **[Troubleshooting](./troubleshooting.md)** - Common issues and solutions

## Tips & Tricks

### Tip 1: Use Tab Completion

```bash
# Install completion (bash example)
devcli --generate-completion bash > /etc/bash_completion.d/devcli
```

### Tip 2: Filter Logs

```bash
# Set log level
RUST_LOG=debug devcli start api

# View structured JSON logs
tail -f ~/.devcli/logs/devcli.$(date +%Y-%m-%d).json | jq
```

### Tip 3: Quick Status Check

```bash
# Add alias to your shell
alias dc='devcli'
alias dcs='devcli status'
alias dcl='devcli ui'

# Now use shortcuts
dcs
dcl
```

### Tip 4: Multiple Environments

```bash
# Dev environment
devcli start api --env local

# Test environment (Docker)
devcli start api --env docker --stage qa

# Production simulation (K8s)
devcli start api --env k8s --stage preprod
```

## Getting Help

```bash
# General help
devcli --help

# Command-specific help
devcli start --help
devcli config --help

# Check version
devcli --version
```

## What's Next?

Now that you have the basics:

1. **Explore the TUI**: `devcli ui` for beautiful log viewing
2. **Set up health checks**: Automatically detect crashes
3. **Configure dependencies**: Start services in order
4. **View metrics**: `devcli metrics` for insights
5. **Customize config**: Add all your projects

Happy process managing! 🚀
