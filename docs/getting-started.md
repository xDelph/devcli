# Getting Started with DevCLI

A comprehensive guide to get you up and running with devcli in minutes.

## What is devcli?

devcli is a powerful process management tool that helps you:

- **Manage processes** through a centralized JSON configuration
- **Auto-start dependencies** in the correct order
- **Run in multiple environments** (local, Docker, OrbStack, Docker Compose,
  Kubernetes, CI)
- **Monitor health** and automatically restart crashed processes
- **View logs** in a beautiful terminal UI
- **Track metrics** for all your running processes

Perfect for microservices development, monorepo projects, and complex local development setups.

## Installation

### Prerequisites

- **macOS** (10.15+) or **Linux** (Ubuntu 20.04+, or equivalent)
- No other dependencies required!

### Quick Install (Recommended)

Installs the latest `devcli` and its `pm-daemon` background daemon into `~/.devcli/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/develop/install.sh | sh
```

### Homebrew

```bash
brew tap xDelph/devcli
brew install devcli
```

### Manual Installation

**macOS (Apple Silicon M1/M2/M3):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-aarch64-apple-darwin.tar.gz | tar xz
sudo mv devcli pm-daemon /usr/local/bin/
devcli --version
```

**macOS (Intel):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-x86_64-apple-darwin.tar.gz | tar xz
sudo mv devcli pm-daemon /usr/local/bin/
devcli --version
```

**Linux (x64):**
```bash
curl -L https://github.com/xDelph/devcli/releases/latest/download/devcli-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv devcli pm-daemon /usr/local/bin/
devcli --version
```

### Build from Source

```bash
git clone https://github.com/xDelph/devcli.git
cd devcli
cargo build --release
sudo cp target/release/devcli target/release/pm-daemon /usr/local/bin/
```

## Quick Start (5 minutes)

### 1. Initialize Configuration

```bash
# Create an empty config file at ~/.devcli/config.json
# (it also prints an example project you can copy)
devcli config init

# Verify it was created
cat ~/.devcli/config.json
# {
#   "projects": {}
# }
```

### 2. Auto-Detect Your First App

Navigate to any Node.js, Docker, or Kubernetes project:

```bash
cd ~/my-awesome-api
devcli auto-add

# This detects your app type and adds it to config
# (asks a few questions; pass --yes to accept the detected defaults)
# Detecting app in ~/my-awesome-api...
#
# ✓ Detected: nodejs app
#   - Local: 3 commands available
#
# ✓ Added successfully!
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

By default `start` stays attached: it streams the app's output and stops the
app on Ctrl+C. Add `--detached` (or `devcli pref set detached-mode true`) to
get your prompt back while the app keeps running.

### 4. Check Status

```bash
devcli status

# Output:
# PROJECT: global
#   [my-awesome-api (local) [start]]  PID: 12345  running  2m  npm run start
```

(`auto-add --yes` puts apps in the project `global` unless you pass
`--project`.)

### 5. View Logs

```bash
# Launch interactive TUI to view logs
devcli ui

# Or print the last lines of the app's log file
devcli logs my-awesome-api -n 50
```

### 6. Stop

```bash
# Stop specific app
devcli stop my-awesome-api

# Stop all apps (refused when run by an AI agent)
devcli stop --all
```

## Core Concepts

### Configuration File

Located at `~/.devcli/config.json`, this file defines:

```json
{
  "projects": {
    "awesome-project": {
      "apps": {
        "api": {
          "type": "nodejs",
          "path": "~/code/awesome-project/api",
          "commands": {
            "local": {
              "start": "npm run dev",
              "test": "npm test"
            },
            "docker": {
              "start": "docker compose up api"
            }
          },
          "defaults": {
            "local": "start",
            "docker": "start"
          },
          "dependencies": [
            { "project": "awesome-project", "app": "database" }
          ],
          "health_check": {
            "type": "http",
            "url": "http://localhost:3000/health",
            "expected_status": 200
          },
          "restart_policy": {
            "max_restarts": 5,
            "restart_window_secs": 300
          }
        },
        "database": {
          "type": "docker",
          "path": "~/code/awesome-project/docker",
          "commands": {
            "local": {
              "start": "docker compose up postgres"
            }
          },
          "defaults": {
            "local": "start"
          },
          "health_check": {
            "type": "tcp",
            "host": "localhost",
            "port": 5432
          }
        }
      }
    }
  }
}
```

### Projects

A **project** is a logical grouping of related apps (e.g., your microservices).

### Apps

An **app** is a single process/service you want to manage. Each app has:
- **Commands** for different environments (local, docker, orbstack,
  docker-compose, k8s, ci)
- **Dependencies** on other apps
- **Health checks** to monitor status
- **Restart policies** for automatic recovery

### Environments

Run the same app in different environments:
- **local**: Native execution (npm, cargo, go, etc.)
- **docker**: Docker Compose or standalone containers
- **orbstack**: Docker commands with the OrbStack context
- **docker-compose** (`--env compose` also works): Docker Compose commands
- **k8s**: Kubernetes manifests
- **ci**: CI pipeline commands

Commands run without a shell: wrap pipes, `&&` or `VAR=value` prefixes in
`sh -c '...'`.

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
# Start the apps you need in Docker (dependencies follow)
devcli start api worker frontend --env docker

# Health checks ensure containers are ready
devcli health-check api

# View container logs
devcli ui
```

### Workflow 4: Managing Long-Running Services

```bash
# Start it; pm-daemon (health checks, auto-restart) starts automatically in
# the background
devcli start database --detached

# Check metrics (computed from the process state)
devcli metrics
```

Automatic restarts need a `restart_policy` on the app (see below).

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

```json
{
  "health_check": {
    "type": "http",
    "url": "http://localhost:3000/health",
    "timeout_secs": 5,
    "expected_status": 200
  }
}
```

The other types are `tcp` (`host`, `port`), `command` (`command`,
`expected_exit_code`) and `process` (alive is healthy). See the
[Configuration Reference](./configuration-reference.md#health_check).

### Set Up Restart Policies

Automatically restart crashed processes:

```json
{
  "restart_policy": {
    "max_restarts": 5,
    "restart_window_secs": 300,
    "initial_backoff_secs": 1,
    "max_backoff_secs": 60
  }
}
```

The delay before each restart doubles (1s, 1s, 2s, 4s, ...) up to
`max_backoff_secs`.

### Add Environment Files

Manage environment-specific variables:

```bash
# Add .env file for different stages
devcli env add api --stage qa --context docker --file .env.qa
devcli env add api --stage prod --context k8s --file .env.prod

# List env files
devcli env list api

# Start with specific stage (dev, qa, preprod or prod)
devcli start api --stage qa --env docker
```

### Set Preferences

```bash
# Set default environment
devcli pref set default-env docker

# Enable auto-start dependencies
devcli pref set auto-start-deps true

# Return right away from start instead of streaming output
devcli pref set detached-mode true

# Stage used when --stage is not given
devcli pref set default-stage dev

# View current preferences
devcli pref show
```

## Detailed Guides

- **[Configuration Reference](./configuration-reference.md)** - Complete config file documentation
- **[Commands Reference](./commands-reference.md)** - All CLI commands with examples
- **[Advanced Features](./advanced-features.md)** - Health checks, metrics, logging
- **[Troubleshooting](./troubleshooting.md)** - Common issues and solutions

## Tips & Tricks

### Tip 1: Filter Logs

```bash
# Set log level (off, error, warn, info, debug, trace)
LOG_LEVEL=debug devcli start api

# View devcli's structured JSON logs
tail -f ~/.devcli/logs/devcli.json.$(date +%Y-%m-%d) | jq
```

### Tip 2: Quick Status Check

```bash
# Add alias to your shell
alias dc='devcli'
alias dcs='devcli status'
alias dcl='devcli ui'

# Now use shortcuts
dcs
dcl
```

### Tip 3: Multiple Environments

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
