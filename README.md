# DevCLI

Powerful process management CLI for developers. Manage multiple processes with automatic restarts, health checks, and a beautiful terminal UI.

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

## Installation

### Quick Install (recommended, macOS & Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/xDelph/devcli/develop/install.sh | sh
```

This installs both `devcli` and its background daemon `pm-daemon` into `~/.devcli/bin`.

### Homebrew

```bash
brew tap xDelph/devcli
brew install devcli
```

### Build from source

```bash
cargo build --release --workspace
# binaries: target/release/devcli and target/release/pm-daemon
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
      "alternative_name": "ProjectX",
      "apps": {
        "api": {
          "type": "nodejs",
          "alternative_name": "Backend",
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

You can also use alternative names (configured via `alternative_name` field):

```bash
devcli start Backend  # Uses alternative_name instead of real name
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

## Using devcli with AI coding agents

devcli speaks JSON and returns meaningful exit codes, so an agent can drive it
without a TTY. The contract lives in [`AGENTS.md`](AGENTS.md) — but an agent
has no reason to know devcli exists, so install the rules once:

```bash
devcli install-agents --global     # ~/.agents/AGENTS.md  (all projects)
devcli install-agents --local .    # ./AGENTS.md          (this project)
devcli uninstall-agents --global   # remove the block again
```

`AGENTS.md` is read natively by Claude Code, Cursor, Codex, Copilot, Gemini,
Windsurf, Zed, Amp, JetBrains and Aider. The block is fenced by two markers so
devcli can refresh or remove it without touching your own lines, and the
previous file is saved as `AGENTS.md.backup` before every write.

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

### Privacy & Alternative Names

Use `alternative_name` for projects and apps to:
- Hide real names in screenshots and demos
- Use privacy-friendly names in TUI
- Start/stop apps using alternative names
- Share configuration examples publicly

```json
{
  "projects": {
    "internal-project": {
      "alternative_name": "ProjectX",
      "apps": {
        "sensitive-api": {
          "alternative_name": "Backend"
        }
      }
    }
  }
}
```

### Log Management

All process logs are automatically saved to:
```
~/.devcli/logs/<app-name>_<timestamp>.log
```

Logs include timestamps and are preserved for review.

### Process Tracking

Process state is managed by the standalone `process-manager` crate and stored in:

```
~/.devcli/processes/<project>.<app>.<environment>.json
```

Supporting files in the same directory:

```
~/.devcli/processes/.status_changed   # mtime notification for TUI polling
~/.devcli/processes/.daemon.lock      # held by pm-daemon while running
```

Each process file includes:

- Process ID (PID) and process group (PGID)
- Command, working directory, and environment
- Start time and restart history
- Project, app, and environment metadata
- Health check and restart policy from config

Health monitoring and auto-restart run in `pm-daemon`, which must be installed alongside `devcli` (release tarballs include both binaries, which install to `~/.devcli/bin`).

## AI Agents

`devcli` is designed to be driven by autonomous agents:

```bash
devcli --json status              # machine-readable process state
devcli --json config list         # full config as JSON
devcli --json logs api -n 100     # recent app logs
devcli --json start api --detached
devcli --json auto-add --path ./apps/api --yes --project proj --name api
```

- **`--json`** on every state-reporting command (also disables console logging).
- **`--no-color`** and automatic color-off when stdout is not a TTY.
- **Meaningful exit codes**: `status` (0 running / 1 not), `health-check`
  (0 healthy / 2 unhealthy / 1 error), `config validate` (1 invalid).
- **Non-interactive mode** for `auto-add` (`--yes`, `--project`, `--name`,
  `--type`); commands fail fast instead of hanging when there is no terminal.
See **[AGENTS.md](./AGENTS.md)** for the full agent contract (flags, exit
codes, JSON shapes, non-interactive usage).

## Documentation

Comprehensive documentation is available in the `docs/` directory:

### 📚 Documentation

- **[AGENTS.md](./AGENTS.md)** - Contract for AI agents (JSON, exit codes)
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

## License

`devcli` is source-available under the [PolyForm Noncommercial License 1.0.0](./LICENSE).
Commercial use requires a paid license — see [LICENSE-COMMERCIAL.md](./LICENSE-COMMERCIAL.md)
or contact devcli@delalonde.dev.
