# Advanced Features

Deep dive into devcli's powerful features for production-ready process management.

## Table of Contents

- [Health Checks](#health-checks)
- [Restart Policies](#restart-policies)
- [Metrics & Monitoring](#metrics--monitoring)
- [Structured Logging](#structured-logging)
- [Dependency Management](#dependency-management)
- [Environment Management](#environment-management)
- [TUI Interface](#tui-interface)
- [Process Lifecycle](#process-lifecycle)

---

## Health Checks

Automatically monitor app health and trigger restarts when unhealthy. Checks
are run by `pm-daemon`, the background monitor that `start` spawns. A health
check is an object tagged by `type`: `http`, `tcp`, `command` or `process`.

### Types of Health Checks

#### HTTP Health Check

Sends HTTP requests to check if service is responding.

```json
{
  "health_check": {
    "type": "http",
    "url": "http://localhost:3000/health",
    "expected_status": 200,
    "timeout_secs": 5
  }
}
```

**Features**:
- Configurable URL and expected status (default 200; with 200, any 2xx passes)
- Timeout protection (`timeout_secs`, default 5)
- Follows redirects
- Supports HTTPS

**Best For**: Web services, APIs, frontends

#### TCP Health Check

Checks if TCP port is accepting connections.

```json
{
  "health_check": {
    "type": "tcp",
    "host": "localhost",
    "port": 5432,
    "timeout_secs": 10
  }
}
```

**Best For**: Databases, message queues, any TCP service

#### Command Health Check

Runs a custom command (with `sh -c`) to determine health.

```json
{
  "health_check": {
    "type": "command",
    "command": "curl -f http://localhost:3000/health",
    "timeout_secs": 5,
    "expected_exit_code": 0
  }
}
```

**Exit code `expected_exit_code`** (default 0) = healthy, anything else (or a
timeout) = unhealthy

**Best For**: Complex health logic, custom protocols

#### Process Health Check

Healthy as long as the process is alive. This is also what happens without a
`health_check`.

```json
{
  "health_check": {
    "type": "process"
  }
}
```

### Health Check Behavior

**Monitor Cycle**:
1. Monitor runs health checks every **3 seconds**, starting 30 seconds after
   each (re)start
2. Failure increments failure counter
3. Success resets failure counter to 0
4. After **3 consecutive failures**, restart is triggered (or, without an
   enabled `restart_policy`, the app is stopped)

**Example Timeline**:
```
00:30 - Check #1: ✓ healthy (failures: 0)
00:33 - Check #2: ✗ failed  (failures: 1)
00:36 - Check #3: ✗ failed  (failures: 2)
00:39 - Check #4: ✗ failed  (failures: 3) → RESTART TRIGGERED
```

### Health Check Thresholds

The interval (3 seconds), the start-up grace period (30 seconds) and the
threshold (3 failures) are fixed; they cannot be configured.

### Health Check Best Practices

#### 1. Implement Proper Health Endpoints

```javascript
// Express.js example
app.get('/health', (req, res) => {
  // Check database connection
  if (!db.isConnected()) {
    return res.status(503).json({ status: 'unhealthy', reason: 'db' });
  }

  // Check critical dependencies
  if (!redis.ping()) {
    return res.status(503).json({ status: 'unhealthy', reason: 'redis' });
  }

  res.json({ status: 'healthy' });
});
```

#### 2. Use Appropriate Timeouts

**Fast service**:
```json
{
  "health_check": {
    "type": "http",
    "url": "http://localhost:3000/health",
    "timeout_secs": 2
  }
}
```

**Slow service (database warmup)**:
```json
{
  "health_check": {
    "type": "tcp",
    "host": "localhost",
    "port": 5432,
    "timeout_secs": 30
  }
}
```

#### 3. Combine with Restart Policies

Without a `restart_policy`, an app that fails its health check 3 times in a
row is stopped, not restarted.

```json
{
  "health_check": {
    "type": "http",
    "url": "http://localhost:3000/health"
  },
  "restart_policy": {
    "max_restarts": 5,
    "restart_window_secs": 300
  }
}
```

---

## Restart Policies

Automatically restart crashed or unhealthy processes.

### Basic Configuration

Every field is optional; these are the defaults:

```json
{
  "restart_policy": {
    "enabled": true,
    "max_restarts": 5,
    "restart_window_secs": 300,
    "initial_backoff_secs": 1,
    "max_backoff_secs": 60,
    "backoff_multiplier": 2.0
  }
}
```

`restart_on_exit_codes` (e.g. `[1, 137]`) restricts restarts to those exit
codes; without it any non-zero exit code restarts and exit code 0 does not.
`max_restarts: 0` means unlimited. An app without `restart_policy` is never
restarted.

### Restart Triggers

**1. Process Crashes** (non-zero exit code, or one listed in
`restart_on_exit_codes`):
```
Process exited with code 1 → automatic restart
```

The exit code is only known for instances that the monitor daemon restarted
itself (it is their parent). When the first instance started by `devcli start`
exits, its code cannot be observed and any exit counts as a crash.

Health checks only start counting failures 30 seconds after each (re)start, so
slow-booting servers are not restarted while they come up.

**2. Health Check Failures** (3 consecutive):
```
Health check failed 3 times → automatic restart
```

### Exponential Backoff

Prevents restart loops by increasing delay:

With the defaults:

```
Restart 1: Wait 1 second
Restart 2: Wait 1 second
Restart 3: Wait 2 seconds
Restart 4: Wait 4 seconds
Restart 5: Wait 8 seconds
...
Capped at 60 seconds
```

**Formula**: `delay = initial_backoff_secs * backoff_multiplier^(max(n,1)-1)`,
where `n` is the number of restarts already done, capped at
`max_backoff_secs`. The restart count is kept for as long as the process is
tracked; a `devcli restart` or a new `start` resets it.

### Restart Window

Only restarts within the last `restart_window_secs` count towards
`max_restarts` (a sliding window):

```json
{
  "restart_policy": {
    "max_restarts": 3,
    "restart_window_secs": 60
  }
}
```

**Scenario**:
- 00:00 - Restart #1
- 00:20 - Restart #2
- 00:40 - Restart #3
- 01:10 - Crash: restart #1 is out of the window, 2 restarts in the last 60s
  → restarted
- 01:15 - Crash: 3 restarts in the last 60s (00:20, 00:40, 01:10) → no
  restart (see below)

### Max Restarts Reached

When max restarts is hit within window:

pm-daemon logs `Max restarts reached or policy prevents restart. Deleting
state.`, kills the process if it still runs (an unhealthy one) and stops
tracking it. Start it again with `devcli start`.

### Strategies by Service Type

**Web Service** (needs to be available):
```json
{
  "restart_policy": {
    "max_restarts": 10,
    "restart_window_secs": 600,
    "initial_backoff_secs": 3
  }
}
```

**Background Worker** (can tolerate downtime):
```json
{
  "restart_policy": {
    "max_restarts": 5,
    "restart_window_secs": 300,
    "initial_backoff_secs": 10,
    "max_backoff_secs": 120
  }
}
```

**Database** (critical, but restart is expensive):
```json
{
  "restart_policy": {
    "max_restarts": 3,
    "restart_window_secs": 600,
    "initial_backoff_secs": 30,
    "max_backoff_secs": 300
  }
}
```

**Development Service** (aggressive restart):
```json
{
  "restart_policy": {
    "max_restarts": 20,
    "restart_window_secs": 600,
    "initial_backoff_secs": 1,
    "backoff_multiplier": 1.0
  }
}
```

---

## Metrics & Monitoring

Real-time insights into process behavior and performance.

### Metrics Architecture

```
┌───────────────────────────────────────┐
│ devcli monitor --daemon               │
│ ┌─────────────────┐                   │
│ │ MetricsCollector│                   │
│ │  - Counters     │                   │
│ │  - Timings      │                   │
│ │  - Exit codes   │                   │
│ └────────┬────────┘                   │
│          │                             │
│  ┌───────▼────────┐                   │
│  │ HTTP API       │                   │
│  │ 127.0.0.1:9090 │                   │
│  └────────────────┘                   │
└───────────────────────────────────────┘
           │
           ├─── devcli metrics (CLI)
           └─── curl http://localhost:9090/metrics
```

The HTTP API is served only by `devcli monitor --daemon`. `pm-daemon`, the
monitor that `start` spawns automatically, has no HTTP server, and
`monitor --daemon` refuses to run while pm-daemon holds the monitor lock (it
also exits when no process is tracked). In everyday use, `devcli metrics`
therefore computes its figures from the process state.

### Viewing Metrics

**CLI (Formatted)**:
```bash
devcli metrics
```

**HTTP API (Raw JSON)**, served while `devcli monitor --daemon` runs:
```bash
curl http://localhost:9090/metrics | jq
```

Without a metrics server, `devcli metrics` computes the process and system
metrics from the state store itself; only the in-memory performance counters
need the server. The listen address can be changed with
`DEVCLI_METRICS_ADDR` (default `127.0.0.1:9090`). The API sends no CORS
headers, so web pages cannot read it.

`health_check_success_rate` is the share of running apps whose latest health
checks pass.

### Metrics Categories

#### Process Metrics

```json
{
  "processes": {
    "total_processes": 5,
    "running_processes": 3,
    "stopped_processes": 2,
    "total_restarts": 12,
    "restarts_last_hour": 3,
    "health_check_success_rate": 0.98,
    "exit_code_distribution": {
      "0": 8,
      "1": 3,
      "137": 1
    },
    "apps": [
      {
        "project": "awesome-project",
        "name": "api",
        "status": "running",
        "uptime_seconds": 3600,
        "restart_count": 2,
        "last_exit_code": null,
        "health_status": "healthy",
        "health_check_failures": 0
      }
    ]
  }
}
```

#### System Metrics

```json
{
  "system": {
    "monitor_uptime_seconds": 7200,
    "monitor_loop_iterations": 2400,
    "devcli_version": "0.6.1",
    "total_apps_configured": 12
  }
}
```

#### Performance Metrics

```json
{
  "performance": {
    "avg_startup_time_ms": 450.5,
    "avg_health_check_duration_ms": 42.3,
    "avg_restart_duration_ms": 1023.7,
    "recent_operations": [
      {
        "operation": "start",
        "app": "api",
        "duration_ms": 523,
        "timestamp": "2024-02-08T10:30:00Z",
        "success": true
      }
    ]
  }
}
```

### Metrics Use Cases

The examples below need the HTTP API, i.e. a running `devcli monitor
--daemon`; `devcli --json metrics` gives the same document without it.

**1. Monitoring Dashboards**:
```bash
# Feed to Prometheus/Grafana
curl -s http://localhost:9090/metrics | \
  jq '.processes.total_restarts' | \
  promtool push
```

**2. Alerting**:
```bash
# Alert if restart rate too high
RESTARTS=$(curl -s http://localhost:9090/metrics | jq '.processes.restarts_last_hour')
if [ "$RESTARTS" -gt 10 ]; then
  notify-send "High restart rate: $RESTARTS/hour"
fi
```

**3. Performance Analysis**:
```bash
# Track startup times
curl -s http://localhost:9090/metrics | \
  jq '.performance.avg_startup_time_ms'
```

**4. Health Monitoring**:
```bash
# Check overall health
HEALTH=$(curl -s http://localhost:9090/metrics | jq '.processes.health_check_success_rate')
echo "Health: $(echo "$HEALTH * 100" | bc)%"
```

---

## Structured Logging

Production-grade logging with JSON format and structured fields.

### Log Files

**Location**: `~/.devcli/logs/` (the `logs/` directory of the devcli
directory: `$DEVCLI_CONFIG_DIR`, else `./.devcli` when it exists, else
`~/.devcli`)

**Files**:
- `devcli.json.YYYY-MM-DD` - devcli's own structured JSON logs (one file per
  day; old files are not deleted)
- `<project>_<app>_<env>_<YYYYMMDD>.log` - each app's raw stdout/stderr, as the
  app prints it (no timestamps added); rotated to `<name>.log.1` past 20 MiB

### JSON Log Format

One JSON object per line:

```json
{
  "timestamp": "2026-10-30T18:16:28.075762Z",
  "level": "INFO",
  "fields": {
    "message": "Starting app process",
    "app": "worker",
    "environment": "local",
    "command": "npm run worker"
  },
  "target": "devcli_core::commands::start::executor",
  "span": {
    "name": "start_single_app_process",
    "project": "awesome-project"
  },
  "spans": [
    { "name": "start_command", "app_names": "[\"api\"]" }
  ]
}
```

### Log Levels

Set via the `LOG_LEVEL` environment variable (`off`, `error`, `warn`,
`info`, `debug`, `trace`); it applies to the console (stderr) and to the log
file. `RUST_LOG` is not read, and there are no per-module filters.

```bash
# Info level (default)
LOG_LEVEL=info devcli start api

# Debug level
LOG_LEVEL=debug devcli start api

# Trace level (very verbose)
LOG_LEVEL=trace devcli start api

# Nothing at all
LOG_LEVEL=off devcli start api
```

### Querying Logs with jq

**All errors**:
```bash
cat ~/.devcli/logs/devcli.json.* | \
  jq 'select(.level == "ERROR")'
```

**Specific app**:
```bash
cat ~/.devcli/logs/devcli.json.* | \
  jq 'select(.fields.app_name == "api")'
```

**Slow operations** (> 1 second):
```bash
cat ~/.devcli/logs/devcli.json.* | \
  jq 'select(.fields.duration_ms > 1000)'
```

**Time range**:
```bash
cat ~/.devcli/logs/devcli.json.2024-02-08 | \
  jq 'select(.timestamp > "2024-02-08T10:00:00Z")'
```

**Span hierarchy**:
```bash
cat ~/.devcli/logs/devcli.json.* | \
  jq '.span.name' | sort | uniq
```

### Structured Fields

Common fields in logs:

- `app_name` - Application name
- `project` - Project name
- `environment` - Execution environment
- `pid` - Process ID
- `exit_code` - Exit code (when process exits)
- `duration_ms` - Operation duration
- `status` - HTTP status code
- `url` - HTTP URL
- `failures` - Health check failure count

---

## Dependency Management

Automatic dependency resolution and ordered startup.

### Defining Dependencies

Each dependency names its project and app:

```json
{
  "frontend": {
    "dependencies": [
      { "project": "shop", "app": "api" }
    ]
  },
  "api": {
    "dependencies": [
      { "project": "shop", "app": "database" },
      { "project": "shop", "app": "redis" }
    ]
  }
}
```

### Dependency Resolution

**Topological Sort** ensures correct order:

```bash
devcli start frontend
```

**Start Order**:
1. database
2. redis
3. api
4. frontend

Missing dependencies start in waves (database and redis together, then api),
and each wave is waited for until its health checks pass, 30 seconds at most,
before the next wave or the app asked for starts. A dependency without a
health check is only waited for until it is spawned.

### Transitive Dependencies

Dependencies of dependencies are automatically included:

```
frontend → api → database
frontend → api → redis
```

**Result**: Starting `frontend` also starts `api`, `database`, and `redis`.

### Circular Dependency Detection

```json
{
  "api": {
    "dependencies": [
      { "project": "shop", "app": "worker" }
    ]
  },
  "worker": {
    "dependencies": [
      { "project": "shop", "app": "api" }
    ]
  }
}
```

```bash
devcli start api
# Error: Circular dependency detected: shop/api -> shop/worker -> shop/api. Check app dependencies in config.
```

`devcli config validate` reports the same error.

### Skipping Dependencies

```bash
# Start without dependencies
devcli start api --skip-deps
```

**Use Cases**:
- Dependencies already running
- Testing in isolation
- Manual dependency management

### Partial Dependency Graphs

```json
{
  "service-a": {
    "dependencies": [
      { "project": "shop", "app": "database" }
    ]
  },
  "service-b": {
    "dependencies": [
      { "project": "shop", "app": "database" },
      { "project": "shop", "app": "redis" }
    ]
  },
  "service-c": {
    "dependencies": [
      { "project": "shop", "app": "service-a" },
      { "project": "shop", "app": "service-b" }
    ]
  }
}
```

**Starting `service-c`**:
1. database, redis (parallel)
2. service-a, service-b (parallel)
3. service-c

---

## Environment Management

Manage environment-specific configuration across dev/qa/prod.

### Stage Concept

A **stage** represents a deployment environment. Only these four exist (any
other `--stage`, `env add --stage` or `default-stage` value is refused):
- `dev` - Local development
- `qa` - QA environment
- `preprod` - Pre-production
- `prod` - Production

In `env_files`, the key `base` holds files used for every stage
(`devcli env add APP --stage base ...`).

### Env File Mapping

```json
{
  "env_files": {
    "dev": {
      "local": ".env.dev",
      "docker": ".env.docker.dev"
    },
    "qa": {
      "docker": ".env.qa",
      "k8s": "k8s/qa/.env"
    },
    "prod": {
      "k8s": "k8s/prod/.env"
    }
  }
}
```

The keys under a stage are environment names (`local`, `docker`, `orbstack`,
`docker-compose`, `k8s`, `ci`). Without `--stage`, the app's
`default_stages` for that environment, then the `default-stage` preference,
pick the stage.

### Usage

```bash
# Use dev stage (local context)
devcli start api --stage dev

# Use qa stage (docker context)
devcli start api --stage qa --env docker

# Use prod stage (k8s context)
devcli start api --stage prod --env k8s
```

### Docker Integration

In the `docker`, `orbstack` and `docker-compose` environments, the env file is
added to `docker run` / `docker create` commands as `--env-file` (other
commands, such as `docker compose up`, are left as they are). In `local`,
`k8s` and `ci`, its variables are set in the command's environment.

```json
{
  "commands": {
    "docker": {
      "start": "docker run --rm my-api"
    }
  },
  "env_files": {
    "dev": {
      "docker": ".env.dev"
    }
  }
}
```

**Becomes** (with `--stage dev`):
```bash
docker run --platform linux/amd64 --rm --env-file .env.dev my-api
```

### Best Practices

**1. Separate env files by stage**:
```
.env.dev       # Local development
.env.qa        # QA
.env.preprod   # Pre-production
.env.prod      # Production (never commit!)
```

**2. Use .gitignore**:
```
.env.prod
.env.*.local
*.secret
```

**3. Document required variables**:
```bash
# .env.example
DATABASE_URL=
API_KEY=
SECRET_KEY=
```

---

## TUI Interface

Terminal UI to watch and drive your apps: status, commands and logs.

### Launching TUI

```bash
devcli ui
```

### Features

- **Status, Commands and Logs tabs** (`1`, `2`, `3`)
- **Start, stop and restart** apps (`Enter`, `s`, `r` in the Status tab)
- **Run commands** of an app (Commands tab)
- **Real-time log streaming**, with search (`/`, then `n`/`N`)
- **ANSI color support** for colored logs
- Worktree instances listed under their project

### Keyboard Shortcuts

- `↑/↓` or `k/j` - Move selection
- `Tab` - Switch panel focus
- `Space` - Expand/collapse a project
- `Enter` - Select / execute / open the log viewer
- In the log viewer: `↑/↓`, `PgUp/PgDn`, `Home/End` or `g/G`, `/` search,
  `n/N` next/previous match, `Esc` to leave
- `Esc` - Back / cancel / close popup
- `Ctrl+C` or `q` - Quit
- `?` - Show help

---

## Process Lifecycle

Understanding how devcli manages processes.

### State Tracking

**Files**: `~/.devcli/processes/<project>.<app>.<env>.json`, one per tracked
process (e.g. `awesome-project.api.local.json`)

Each file holds the PID and process group, the command, working directory and
environment devcli set, the start time, the restart count and history, the
last exit code, and the project/app/environment metadata, with the health
check and restart policy taken from the config. Inspect them with
`devcli --json status` rather than reading them.

### Spawner Architecture

```
devcli start api
      │
      ├─ Spawn the app in its own session (setsid)
      │      └─ stdout/stderr = the log file itself (append mode)
      │
      ├─ Record it in ~/.devcli/processes/<id>.json
      ├─ Make sure pm-daemon is running (health checks, auto-restart)
      │
      └─ Detached: return immediately; attached (the default for a person):
         stream the output and stop the apps on Ctrl+C
```

- The app writes its output straight into its log file, so it keeps running
  and logging after the CLI (or the TUI) exits.
- Logs rotate by size (20 MiB): the file is copied to `<name>.log.1` and
  truncated in place.
- pm-daemon is the parent of the instances it restarts, so their exit codes
  are recorded; the exit code of the first instance is not observable.

### Monitor Lifecycle

**Auto-start**:
```bash
devcli start api
# → Spawns pm-daemon if it is not running
```

pm-daemon must be installed next to `devcli` or on the `PATH`; without it,
apps still run, but nothing checks their health or restarts them.

**Auto-exit**: pm-daemon exits on its own when no process is tracked any more
(it logs `No processes remaining, stopping monitor`).

**Manual control**:
```bash
# Check if running
ps aux | grep pm-daemon

# Stop every app (pm-daemon then exits by itself)
devcli stop --all
```

### Cleanup

**Dead process cleanup**:
```bash
# Manual cleanup
devcli monitor

# Output:
# Cleaned up 2 dead process(es):
#   - old-api
#   - crashed-worker
```

**Automatic cleanup**:
- Commands such as `status`, `start` and `stop` drop the entries of dead
  processes before they run
- pm-daemon checks every tracked process on each cycle, and restarts or
  forgets the dead ones
- Entries live in `~/.devcli/processes/`

---

## See Also

- [Configuration Reference](./configuration-reference.md) - Config file format
- [Commands Reference](./commands-reference.md) - All CLI commands
- [Troubleshooting](./troubleshooting.md) - Common issues
