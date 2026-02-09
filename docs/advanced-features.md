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

Automatically monitor app health and trigger restarts when unhealthy.

### Types of Health Checks

#### HTTP Health Check

Sends HTTP requests to check if service is responding.

```yaml
health_check:
  http:
    url: http://localhost:3000/health
    expected_status: 200
    timeout_secs: 5
```

**Features**:
- Configurable URL and expected status
- Timeout protection
- Follows redirects (up to 5)
- Supports HTTPS

**Best For**: Web services, APIs, frontends

#### TCP Health Check

Checks if TCP port is accepting connections.

```yaml
health_check:
  tcp:
    host: localhost
    port: 5432
    timeout_secs: 10
```

**Best For**: Databases, message queues, any TCP service

#### Command Health Check

Runs custom command to determine health.

```yaml
health_check:
  command:
    cmd: curl -f http://localhost:3000/health
    timeout_secs: 5
```

**Exit code 0** = healthy, **non-zero** = unhealthy

**Best For**: Complex health logic, custom protocols

### Health Check Behavior

**Monitor Cycle**:
1. Monitor runs health checks every **3 seconds**
2. Failure increments failure counter
3. Success resets failure counter to 0
4. After **3 consecutive failures**, restart is triggered

**Example Timeline**:
```
00:00 - Check #1: ✓ healthy (failures: 0)
00:03 - Check #2: ✗ failed  (failures: 1)
00:06 - Check #3: ✗ failed  (failures: 2)
00:09 - Check #4: ✗ failed  (failures: 3) → RESTART TRIGGERED
```

### Configuring Health Check Thresholds

Currently hardcoded to 3 failures. To customize (future feature):

```yaml
health_check:
  http:
    url: http://localhost:3000/health
  failure_threshold: 5  # Allow 5 failures before restart
  check_interval_secs: 10  # Check every 10 seconds
```

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

```yaml
# Fast service
health_check:
  http:
    timeout_secs: 2

# Slow service (database warmup)
health_check:
  tcp:
    timeout_secs: 30
```

#### 3. Combine with Restart Policies

```yaml
health_check:
  http:
    url: http://localhost:3000/health
restart_policy:
  max_restarts: 5
  restart_window_secs: 300
```

---

## Restart Policies

Automatically restart crashed or unhealthy processes.

### Basic Configuration

```yaml
restart_policy:
  max_restarts: 5           # Maximum restart attempts
  restart_window_secs: 300  # Within this time window (5 minutes)
  backoff_secs: 5           # Initial backoff delay
```

### Restart Triggers

**1. Process Crashes** (non-zero exit code):
```
Process exited with code 1 → automatic restart
```

**2. Health Check Failures** (3 consecutive):
```
Health check failed 3 times → automatic restart
```

### Exponential Backoff

Prevents restart loops by increasing delay:

```
Attempt 1: Wait 5 seconds
Attempt 2: Wait 10 seconds
Attempt 3: Wait 20 seconds
Attempt 4: Wait 40 seconds
Attempt 5: Wait 80 seconds
```

**Formula**: `delay = backoff_secs * 2^(attempt-1)`

**Max backoff**: Capped at 300 seconds (5 minutes)

### Restart Window

Restart counter resets after the window expires:

```yaml
restart_policy:
  max_restarts: 3
  restart_window_secs: 60  # 1 minute window
```

**Scenario**:
- 00:00 - Restart #1
- 00:20 - Restart #2
- 00:40 - Restart #3
- 01:10 - Window expired, counter resets to 0
- 01:15 - Restart #1 (new window starts)

### Max Restarts Reached

When max restarts is hit within window:

```
Monitor: Max restarts (5) reached for api within 300s window
Monitor: Giving up on restarting api
```

Process remains stopped until manually restarted.

### Strategies by Service Type

**Web Service** (needs to be available):
```yaml
restart_policy:
  max_restarts: 10
  restart_window_secs: 600  # 10 minutes
  backoff_secs: 3
```

**Background Worker** (can tolerate downtime):
```yaml
restart_policy:
  max_restarts: 5
  restart_window_secs: 300
  backoff_secs: 10
```

**Database** (critical, but restart is expensive):
```yaml
restart_policy:
  max_restarts: 3
  restart_window_secs: 600
  backoff_secs: 30
```

**Development Service** (aggressive restart):
```yaml
restart_policy:
  max_restarts: 20
  restart_window_secs: 600
  backoff_secs: 1
```

---

## Metrics & Monitoring

Real-time insights into process behavior and performance.

### Metrics Architecture

```
┌───────────────────────────────────────┐
│ Monitor Daemon                        │
│ ┌─────────────────┐                   │
│ │ MetricsCollector│                   │
│ │  - Counters     │                   │
│ │  - Timings      │                   │
│ │  - Exit codes   │                   │
│ └────────┬────────┘                   │
│          │                             │
│  ┌───────▼────────┐                   │
│  │ HTTP API       │                   │
│  │ localhost:9090 │                   │
│  └────────────────┘                   │
└───────────────────────────────────────┘
           │
           ├─── devcli metrics (CLI)
           └─── curl http://localhost:9090/metrics
```

### Viewing Metrics

**CLI (Formatted)**:
```bash
devcli metrics
```

**HTTP API (Raw JSON)**:
```bash
curl http://localhost:9090/metrics | jq
```

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
        "health_status": "healthy"
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
    "devcli_version": "0.1.0",
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

**Location**: `~/.devcli/logs/`

**Files**:
- `devcli.YYYY-MM-DD.json` - Structured JSON logs (daily rotation)
- `<project>_<app>_<date>.log` - App-specific logs (legacy)

### JSON Log Format

```json
{
  "timestamp": "2024-02-08T10:30:15.123Z",
  "level": "INFO",
  "fields": {
    "app_name": "api",
    "project": "awesome-project",
    "environment": "docker",
    "duration_ms": 523
  },
  "target": "devcli_core::commands::start",
  "span": {
    "name": "start_command",
    "app_count": 1
  },
  "message": "Starting application"
}
```

### Log Levels

Set via `RUST_LOG` environment variable:

```bash
# Info level (default)
RUST_LOG=info devcli start api

# Debug level
RUST_LOG=debug devcli start api

# Trace level (very verbose)
RUST_LOG=trace devcli start api

# Module-specific
RUST_LOG=devcli_core::commands::start=debug devcli start api
```

### Querying Logs with jq

**All errors**:
```bash
cat ~/.devcli/logs/devcli.*.json | \
  jq 'select(.level == "ERROR")'
```

**Specific app**:
```bash
cat ~/.devcli/logs/devcli.*.json | \
  jq 'select(.fields.app_name == "api")'
```

**Slow operations** (> 1 second):
```bash
cat ~/.devcli/logs/devcli.*.json | \
  jq 'select(.fields.duration_ms > 1000)'
```

**Time range**:
```bash
cat ~/.devcli/logs/devcli.2024-02-08.json | \
  jq 'select(.timestamp > "2024-02-08T10:00:00Z")'
```

**Span hierarchy**:
```bash
cat ~/.devcli/logs/devcli.*.json | \
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

```yaml
frontend:
  dependencies:
    - api

api:
  dependencies:
    - database
    - redis
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

### Transitive Dependencies

Dependencies of dependencies are automatically included:

```
frontend → api → database
frontend → api → redis
```

**Result**: Starting `frontend` also starts `api`, `database`, and `redis`.

### Circular Dependency Detection

```yaml
# INVALID: circular dependency
api:
  dependencies: [worker]

worker:
  dependencies: [api]
```

```bash
devcli start api
# Error: Circular dependency detected: api → worker → api
```

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

```yaml
service-a:
  dependencies: [database]

service-b:
  dependencies: [database, redis]

service-c:
  dependencies: [service-a, service-b]
```

**Starting `service-c`**:
1. database
2. redis
3. service-a, service-b (parallel)
4. service-c

---

## Environment Management

Manage environment-specific configuration across dev/qa/prod.

### Stage Concept

A **stage** represents a deployment environment:
- `dev` - Local development
- `qa` - QA environment
- `staging` - Pre-production
- `prod` - Production

### Env File Mapping

```yaml
env_files:
  dev:
    local: .env.dev
    docker: .env.docker.dev
  qa:
    docker: .env.qa
    k8s: k8s/qa/env.yaml
  prod:
    k8s: k8s/prod/env.yaml
```

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

Env files automatically injected:

```yaml
commands:
  docker:
    start: docker compose up api

env_files:
  dev:
    docker: .env.dev
```

**Becomes**:
```bash
docker compose --env-file .env.dev up api
```

### Best Practices

**1. Separate env files by stage**:
```
.env.dev       # Local development
.env.qa        # QA
.env.staging   # Staging
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

Beautiful terminal UI for viewing logs in real-time.

### Launching TUI

```bash
devcli ui
```

### Features

- **Real-time log streaming**
- **Syntax highlighting** for code and JSON
- **Multi-pane view** for multiple apps
- **ANSI color support** for colored logs
- **Automatic scrolling** to latest logs
- **Search and filter** (coming soon)

### Keyboard Shortcuts

- `↑/↓` or `j/k` - Scroll up/down
- `Page Up/Page Down` - Page scroll
- `Home/End` or `g/G` - Jump to top/bottom
- `Tab` - Switch between apps
- `Ctrl+C` or `q` - Quit
- `h` or `?` - Show help

### Log Filtering

Currently shows logs for all running processes. Future: filter by project or app.

---

## Process Lifecycle

Understanding how devcli manages processes.

### State Tracking

**File**: `~/.devcli/state.json`

Tracks all managed processes:

```json
{
  "processes": [
    {
      "app_name": "api",
      "pid": 12345,
      "project": "awesome-project",
      "environment": "local",
      "start_time": "2024-02-08T10:00:00Z",
      "restart_count": 2,
      "last_exit_code": null
    }
  ]
}
```

### Spawner Architecture

```
devcli start api
      │
      ├─ Spawn internal-spawner (detached)
      │      │
      │      └─ Spawn actual app process
      │             │
      │             └─ Capture stdout/stderr
      │             └─ Write to log file
      │
      └─ Return immediately (non-blocking)
```

**Internal Spawner**:
- Runs as separate process
- Captures app output
- Writes to log files
- Reports exit codes
- Survives parent process exit

### Monitor Lifecycle

**Auto-start**:
```bash
devcli start api
# → Starts monitor daemon automatically if not running
```

**Auto-exit**:
```
Monitor: No processes remaining
Monitor: Exiting
```

**Manual control**:
```bash
# Check if running
ps aux | grep "devcli monitor --daemon"

# Stop monitor (stops all apps)
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
- Monitor runs cleanup on each cycle
- Removes processes with no PID
- Updates state.json

---

## See Also

- [Configuration Reference](./configuration-reference.md) - Config file format
- [Commands Reference](./commands-reference.md) - All CLI commands
- [Troubleshooting](./troubleshooting.md) - Common issues
