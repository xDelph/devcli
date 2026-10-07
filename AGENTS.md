# AGENTS.md — Using devcli from AI agents

This document describes how an autonomous agent should drive `devcli`.
It is the contract for machine consumption: stable flags, exit codes, JSON
shapes, and non-interactive behaviour.

## Golden rules

1. **Always pass `--json`.** Every command that reports state has a
   machine-readable form. `--json` also implies *detached* mode for `start`
   (devcli returns immediately instead of streaming logs).
2. **Never rely on prompts.** Pass explicit flags (`--project`, `--name`,
   `--yes`, `--env`). When stdin is not a TTY, interactive commands fail
   fast with a clear error instead of hanging.
3. **Check the exit code**, not the text.
4. **Isolate state** with `devcli_CONFIG_DIR` when testing.

```bash
devcli --json status          # parse JSON on stdout
devcli --json config list
devcli --json logs api -n 100
```

Global flags (valid on any subcommand):

| Flag         | Effect                                                            |
| ------------ | ----------------------------------------------------------------- |
| `--json`     | Emit JSON on stdout; disable console logging on stderr.           |
| `--no-color` | Disable ANSI colors (also honours `NO_COLOR`).                    |

Colors are automatically disabled when stdout is not a TTY.

## Exit codes

| Command        | 0                  | 1                       | 2          |
| -------------- | ------------------ | ----------------------- | ---------- |
| `status`       | ≥1 process running | no tracked process runs | —          |
| `health-check` | healthy            | execution error         | unhealthy  |
| `config validate` | valid           | invalid config          | —          |
| everything else | success           | error (message on stderr) | —        |

On error with `--json`, a JSON object `{"error": "..."}` is written to stderr
and the process exits `1`.

## Commands and JSON output

### `status`

```bash
devcli --json status [APP] [--project P] [--deps]
```

```json
{ "processes": [
  { "id": "proj.api.local", "app": "api", "project": "proj",
    "environment": "local", "command_variant": null, "pid": 1234,
    "running": true, "uptime_seconds": 42, "command": "npm run dev" } ] }
```

With `--deps APP`: `{ "project", "app", "running", "pid", "uptime_seconds",
"dependencies": [{ "project", "app", "running" }] }`.

### `start` / `restart`

```bash
devcli --json start api worker --project proj --env local --stage dev
```

`--json` (or `--detached`) starts processes in the background and prints:

```json
{ "started": ["api", "worker"], "environment": "local", "detached": true }
```

### `stop`

```bash
devcli --json stop api --project proj        # one app
devcli --json stop --all                     # everything
devcli --json stop --all --dry-run           # report only, no signals
```

```json
{ "dry_run": false, "stopped": 1, "failed": 0, "errors": [] }
```

### `logs`

Reads the most recent captured stdout/stderr file for an app
(`~/.devcli/logs/{project}_{app}_{env}_{date}.log`).

```bash
devcli --json logs api --project proj --env local -n 200
devcli logs api --follow                     # human streaming mode
```

```json
{ "project": "proj", "app": "api", "environment": "local",
  "file": "/Users/me/.devcli/logs/proj_api_local_20260101.log",
  "lines": ["...", "..."] }
```

`--follow` is for humans; it blocks and streams. Agents should poll `logs`
instead.

### `config`

```bash
devcli --json config list [--project P] [--apps-only]
devcli --json config show api --project P
devcli --json config validate
```

- `config list` → `{ "projects": [ { "name", "alternative_name", "apps": [...] } ] }`
  or `{ "apps": ["api", "worker"] }` with `--apps-only`.
- `config show` → the fully resolved app (type, path, defaults, dependencies,
  commands per environment, env files).
- `config validate` → `{ "valid", "errors": [{ "field", "message" }],
  "warnings": [...], "projects", "apps" }`.

### `health-check`

```bash
devcli --json health-check api --env local
```

```json
{ "project": "proj", "app": "api", "pid": 1234,
  "healthy": true, "duration_seconds": 0.03 }
```

### `metrics`

```bash
devcli --json metrics
```

Returns the raw monitor metrics document (process, system, performance).

### `auto-add` (non-interactive)

```bash
devcli --json auto-add --path ./apps/api --yes --project proj --name api --type nodejs
```

`--yes` (or any of `--project/--name/--type`) selects non-interactive mode:
detected defaults are accepted without prompting, and env files with an
ambiguous context are skipped. Without a TTY and without `--yes`, the command
fails with a clear message. Monorepos and multi-app directories require the
interactive mode.

```json
{ "project": "proj", "app": "api", "type": "nodejs", "path": "/abs/apps/api" }
```

## Environment

| Variable            | Effect                                                       |
| ------------------- | ------------------------------------------------------------ |
| `devcli_CONFIG_DIR` | Override the config directory (default `~/.devcli`).         |
| `DEVCLI_BIN`        | Binary devcli uses when spawning child processes.            |
| `NO_COLOR`          | Disable colored output.                                      |
| `LOG_LEVEL`         | `off` / `error` / `warn` / `info` / `debug` / `trace`.       |
