# AGENTS.md — Using devcli from AI agents

This document describes how an autonomous agent should drive `devcli`.
It is the contract for machine consumption: stable flags, exit codes, JSON
shapes, and non-interactive behaviour.

## Golden rules

1. **Always pass `--json`.** Every command that reports state has a
   machine-readable form. `--json` also implies *detached* mode for `start`
   (devcli returns immediately instead of streaming logs, and the apps keep
   running afterwards). Without it, a person's `start` stays attached and
   stops the apps it started when it is interrupted or exits; when devcli
   detects it runs under an AI agent, `start`, `restart` and `run` always
   return right away.
2. **Never rely on prompts.** Pass explicit flags (`--project`, `--name`,
   `--yes`, `--env`). When stdin is not a TTY, interactive commands fail
   fast with a clear error instead of hanging.
3. **Check the exit code**, not the text.
4. **Isolate state** with `DEVCLI_CONFIG_DIR` when testing.
5. **Act on the apps you work on, by name.** The user and other agents run
   apps too. When devcli detects it runs under an AI agent, it refuses what
   would reach their apps (exit code 1): `stop --all`, `stop --project P`
   without an app, and `start`/`stop` with `APP@*` or `--project P@*` (every
   git worktree). `--dry-run` and `status` are fine, and so is
   `stop --project P` run inside your own worktree, which only reaches your
   worktree's apps.
6. **In a git worktree, just run devcli from it.** `devcli start api` run
   inside a worktree starts *that worktree's* copy of the app, on its own
   ports, and `stop`/`status`/`logs` act on that copy only. See
   [Git worktrees](#git-worktrees).

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
and the process exits `1`. This includes command-line usage errors (unknown
subcommand or flag); without `--json`, those usage errors print clap's message
and exit `2`.

An app name that is neither configured nor tracked is an error (exit `1`) for
every command taking one, with a "did you mean" suggestion when a configured
name is close.

`--env` takes `local`, `docker`, `orbstack`, `docker-compose` (alias
`compose`), `k8s` (alias `kubernetes`) or `ci`; an alias is replaced by the
canonical name before anything runs (process ids, log files and JSON answers
always carry the canonical name), and any other value is a usage error.
`--stage` takes `dev`, `qa`, `preprod` or `prod`.

## Commands and JSON output

### `status`

```bash
devcli --json status [APP] [--project P] [--deps]
```

```json
{ "processes": [
  { "id": "proj.api.local", "app": "api", "project": "proj",
    "environment": "local", "command_variant": "start", "pid": 1234,
    "running": true, "uptime_seconds": 42, "command": "npm run dev",
    "worktree": null } ] }
```

`command_variant` is the name of the command that runs (the environment's
default, e.g. `"start"`, or the variant given to `run`). `worktree` is
`{ "slug", "path", "branch" }` for an instance running from a git worktree
(its `project` is then `proj@slug`).

With `--deps APP`: `{ "project", "app", "running", "pid", "uptime_seconds",
"dependencies": [{ "project", "app", "running" }] }`.

### `start` / `restart`

```bash
devcli --json start api worker --project proj --env local --stage dev
```

`--json` (or `--detached`) starts processes in the background and prints:

```json
{ "started": ["api", "worker"], "already_running": [], "environment": "local",
  "detached": true }
```

`started` lists what this command started; `already_running` the apps asked
for that were running already (nothing was started for them). An app started
as another's dependency appears in neither. `restart` answers
`{ "started", "environment", "detached" }` (no `already_running`) and fails
with exit code 1 when the app is not running.

A worktree instance is reported as `api@slug`.

### `stop`

```bash
devcli --json stop api --project proj        # one app
devcli --json stop --all --dry-run           # what runs; nothing is stopped
```

`stop --all`, `stop --project P` (no app) and `stop APP@*` reach what the
user and other agents run: under an AI agent they are refused with
`{"error": ...}` and exit code 1 (`--dry-run` is allowed, and so is
`stop --project P` inside your own worktree). Stop the apps you started, by
name.

```json
{ "dry_run": false, "stopped": 1, "failed": 0, "errors": [] }
```

When nothing matched a running process, the answer also has
`"message": "No processes to stop"` (exit code 0).

`--dry-run` answers with the same keys (`"dry_run": true`, nothing stopped)
plus `count` and `processes` (`[{ "app", "project", "pid", "command" }]`).

### `run`

```bash
devcli --json run api build --project proj --env local
```

Runs a named command variant in the background and returns after a short
check (`running: false` means it already finished; read `log_file`). Its
output goes to the app's log file for that environment, the same file as
`start` (read it with `logs`). Only one process per app and environment is
tracked: `run` fails with exit code 1 (`Process 'api:build' is already running
with PID ...`) when that app already runs in that environment, whatever its
command.

```json
{ "project": "proj", "app": "api", "environment": "local",
  "command_variant": "build", "pid": 1234, "running": true,
  "log_file": "/Users/me/.devcli/logs/proj_api_local_20260101.log" }
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
instead. With `--json`, `--follow` writes newline-delimited JSON: the document
above on one line, then one `{"line": "..."}` object per new line.

### `config`

```bash
devcli --json config list [--project P] [--apps-only]
devcli --json config show api --project P
devcli --json config validate
```

- `config list` → `{ "projects": [ { "name", "alternative_name", "apps": [...] } ] }`
  or `{ "apps": ["api", "worker"] }` with `--apps-only`.
- `config show` → the fully resolved app (type, path, port, health check,
  restart policy, defaults, dependencies,
  commands per environment, env files). `defaults` and `commands` have one key
  per environment (`local`, `docker`, `orbstack`, `docker-compose`, `k8s`,
  `ci`), `null` when not configured.
- `config list-commands APP [--env E]` → `{ "project", "app", "environments":
  { "<env>": { "default", "commands" } } }`.
- `config validate` → `{ "valid", "errors": [{ "field", "message" }],
  "warnings": [...], "projects", "apps" }`.
- `config add-command APP ENV NAME VALUE` → `{ "project", "app",
  "environment", "command", "value" }` (an existing command of that name is
  replaced).
- `config remove-command APP ENV NAME` → `{ "project", "app", "environment",
  "command", "default_cleared" }`.
- `config set-default APP ENV NAME` → `{ "project", "app", "environment",
  "default" }`.
- `config init` → `{ "created": "<path of config.json>" }`.
- `config edit` (opens `$VISUAL`/`$EDITOR`) → `{ "path", "editor" }`;
  `config edit-command` always prompts for the new value and is refused with
  `--json` (use `config add-command`).

Every argument these commands would prompt for must be given: with `--json`,
or without a terminal on stdin, a missing one is an error naming it
(`Missing the app name (APP_NAME): ...`).

### `env`

```bash
devcli --json env add APP --stage dev --context local --file .env.dev [--project P]
devcli --json env remove APP --stage dev [--context local] [--project P]
devcli --json env list APP [--project P]
devcli --json env set-default APP --context docker --stage qa [--project P]
```

The app is found like everywhere else (`--project`, `project/app`,
alternative name; an ambiguous name is an error). `--context` is an
environment (aliases accepted, stored canonical); `env add` writes only that
context.

- `env add` → `{ "project", "app", "stage", "context", "file" }`
- `env remove` → `{ "project", "app", "stage", "context", "removed":
  { "<context>": "<file>" } }` (`context` is `null` when the whole stage was
  removed)
- `env list` → `{ "project", "app", "env_files": { "<stage>": { "<context>":
  "<file>" } }, "default_stages": { "<context>": "<stage>" } }`
- `env set-default` → `{ "project", "app", "context", "stage" }`

### `pref`

```bash
devcli --json pref show
devcli --json pref set default-env docker
devcli --json pref reset
```

- `pref show` and `pref reset` → `{ "default-env", "detached-mode",
  "auto-start-deps", "docker-platform", "default-stage" }` (the keys `pref
  set` takes; `default-stage` is `null` when unset)
- `pref set KEY VALUE` → `{ "key", "value" }` with the value as saved
  (`compose` is saved as `docker-compose`)

### `health-check`

```bash
devcli --json health-check api --project proj --env local
```

```json
{ "project": "proj", "app": "api", "pid": 1234,
  "healthy": true, "duration_seconds": 0.03 }
```

When the check fails, `healthy` is `false`, a `failures` key gives the number
of consecutive failures, and the exit code is `2`. An app that is not running
is an error (exit code 1).

### `metrics`

```bash
devcli --json metrics
```

Returns the metrics document (process, system, performance). Works without a
running metrics server: process and system metrics are then computed from the
state (the performance counters stay at zero). Only `devcli monitor --daemon`
serves the metrics API; pm-daemon, which `start` spawns, does not.

### `auto-add` (non-interactive)

```bash
devcli --json auto-add --path ./apps/api --yes --project proj --name api --type nodejs
```

`--yes` (or any of `--project/--name/--type`, or `--json`) selects non-interactive mode:
detected defaults are accepted without prompting, and env files with an
ambiguous context are skipped. Without a TTY and without `--yes`, the command
fails with a clear message. Monorepos and multi-app directories require the
interactive mode. An app that already exists is not replaced unless `--force`
is given; what was set by hand is then kept: dependencies, port, health check,
restart policy, alternative name, default stages, and the commands, defaults
and env files the detection does not produce (detected ones are refreshed). `--type` must be one of `nodejs`, `nx`, `nx-workspace`, `rust`,
`python`, `docker`, `redis`, `traefik`; `path` in the answer is absolute.

```json
{ "project": "proj", "app": "api", "type": "nodejs", "path": "/abs/apps/api" }
```

## Git worktrees

An app is configured once, with the path of one checkout; devcli runs it
from **any git worktree of that repository**, wherever the worktree lives
(`.claude/worktrees/x`, `.worktrees/x`, a sibling directory, ...). Nothing
is added to the config. A worktree instance is named `project@slug` (slug:
the worktree's name in git), so its process id, state and log file never
collide with another checkout's.

| Name | Designates |
| ---- | ---------- |
| `api` | the worktree the command runs in, else the configured checkout |
| `api@slug` | that worktree, from anywhere |
| `api@` | the configured checkout, from anywhere |
| `api@*` | every checkout (`start`, `stop`, `status`) |

The suffix also works on `--project` (`--project shop@slug`). From inside a
worktree, `stop --project shop` only selects that worktree's processes.

Each instance gets its own ports and variables:

| Variable | Value |
| -------- | ----- |
| `DEVCLI_WORKTREE` | slug (empty in the configured checkout) |
| `DEVCLI_WORKTREE_INDEX` | 0 for the configured checkout, then 1, 2, ... (stable while the worktree exists) |
| `DEVCLI_PORT_OFFSET` | index × 100 |
| `DEVCLI_WORKTREE_PATH`, `DEVCLI_WORKTREE_BRANCH` | in a worktree |
| `PORT`, `DEVCLI_PORT` | the app's port + offset: its env files' `PORT`, else its configured `port` |
| `DEVCLI_PORT_<APP>` | the port of each dependency with a configured `port`, as it runs for this instance (shifted when it runs in the same worktree), e.g. `DEVCLI_PORT_API` |

Without a `port` in the config, a `PORT` from the app's env files is shifted
in worktrees. Health checks follow the shifted port. In a fresh worktree
without env files (they are usually gitignored), the configured checkout's
are used. Dependencies in the same repository start in the same worktree.
When a worktree directory is deleted, pm-daemon stops the apps that ran
from it.

### `worktrees`

```bash
devcli --json worktrees [--project P]
```

```json
{ "repositories": [ { "root": "/abs/repo", "common_dir": "/abs/repo/.git",
    "projects": ["shop"],
    "worktrees": [
      { "slug": null, "index": 0, "port_offset": 0, "path": "/abs/repo",
        "branch": "main", "configured": true, "current": false,
        "stale": false, "locked": false,
        "apps": [{ "project": "shop", "app": "api", "name": "api", "running": true }] },
      { "slug": "agent-1", "index": 1, "port_offset": 100,
        "path": "/abs/repo/.claude/worktrees/agent-1", "branch": "agent-1",
        "configured": false, "current": true, "stale": false, "locked": false,
        "apps": [{ "project": "shop@agent-1", "app": "api", "name": "api@agent-1",
                   "running": false }] } ] } ] }
```

`stale` worktrees were deleted without `git worktree remove`; devcli never
prunes them.

## Environment

| Variable            | Effect                                                       |
| ------------------- | ------------------------------------------------------------ |
| `DEVCLI_CONFIG_DIR` | Override the config directory (default `~/.devcli`). The former name `devcli_CONFIG_DIR` still works. |
| `DEVCLI_METRICS_ADDR` | Metrics API address (default `127.0.0.1:9090`), served only by `devcli monitor --daemon`. |
| `DEVCLI_AGENT`      | `1` (or any true value) tells devcli it runs under an AI agent when your harness is not recognised. |
| `NO_COLOR`          | Disable colored output.                                      |
| `LOG_LEVEL`         | `off` / `error` / `warn` / `info` / `debug` / `trace`.       |
