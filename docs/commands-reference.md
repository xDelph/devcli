# Commands Reference

Complete reference for all devcli CLI commands.

## Table of Contents

- [Process Management](#process-management)
  - [start](#start) - Start apps
  - [stop](#stop) - Stop apps
  - [restart](#restart) - Restart apps
  - [run](#run) - Run command variants
  - [status](#status) - Show process status
- [Monitoring](#monitoring)
  - [monitor](#monitor) - Health monitoring daemon
  - [health-check](#health-check) - Manual health check
  - [metrics](#metrics) - View metrics
  - [ui](#ui) - Launch TUI
- [Configuration](#configuration)
  - [config init](#config-init) - Initialize config
  - [config validate](#config-validate) - Validate config
  - [config list](#config-list) - List apps
  - [config show](#config-show) - Show app details
  - [config edit](#config-edit) - Edit config file
  - [config add-command](#config-add-command) - Add command
  - [config remove-command](#config-remove-command) - Remove command
  - [config set-default](#config-set-default) - Set default command
  - [config list-commands](#config-list-commands) - List commands
  - [config edit-command](#config-edit-command) - Edit command
- [Environment Management](#environment-management)
  - [env add](#env-add) - Add env file
  - [env remove](#env-remove) - Remove env file
  - [env list](#env-list) - List env files
  - [env set-default](#env-set-default) - Set default stage
- [Preferences](#preferences)
  - [pref set](#pref-set) - Set preference
  - [pref show](#pref-show) - Show preferences
  - [pref reset](#pref-reset) - Reset preferences
- [Utilities](#utilities)
  - [auto-add](#auto-add) - Auto-detect and add app

---

## Process Management

### `start`

Start one or more applications.

**Usage**:
```bash
devcli start <app-names>... [OPTIONS]
```

**Arguments**:
- `<app-names>` - One or more app names to start

**Options**:
- `-p, --project <NAME>` - Project name (if app name is ambiguous)
- `-e, --env <ENV>` - Environment: `local`, `docker`, `k8s` (overrides preference)
- `--skip-deps` - Skip starting dependencies
- `-s, --stage <STAGE>` - Deployment stage: `dev`, `qa`, `prod`, etc.

**Examples**:
```bash
# Start single app
devcli start api

# Start multiple apps
devcli start api worker frontend

# Start in Docker environment
devcli start api --env docker

# Start with dependencies skipped
devcli start api --skip-deps

# Start with specific stage
devcli start api --stage qa --env docker

# Start with project disambiguation
devcli start api --project awesome-project
```

**Behavior**:
- Auto-starts dependencies (unless `--skip-deps`)
- Spawns process in background
- Starts monitor daemon automatically
- Tracks process state in `~/.devcli/state.json`
- Logs to `~/.devcli/logs/<project>_<app>_<date>.log`

**Dependencies**:
```bash
# frontend depends on api, api depends on database
devcli start frontend
# → Starts: database → api → frontend
```

---

### `stop`

Stop one or more running applications.

**Usage**:
```bash
devcli stop [app-name] [OPTIONS]
```

**Arguments**:
- `[app-name]` - Optional app name (if not using --all or --project)

**Options**:
- `-p, --project <NAME>` - Stop all apps in project
- `--all` - Stop all running processes
- `--force` - Force kill (SIGKILL instead of SIGTERM)

**Examples**:
```bash
# Stop specific app
devcli stop api

# Stop all apps in project
devcli stop --project awesome-project

# Stop everything
devcli stop --all

# Force kill
devcli stop api --force
```

**Behavior**:
- Sends SIGTERM by default (graceful shutdown)
- Waits 5 seconds for graceful exit
- Uses SIGKILL if --force or after timeout
- Removes from state tracking
- Monitor daemon exits when no processes remain

---

### `restart`

Restart a running application with same configuration.

**Usage**:
```bash
devcli restart <app-name> [OPTIONS]
```

**Arguments**:
- `<app-name>` - App name to restart

**Options**:
- `-p, --project <NAME>` - Project name
- `-e, --env <ENV>` - Environment (can change from current)
- `--skip-deps` - Skip restarting dependencies

**Examples**:
```bash
# Restart app
devcli restart api

# Restart in different environment
devcli restart api --env docker

# Restart without dependencies
devcli restart api --skip-deps
```

**Behavior**:
- Stops current process
- Starts new process with same config
- Preserves restart count
- Can change environment on restart

---

### `run`

Run a specific command variant for an app.

**Usage**:
```bash
devcli run <app-name> <command-variant> [OPTIONS]
```

**Arguments**:
- `<app-name>` - App name
- `<command-variant>` - Command variant (e.g., `test`, `build:production`)

**Options**:
- `-p, --project <NAME>` - Project name
- `-e, --env <ENV>` - Environment
- `--skip-deps` - Skip dependencies

**Examples**:
```bash
# Run tests
devcli run api test

# Run production build
devcli run api build:production

# Run in Docker
devcli run api test --env docker

# Run specific variant
devcli run api build:staging
```

**Command Variants**:
```yaml
# In config:
commands:
  local:
    test: npm test
    test:unit: npm run test:unit
    test:e2e: npm run test:e2e
    build: npm run build
    build:production: npm run build:prod
```

---

### `status`

Show status of running processes.

**Usage**:
```bash
devcli status [app-name] [OPTIONS]
```

**Arguments**:
- `[app-name]` - Optional app name to filter

**Options**:
- `-p, --project <NAME>` - Filter by project
- `--deps` - Show dependency status

**Examples**:
```bash
# Show all running processes
devcli status

# Show specific app
devcli status api

# Show project apps
devcli status --project awesome-project

# Show with dependencies
devcli status --deps
```

**Output**:
```
┌─────────────────────────────────────────────┐
│ Running Processes                           │
├─────────────────────────────────────────────┤
│ api (PID 12345) ✓ healthy                  │
│   Project: awesome-project                  │
│   Environment: local                        │
│   Uptime: 1h 23m 45s                       │
│   Restarts: 0                              │
│   Health: ✓ http://localhost:3000/health  │
├─────────────────────────────────────────────┤
│ worker (PID 12346) ● running               │
│   Project: awesome-project                  │
│   Environment: docker                       │
│   Uptime: 45m 12s                          │
│   Restarts: 2                              │
└─────────────────────────────────────────────┘
```

---

## Monitoring

### `monitor`

Run health monitoring daemon (usually auto-started).

**Usage**:
```bash
devcli monitor [OPTIONS]
```

**Options**:
- `--daemon` - Run as background daemon

**Examples**:
```bash
# Run daemon (internal use, auto-started by 'start')
devcli monitor --daemon

# Manual cleanup run (without daemon)
devcli monitor
```

**Behavior**:
- Monitors all running processes
- Performs health checks every 3 seconds
- Auto-restarts crashed processes (with restart_policy)
- Tracks metrics
- Exposes metrics API on localhost:9090
- Exits automatically when no processes remain

---

### `health-check`

Manually check health of a running app.

**Usage**:
```bash
devcli health-check <app-name> [OPTIONS]
```

**Arguments**:
- `<app-name>` - App name to check

**Options**:
- `-e, --env <ENV>` - Environment

**Examples**:
```bash
# Check health
devcli health-check api

# Check in Docker
devcli health-check api --env docker
```

**Output**:
```
Health Check: api
✓ HTTP check passed
  URL: http://localhost:3000/health
  Status: 200 OK
  Response time: 45ms
```

---

### `metrics`

Display metrics from the monitor daemon.

**Usage**:
```bash
devcli metrics
```

**Examples**:
```bash
# Show all metrics
devcli metrics
```

**Output**:
```
=== Process Metrics ===
Total Processes:       3
  Running:             3 ●
  Stopped:             0 ○

Total Restarts:        5
Restarts (last hour):  2
Health Check Success:  98.5%

Exit Code Distribution:
  Exit 0: 3 times ✓
  Exit 1: 2 times ✗

=== System Metrics ===
Monitor Uptime:        3600 seconds (60 minutes)
Loop Iterations:       1200
DevCLI Version:        0.1.0
Apps Configured:       12

=== Performance Metrics ===
Avg Startup Time:      450.23 ms
Avg Health Check:      42.15 ms
Avg Restart Time:      1023.45 ms

Recent Operations (last 5):
  ✓ start - api (523 ms)
  ✓ health_check - api (38 ms)
  ✓ restart - worker (1250 ms)
```

**HTTP API**:
```bash
# Raw JSON via API
curl http://localhost:9090/metrics | jq
```

---

### `ui`

Launch interactive TUI for viewing logs.

**Usage**:
```bash
devcli ui
```

**Features**:
- View logs for all running processes
- Real-time log streaming
- Syntax highlighting
- Multi-pane view
- Keyboard navigation
- ANSI color support

**Keyboard Shortcuts**:
- `↑/↓` or `j/k` - Scroll
- `Tab` - Switch between apps
- `g/G` - Jump to top/bottom
- `h` - Help overlay
- `q` - Quit

---

## Configuration

### `config init`

Initialize a new configuration file.

**Usage**:
```bash
devcli config init
```

**Examples**:
```bash
# Create default config
devcli config init

# Output: Created config at ~/.devcli/config.yaml
```

**Behavior**:
- Creates `~/.devcli/config.yaml`
- Starts with empty projects
- Won't overwrite existing file

---

### `config validate`

Validate the configuration file.

**Usage**:
```bash
devcli config validate
```

**Examples**:
```bash
devcli config validate
```

**Checks**:
- YAML syntax
- Required fields
- Dependency existence
- Circular dependencies
- Health check format
- Command structure

**Output**:
```
✓ Config is valid
✓ 3 projects, 8 apps
✓ No circular dependencies
✓ All dependencies exist
```

---

### `config list`

List all projects and apps.

**Usage**:
```bash
devcli config list [OPTIONS]
```

**Options**:
- `-p, --project <NAME>` - Filter by project
- `--apps-only` - Show only app names

**Examples**:
```bash
# List everything
devcli config list

# List specific project
devcli config list --project awesome-project

# List just app names
devcli config list --apps-only
```

**Output**:
```
Projects:

awesome-project:
  - api (nodejs)
  - worker (nodejs)
  - frontend (nodejs)
  - database (docker)

monitoring:
  - prometheus (docker)
  - grafana (docker)
```

---

### `config show`

Show detailed configuration for an app.

**Usage**:
```bash
devcli config show <app-name> [OPTIONS]
```

**Arguments**:
- `<app-name>` - App name

**Options**:
- `-p, --project <NAME>` - Project name

**Examples**:
```bash
devcli config show api
devcli config show api --project awesome-project
```

**Output**:
```yaml
App: api
Project: awesome-project
Type: nodejs
Path: ~/code/awesome-project/apps/api

Commands:
  local:
    start: npm run dev
    test: npm test
    build: npm run build

Dependencies:
  - database
  - redis

Health Check:
  http:
    url: http://localhost:3000/health
    expected_status: 200

Restart Policy:
  max_restarts: 5
  restart_window_secs: 300
```

---

### `config edit`

Open configuration file in editor.

**Usage**:
```bash
devcli config edit
```

**Examples**:
```bash
devcli config edit
```

**Behavior**:
- Opens `~/.devcli/config.yaml` in `$EDITOR`
- Falls back to `vim` if `$EDITOR` not set

---

### `config add-command`

Add a command to an app (interactive).

**Usage**:
```bash
devcli config add-command [app-name] [environment] [command-name] [command-value] [OPTIONS]
```

**Examples**:
```bash
# Interactive (prompts for all fields)
devcli config add-command

# Partial (prompts for missing)
devcli config add-command api

# Full command
devcli config add-command api local build "npm run build"
```

---

### `config remove-command`

Remove a command from an app (interactive).

**Usage**:
```bash
devcli config remove-command [app-name] [environment] [command-name] [OPTIONS]
```

---

### `config set-default`

Set the default command for an environment.

**Usage**:
```bash
devcli config set-default [app-name] [environment] [command-name] [OPTIONS]
```

---

### `config list-commands`

List all commands for an app.

**Usage**:
```bash
devcli config list-commands [app-name] [OPTIONS]
```

**Options**:
- `-p, --project <NAME>` - Project name
- `-e, --env <ENV>` - Filter by environment

---

### `config edit-command`

Edit an existing command (interactive).

**Usage**:
```bash
devcli config edit-command [app-name] [environment] [command-name] [OPTIONS]
```

---

## Environment Management

### `env add`

Add an environment file mapping.

**Usage**:
```bash
devcli env add <app-name> --stage <STAGE> --context <CONTEXT> --file <FILE>
```

**Options**:
- `--stage <STAGE>` - Stage name (dev, qa, prod, etc.)
- `--context <CONTEXT>` - Context (local, docker, k8s)
- `--file <FILE>` - Path to env file (relative to app path)

**Examples**:
```bash
devcli env add api --stage dev --context local --file .env.dev
devcli env add api --stage qa --context docker --file .env.qa
```

---

### `env remove`

Remove an environment file mapping.

**Usage**:
```bash
devcli env remove <app-name> --stage <STAGE> [--context <CONTEXT>]
```

**Examples**:
```bash
# Remove specific context
devcli env remove api --stage dev --context local

# Remove entire stage
devcli env remove api --stage dev
```

---

### `env list`

List environment files for an app.

**Usage**:
```bash
devcli env list <app-name>
```

**Examples**:
```bash
devcli env list api
```

**Output**:
```
Environment Files for api:

dev:
  local: .env.dev
  docker: .env.docker.dev

qa:
  docker: .env.qa
  k8s: k8s/qa/env.yaml

prod:
  k8s: k8s/prod/env.yaml
```

---

### `env set-default`

Set default stage for a context.

**Usage**:
```bash
devcli env set-default <app-name> --context <CONTEXT> --stage <STAGE>
```

**Examples**:
```bash
devcli env set-default api --context docker --stage dev
```

---

## Preferences

### `pref set`

Set a preference value.

**Usage**:
```bash
devcli pref set <key> <value>
```

**Available Keys**:
- `default-env` - Default environment (`local`, `docker`, `k8s`)
- `auto-start-deps` - Auto-start dependencies (`true`, `false`)
- `docker-platform` - Docker platform (`linux/amd64`, `linux/arm64`)

**Examples**:
```bash
devcli pref set default-env docker
devcli pref set auto-start-deps false
devcli pref set docker-platform linux/arm64
```

---

### `pref show`

Show current preferences.

**Usage**:
```bash
devcli pref show
```

**Output**:
```
Current Preferences:
  default-env:       local
  auto-start-deps:   true
  docker-platform:   linux/amd64
```

---

### `pref reset`

Reset preferences to defaults.

**Usage**:
```bash
devcli pref reset
```

---

## Utilities

### `auto-add`

Auto-detect app and add to configuration.

**Usage**:
```bash
devcli auto-add [OPTIONS]
```

**Options**:
- `--path <PATH>` - Path to detect (defaults to current directory)

**Examples**:
```bash
# Detect current directory
cd ~/my-app
devcli auto-add

# Detect specific path
devcli auto-add --path ~/code/another-app
```

**Supported Detection**:
- Node.js (`package.json`)
- Nx Monorepo (`nx.json`)
- Docker (`Dockerfile`, `docker-compose.yml`)
- Kubernetes (`k8s/*.yaml`)
- Rust (`Cargo.toml`)
- Go (`go.mod`)
- Python (`setup.py`, `pyproject.toml`)

---

## Global Options

Available for all commands:

- `-h, --help` - Show help
- `-V, --version` - Show version

**Examples**:
```bash
devcli --help
devcli start --help
devcli --version
```

---

## Environment Variables

### `RUST_LOG`

Control logging verbosity:

```bash
# Debug logs
RUST_LOG=debug devcli start api

# Trace (very verbose)
RUST_LOG=trace devcli start api

# Module-specific
RUST_LOG=devcli_core::commands::start=debug devcli start api
```

### `DEVCLI_CONFIG`

Override config file location:

```bash
export DEVCLI_CONFIG=~/my-config.yaml
devcli start api
```

---

## Exit Codes

- `0` - Success
- `1` - General error
- Other codes preserved from child processes

---

## See Also

- [Getting Started](./getting-started.md) - Quick start guide
- [Configuration Reference](./configuration-reference.md) - Config file format
- [Advanced Features](./advanced-features.md) - Health checks, metrics, logging
- [Troubleshooting](./troubleshooting.md) - Common issues
