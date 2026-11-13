# Commands Reference

Complete reference for all devcli CLI commands.

## Table of Contents

- [Process Management](#process-management)
  - [start](#start) - Start apps
  - [stop](#stop) - Stop apps
  - [restart](#restart) - Restart apps
  - [run](#run) - Run command variants
  - [status](#status) - Show process status
  - [logs](#logs) - Show app logs
  - [worktrees](#worktrees) - Git worktrees and their instances
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

> **Git worktrees.** Every command taking an app name runs, from inside a
> git worktree of the app's repository, against that worktree's copy of the
> app (project `proj@slug`). `APP@slug` names a worktree's instance from
> anywhere, `APP@` the configured checkout, and `APP@*` every checkout
> (`start`, `stop`, `status`). The suffix also works on `--project`. See
> [`worktrees`](#worktrees). Under an AI agent, `start` and `stop` refuse
> `@*`.
>
> **Unknown names.** An app name that is neither configured nor running is an
> error (exit code 1, with a "did you mean" suggestion when a configured name
> is close), for `start` and `stop` as for `status`.

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
- `-e, --env <ENV>` - Environment: `local`, `docker`, `orbstack`, `docker-compose` (alias `compose`), `k8s` (alias `kubernetes`), `ci` (overrides preference)
- `--skip-deps` - Skip starting dependencies
- `-s, --stage <STAGE>` - Deployment stage: `dev`, `qa`, `preprod` or `prod` (overrides the app's `default_stages` and the `default-stage` preference)
- `--detached` - Leave the apps running in the background and return immediately (overrides the detached-mode preference). `--json` also implies this.

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
- **Attached** (default): streams the apps' output and, like
  `docker compose up`, stops the apps it started (dependencies included) on
  Ctrl+C, SIGTERM or when the terminal closes. Press Ctrl+C a second time to
  kill them right away. Apps that were already running are left alone. If
  `devcli` itself is killed, `pm-daemon` stops its apps within a few seconds.
- **Detached** (`--detached`, `--json`, or `devcli pref set detached-mode true`):
  returns immediately; the apps keep running until `devcli stop`. Under an AI
  agent, `start` is always detached.
- Apps asked for that already run are left alone (`--json` lists them in
  `already_running`)
- Spawns `pm-daemon` (health checks, auto-restart) if it is not running
- Tracks process state in `~/.devcli/processes/<project>.<app>.<env>.json`
- Logs to `~/.devcli/logs/<project>_<app>_<env>_<YYYYMMDD>.log`

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
- `--dry-run` - Report what would be stopped without sending any signal

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

**Under an AI agent**: `stop --all`, `stop --project P` without an app, and
`stop APP@*` (every worktree) are refused (exit code 1), since they would also
stop what the user and other agents run (`stop --project P` run inside one of
the project's worktrees only reaches that worktree, and is allowed). devcli recognises agents by the variables their harnesses set on
the commands they run (`CLAUDE_CODE_CHILD_SESSION`/`CLAUDECODE`,
`CODEX_SANDBOX`/`CODEX_THREAD_ID`, `GEMINI_CLI`, `CURSOR_AGENT`, `OPENCODE`,
`PI_CODING_AGENT`, `AI_AGENT`, ...). `--dry-run` is still allowed. Set
`DEVCLI_AGENT=1` to apply this to a harness devcli does not recognise, or
`DEVCLI_AGENT=0` where it misfires (the integrated terminal of Claude Code's
IDE extensions also sets `CLAUDECODE`). This guards against mistakes; it is not
a security boundary.

**Behavior**:
- Sends SIGTERM by default (graceful shutdown), to the app's whole process group
- Waits 5 seconds for graceful exit
- Uses SIGKILL if --force or after timeout
- Removes from state tracking
- An app name that is neither configured nor running is an error (exit code 1);
  a configured app that is not running is reported, with exit code 0
- `pm-daemon` exits when no processes remain

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
- `-e, --env <ENV>` - Environment: `local`, `docker`, `orbstack`, `docker-compose` (alias `compose`), `k8s` (alias `kubernetes`), `ci` (defaults to the one it runs in)
- `--skip-deps` - Skip restarting dependencies
- `--detached` - Leave the app running in the background and return immediately (overrides the detached-mode preference). `--json` also implies this.

**Examples**:
```bash
# Restart app
devcli restart api

# Restart and return, leaving it running in the background
devcli restart api --detached

# Restart in different environment
devcli restart api --env docker

# Restart without dependencies
devcli restart api --skip-deps
```

**Behavior**:
- Fails (exit code 1) when the app is not running: use `start`
- Stops current process
- Starts it again with the same command (the environment's default, or the
  variant last given to `run`), attached or detached like `start`; under an AI
  agent it always returns right away
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
- `-e, --env <ENV>` - Environment: `local`, `docker`, `orbstack`, `docker-compose` (alias `compose`), `k8s` (alias `kubernetes`), `ci`
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
devcli run api test:e2e
```

**Behavior**:
- Starts the command in the background, with its output in the app's log file
  (`~/.devcli/logs/<project>_<app>_<env>_<YYYYMMDD>.log`, the same file as
  `start`)
- Then streams that output until Ctrl+C, which stops viewing but leaves the
  command running; with `--json`, the `detached-mode` preference or under an AI
  agent it returns right away
- Only one process per app and environment is tracked: `run` fails (exit code
  1) when the app already runs in that environment, whatever its command

**Command Variants**:
```json
{
  "commands": {
    "local": {
      "test": "npm test",
      "test:unit": "npm run test:unit",
      "test:e2e": "npm run test:e2e",
      "build": "npm run build",
      "build:production": "npm run build:prod"
    }
  }
}
```

---

### `status`

Show status of running processes **defined in the current config**.

**Usage**:
```bash
devcli status [app-name] [OPTIONS]
```

> **Note**: The status command only displays processes that are defined in the currently loaded config file (either local `./.devcli/config.json` or global `~/.devcli/config.json`). This ensures you only see relevant processes for your current context.

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
PROJECT: awesome-project
  [api (local) [start]]  PID: 12345  running  1h 23m  npm run dev
  [worker (docker) [start]]  PID: 12346  running  45m  docker compose up worker
```

Each line gives the app, its environment, the command name, PID, state,
uptime and the command. With no tracked process it prints
`No processes are currently tracked` and exits with code 1.

`devcli status api --deps`:
```
App: api (Project: awesome-project)

Dependencies:
  - awesome-project/database ✓ running (PID: 12340, env: local, cmd: start)

Status: running
✓ running (PID: 12345, env: local, cmd: start)
Uptime: 1h 23m
```

---

### `logs`

Show the captured stdout/stderr for an app.

**Usage**:
```bash
devcli logs <app-name> [OPTIONS]
```

**Options**:
- `-p, --project <NAME>` - Project name (if app name is ambiguous)
- `-e, --env <ENV>` - Filter by environment (`local`, `docker`, `orbstack`, `docker-compose` (alias `compose`), `k8s` (alias `kubernetes`), `ci`)
- `-n, --lines <N>` - Number of trailing lines to show (default `200`)
- `-f, --follow` - Follow the file and stream new lines (blocks)

**Examples**:
```bash
# Last 200 lines
devcli logs api

# Last 50 lines for a specific environment
devcli logs api --project awesome-project --env local -n 50

# Stream new lines (Ctrl+C to stop)
devcli logs api --follow

# Machine-readable (array of lines + file path)
devcli --json logs api -n 100
```

**Behavior**:
- Reads the most recent file matching
  `~/.devcli/logs/<project>_<app>_<env>_<YYYYMMDD>.log`
- Lines are the app's raw stdout/stderr; devcli adds no timestamps
- `--json` returns `{ project, app, environment, file, lines: [...] }`
- Use `--json` for agents; `--follow` is meant for humans. With both, the
  output is newline-delimited JSON: the usual document on one line, then one
  `{"line": "..."}` object per new line

---

## Monitoring

### `monitor`

Run the monitor loop in the foreground, with the metrics API (internal use).
The monitor that normally runs is `pm-daemon`, which `start` and `run` spawn
automatically.

**Usage**:
```bash
devcli monitor [OPTIONS]
```

**Options**:
- `--daemon` - Run as background daemon

**Examples**:
```bash
# Run the monitor loop and the metrics API in the foreground
devcli monitor --daemon

# One-shot cleanup of dead process entries (no daemon)
devcli monitor
```

**Behavior** (`--daemon`):
- Refuses to run (exit code 1, `A monitor (pm-daemon) is already running for
  ...`) while pm-daemon holds the monitor lock; `start` does not spawn
  pm-daemon while it runs
- Monitors all running processes
- Performs health checks every 3 seconds
- Auto-restarts crashed processes (with restart_policy)
- Tracks metrics
- Serves the metrics API on `DEVCLI_METRICS_ADDR` (default `127.0.0.1:9090`);
  pm-daemon has no HTTP server
- Exits automatically when no processes remain

---

### `worktrees`

List the git worktrees of the repositories holding configured apps.

**Usage**:
```bash
devcli worktrees [--project <NAME>]
```

**Output** (per repository): each worktree's name (`(configured)` for the
checkout in the config), port offset, branch, running apps and path; `*`
marks the current directory's worktree. `--json` gives the full document
(see AGENTS.md).

**Behavior**:
- Worktrees are found through git (`git worktree list`), wherever they live
- Each worktree keeps a slot (1, 2, ...) while it exists; `DEVCLI_PORT_OFFSET`
  is slot × 100 and an app's configured `port` is shifted by it
- Worktrees deleted without `git worktree remove` are shown as stale; run
  `git worktree prune` to forget them (devcli never does)

**Examples**:
```bash
devcli worktrees
devcli start api@agent-1     # start api in worktree agent-1
devcli stop api@*            # stop api in every checkout (refused under an AI agent)
```

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
- `-p, --project <NAME>` - Project name (if app name is ambiguous)
- `-e, --env <ENV>` - Environment: `local`, `docker`, `orbstack`, `docker-compose` (alias `compose`), `k8s` (alias `kubernetes`), `ci`

**Examples**:
```bash
# Check health
devcli health-check api

# Check in Docker
devcli health-check api --env docker
```

**Output**:
```
Checking health of: awesome-project/api
Process ID: 12345
Health check type: Http { url: "http://localhost:3000/health", timeout_secs: 5, expected_status: 200 }

Executing health check...
✅ Health check PASSED (0.01s)

Status: Healthy
```

When the check fails it prints `❌ Health check FAILED`, `Status: Unhealthy`
and the number of consecutive failures, and exits with code 2. An app that is
not running is an error (exit code 1).

---

### `metrics`

Display process, system and performance metrics. Fetched from the metrics API
when `devcli monitor --daemon` is running (`DEVCLI_METRICS_ADDR`, default
`127.0.0.1:9090`), computed from the state store otherwise (pm-daemon, the
monitor `start` spawns, serves no API; the monitor and performance counters
are then 0).

**Usage**:
```bash
devcli metrics
```

**Examples**:
```bash
# Show all metrics
devcli metrics
```

**Output** (without a metrics server):
```
=== Process Metrics ===
Total Processes:       1
  Running:             1 ●
  Stopped:             0 ○

Total Restarts:        0
Restarts (last hour):  0
Health Check Success:  100.0%

=== System Metrics ===
Monitor Uptime:        0 seconds (0 minutes)
Loop Iterations:       0
DevCLI Version:        0.6.1
Apps Configured:       2

=== Performance Metrics ===
Avg Startup Time:      0.00 ms
Avg Health Check:      0.00 ms
Avg Restart Time:      0.00 ms

=== Running Apps ===
  ✓ my-project/worker (uptime: 1s, restarts: 0)

Timestamp: 2026-10-30 18:21:24.150690 UTC
```

**HTTP API** (only while `devcli monitor --daemon` runs):
```bash
# Raw JSON via API
curl http://localhost:9090/metrics | jq
```

`devcli --json metrics` prints the same document without a server.

---

### `ui`

Launch the interactive TUI: status, commands and logs of your apps.

**Usage**:
```bash
devcli ui
```

**Features**:
- Status, Commands and Logs tabs
- Start, stop and restart apps, run their commands
- Real-time log streaming, with search
- ANSI color support

**Keyboard Shortcuts** (press `?` in the TUI for the full list):
- `↑/↓` or `k/j` - Move selection
- `Tab` - Switch panel focus
- `1` / `2` / `3` - Status, Commands, Logs tab
- `Enter` - Start (Status tab), run (Commands tab), open log viewer (Logs tab)
- `s` / `r` - Stop / restart the selected app
- `Space` - Expand/collapse a project
- In the log viewer: `PgUp/PgDn`, `Home/End`, `g/G`, `/` search, `n/N` next/previous match
- `?` - Help overlay
- `Esc` - Back / close
- `q` or `Ctrl+C` - Quit

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

# Output:
# ✓ Created empty config file at ~/.devcli/config.json
#
# Here's an example project configuration you can use as a reference:
# { "projects": { "example": { "apps": { "my-app": { ... } } } } }
#
# Edit the config file to add your projects and apps.
# Then run 'devcli config validate' to check your configuration.
```

**Behavior**:
- Creates `config.json` in the devcli directory (`DEVCLI_CONFIG_DIR`, else
  `./.devcli` when it exists, else `~/.devcli`)
- Writes `{"projects": {}}` and prints an example project to copy
- Won't overwrite an existing file (exit code 1)

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
- JSON syntax
- Required fields
- Dependency existence
- Circular dependencies
- Health check format
- Command structure

**Output**:
```
Validating configuration...

✓ Configuration is valid!
  - 3 projects
  - 8 apps
```

With problems (exit code 1 when there are errors; warnings alone do not fail):
```
Validating configuration...

⚠ Warnings:
  - [my-project/api] Unknown field 'dependecies' is ignored. Did you mean 'dependencies'?

✗ Errors:
  - [my-project/api] Default local command 'dev' not found in local commands
  - [my-project/api] Invalid dependency my-project/database: App 'database' not found in project 'my-project'.

Validation failed with 2 error(s)
Error: Config validation failed
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
PROJECT: awesome-project
  [api] (nodejs)
    Path: ~/code/awesome-project/apps/api
    Defaults: local='start'
    Dependencies:
      - awesome-project/database
  [database] (docker)
    Path: ~/code/awesome-project/docker
    Defaults: local='start', docker='start'
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
```text
App: api
Project: awesome-project
Type: nodejs
Path: ~/code/awesome-project/apps/api

Defaults: local='start'

Dependencies:
  - awesome-project/database

LOCAL Commands:
  build: npm run build
  start: npm run dev
  test: npm test

Environment Files:
  local: .env
```

The port, health check and restart policy are printed after the path when
configured. `--json` gives the full resolved app, including `env_files` and
`default_stages` (see [AGENTS.md](../AGENTS.md)).

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
- Opens the config file in use (see [config init](#config-init)) in
  `$VISUAL`, else `$EDITOR`
- Falls back to `vim` if neither is set

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
- `-e, --env <ENV>` - Filter by environment (`local`, `docker`, `orbstack`, `docker-compose` (alias `compose`), `k8s` (alias `kubernetes`), `ci`)

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
- `-s, --stage <STAGE>` - Stage: `dev`, `qa`, `preprod`, `prod`, or `base`
  (a file for every stage); any other value is refused
- `-c, --context <CONTEXT>` - Context, i.e. the environment: `local`, `docker`,
  `orbstack`, `docker-compose` (alias `compose`), `k8s` (alias `kubernetes`),
  `ci`; any other value is refused
- `-f, --file <FILE>` - Path to env file (relative to app path)
- `-p, --project <PROJECT>` - Project name (required if the app name is ambiguous)

The app is found like every other command finds it: by name, alternative name
or `project/app`; a name used by several projects is refused unless
`--project` says which. Only the given context is written: a file shared by
`docker` and `orbstack` is added once for each.

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
devcli env remove <app-name> --stage <STAGE> [--context <CONTEXT>] [--project <PROJECT>]
```

Removes exactly what `env add` wrote for that context, or the whole stage
without `--context`. A stage or context that has no entry is an error.

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
devcli env list <app-name> [--project <PROJECT>]
```

**Examples**:
```bash
devcli env list api
```

**Output**:
```
Environment files for awesome-project/api:

  base:
    • local: .env

  dev:
    • local: .env.dev

Default stages:
  • local: dev
```

---

### `env set-default`

Set default stage for a context.

**Usage**:
```bash
devcli env set-default <app-name> --context <CONTEXT> --stage <STAGE> [--project <PROJECT>]
```

**Examples**:
```bash
devcli env set-default api --context docker --stage dev
```

The stage must be `dev`, `qa`, `preprod` or `prod` (`base` is not a stage you
can start with); anything else is refused with exit code 1.

---

## Preferences

### `pref set`

Set a preference value.

**Usage**:
```bash
devcli pref set <key> <value>
```

**Available Keys**:
- `default-env` - Default environment (`local`, `docker`, `orbstack`,
  `docker-compose`, `k8s`, `ci`; default `local`). The aliases `compose` and
  `kubernetes` are accepted and saved as `docker-compose` and `k8s`.
- `detached-mode` - `start`/`restart`/`run` return right away instead of
  staying attached (`true`, `false`; default `false`)
- `auto-start-deps` - Auto-start dependencies (`true`, `false`; default `true`)
- `docker-platform` - Docker platform, `os/arch` (`linux/amd64`, `linux/arm64`;
  default `linux/amd64`)
- `default-stage` - Stage used when neither `--stage` nor the app's
  `default_stages` sets one (`dev`, `qa`, `preprod`, `prod`, or `none` to
  unset; default unset)

Boolean values also accept `yes`/`no` and `1`/`0`.

**Examples**:
```bash
devcli pref set default-env docker
devcli pref set detached-mode true
devcli pref set auto-start-deps false
devcli pref set docker-platform linux/arm64
devcli pref set default-stage dev
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
Current preferences:
  default-env: local
  detached-mode: false
  auto-start-deps: true
  docker-platform: linux/amd64
  default-stage: (not set)
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
- `-y, --yes` - Non-interactive: accept detected defaults without prompting
- `-p, --project <NAME>` - Target project (non-interactive)
- `-n, --name <NAME>` - App name to register (non-interactive)
- `--type <TYPE>` - App type override (non-interactive): `nodejs`, `nx`,
  `nx-workspace`, `rust`, `python`, `docker`, `redis`, `traefik`
- `--force` - Replace the detected settings of an app that already exists.
  What was set by hand is kept: dependencies, port, health check, restart
  policy, alternative name, default stages, and the commands, defaults and env
  files the detection does not produce (docker-compose and ci commands, commands
  added with `config add-command`). Detected commands are refreshed.

**Examples**:
```bash
# Detect current directory
cd ~/my-app
devcli auto-add

# Detect specific path
devcli auto-add --path ~/code/another-app

# Fully non-interactive (for scripts and AI agents)
devcli --json auto-add --path ./apps/api --yes --project awesome-project --name api
```

> Without a terminal and without `--yes`, `auto-add` fails fast instead of
> hanging on a prompt. Monorepos and directories containing several apps
> require the interactive mode.

**Supported Detection**:
- Node.js (`package.json`)
- Nx Monorepo (`nx.json`)
- Docker (`Dockerfile`, `compose.yaml`, `docker-compose.yml` and variants)
- Kubernetes (`k8s/*.yaml`)
- Rust (`Cargo.toml`, including workspaces)
- Python (`setup.py`, `pyproject.toml`, including uv workspaces)
- Redis (`redis.conf`) and Traefik (`traefik.yml`/`traefik.toml`) configs

---

## Global Options

Available for all commands:

- `-h, --help` - Show help
- `-V, --version` - Show version
- `--json` - Emit machine-readable JSON on stdout (also disables console logging)
- `--no-color` - Disable ANSI colors (also honours the `NO_COLOR` env var)

Colors are automatically disabled when stdout is not a terminal. `--json` is
the recommended mode for scripts and AI agents; see [AGENTS.md](../AGENTS.md).

**Examples**:
```bash
devcli --help
devcli --json status
devcli --json config list --apps-only
devcli --no-color status
devcli start --help
devcli --version
```

---

## Environment Variables

### `LOG_LEVEL`

Control logging verbosity, on stderr and in devcli's own log file
(`~/.devcli/logs/devcli.json.YYYY-MM-DD`): `off`, `error`, `warn`, `info`
(default), `debug` or `trace`. There are no per-module filters, and
`RUST_LOG` is not read.

```bash
# Debug logs
LOG_LEVEL=debug devcli start api

# Trace (very verbose)
LOG_LEVEL=trace devcli start api

# Quiet
LOG_LEVEL=off devcli start api
```

### `DEVCLI_CONFIG_DIR`

Override config directory location (config, preferences, process state and
logs all move with it). The former lowercase name `devcli_CONFIG_DIR` is still
honoured; an empty value is ignored.

```bash
export DEVCLI_CONFIG_DIR=~/.devcli
devcli start api
```

### `DEVCLI_AGENT`

`1` (or any true value) makes devcli behave as under an AI agent (detached
`start`/`restart`/`run`, bulk stops refused) for a harness it does not
recognise; `0` turns agent detection off.

### `DEVCLI_METRICS_ADDR`

Address of the metrics API served by `devcli monitor --daemon` (default
`127.0.0.1:9090`).

### `NO_COLOR`

Disable colored output, like `--no-color`.

---

## Exit Codes

- `0` - Success
- `1` - General error, or a negative result:
  - `status`: no tracked process is running
  - `config validate`: the config is invalid
- `2` - `health-check`: the app is running but unhealthy; also a command-line
  usage error (unknown subcommand or flag) without `--json`

When `--json` is set, errors are reported as `{"error": "..."}` on stderr, and
usage errors exit with `1`.

---

## See Also

- [AGENTS.md](../AGENTS.md) - Machine-oriented contract for AI agents
- [Getting Started](./getting-started.md) - Quick start guide
- [Configuration Reference](./configuration-reference.md) - Config file format
- [Advanced Features](./advanced-features.md) - Health checks, metrics, logging
- [Troubleshooting](./troubleshooting.md) - Common issues
