# Troubleshooting Guide

Solutions to common issues and debugging tips for devcli.

## Table of Contents

- [Installation Issues](#installation-issues)
- [Configuration Errors](#configuration-errors)
- [Process Management](#process-management)
- [Health Checks](#health-checks)
- [Dependencies](#dependencies)
- [Logging & Metrics](#logging--metrics)
- [Environment Issues](#environment-issues)
- [Performance Problems](#performance-problems)
- [Debugging Tips](#debugging-tips)

---

## Installation Issues

### Binary Not Found After Installation

**Problem**: `command not found: devcli`

**Solution**:
```bash
# Check if binary exists
ls -l /usr/local/bin/devcli

# Make sure /usr/local/bin is in PATH
echo $PATH | grep "/usr/local/bin"

# Add to PATH if missing (add to ~/.zshrc or ~/.bashrc)
export PATH="/usr/local/bin:$PATH"

# Reload shell
source ~/.zshrc
```

### Permission Denied

**Problem**: `Permission denied` when running devcli

**Solution**:
```bash
# Make binary executable
chmod +x /usr/local/bin/devcli

# Verify permissions
ls -l /usr/local/bin/devcli
# Should show: -rwxr-xr-x
```

### macOS Gatekeeper Warning

**Problem**: "devcli cannot be opened because the developer cannot be verified"

**Solution**:
```bash
# Remove quarantine attribute
xattr -d com.apple.quarantine /usr/local/bin/devcli

# Or allow in System Preferences:
# System Preferences → Security & Privacy → General → Allow
```

---

## Configuration Errors

### Config File Not Found

**Problem**: `Config file not found at ~/.devcli/config.json. Run 'devcli config init' to create one.`

**Solution**:
```bash
# Initialize config
devcli config init

# Verify location
ls -l ~/.devcli/config.json

# Check DEVCLI_CONFIG_DIR override
echo $DEVCLI_CONFIG_DIR

# A ./.devcli directory in the current directory is used instead of ~/.devcli
ls -d .devcli
```

### Invalid JSON Syntax

**Problem**: `Failed to load configuration: Failed to parse JSON from ~/.devcli/config.json: ...`

**Solution**:
```bash
# Validate JSON syntax
devcli config validate

# Use an online JSON validator
cat ~/.devcli/config.json | pbcopy
# Paste into https://jsonlint.com/

# Common issues:
# - Unmatched braces or brackets
# - Trailing commas (not allowed in JSON)
# - Unquoted object keys or single quotes
```

**Example Fix**:
```json
{
  "commands": {
    "local": {
      "start": "npm start"
    }
  }
}
```

> **Note**: JSON does not allow trailing commas or comments. Whitespace is flexible, but validate with an editor to catch mistakes early.

### App Not Found

**Problem**: `App 'api' not found in config` (`start`, `stop`, `status` and
`run` add `Did you mean '...'?` when a configured name is close)

**Solution**:
```bash
# List all apps
devcli config list

# Check project name
devcli config list --project myproject

# Add --project if ambiguous
devcli start api --project myproject

# Verify config structure
devcli config show api
```

### Circular Dependency Error

**Problem**: `Circular dependency detected: shop/api -> shop/worker -> shop/api. Check app dependencies in config.`

**Solution**:
**WRONG**:
```json
{
  "api": { "dependencies": [{ "project": "shop", "app": "worker" }] },
  "worker": { "dependencies": [{ "project": "shop", "app": "api" }] }
}
```

**CORRECT**:
```json
{
  "api": { "dependencies": [{ "project": "shop", "app": "database" }] },
  "worker": { "dependencies": [{ "project": "shop", "app": "api" }] }
}
```

### Invalid Dependency Format

**Problem**: `invalid type: string "database", expected struct Dependency`

**Solution**: a dependency is an object naming the project and the app, not a
plain string:

```json
{
  "dependencies": [{ "project": "shop", "app": "database" }]
}
```

### Invalid Health Check Format

**Problem**: the config fails to load with `missing field `type`` (or
`unknown variant`) at a `health_check`

**Solution**: a health check is tagged by `type` (`http`, `tcp`, `command`,
`process`), with its fields next to it:

```json
{
  "health_check": {
    "type": "http",
    "url": "http://localhost:3000/health"
  }
}
```

---

## Process Management

### Process Won't Start

**Problem**: App starts but immediately exits

**Diagnosis**:
```bash
# Check the app's output
devcli logs app -n 100
cat ~/.devcli/logs/project_app_*.log

# Check devcli's JSON logs for errors
cat ~/.devcli/logs/devcli.json.* | jq 'select(.level == "ERROR")'

# Run command manually to see error
cd ~/path/to/app
npm start  # or whatever the command is
```

**Common Causes**:
1. **Missing dependencies**: `npm install`, `cargo build`, etc.
2. **Port already in use**: Check with `lsof -i :3000`
3. **Environment variables missing**: Check `.env` file
4. **Permissions**: Check file/directory permissions
5. **Shell syntax in the command** (`Failed to spawn process`): commands run
   without a shell, so `&&`, pipes and `VAR=value` prefixes fail; use
   `sh -c '...'`

**Solutions**:
```bash
# Install dependencies
cd ~/path/to/app
npm install

# Find and kill process on port
lsof -i :3000
kill -9 <PID>

# Check environment
devcli env list api
```

### Process Not Stopping

**Problem**: `devcli stop api` doesn't stop the process

**Diagnosis**:
```bash
# Check if process is actually running
devcli status

# Check system processes
ps aux | grep "my-app"

# Try force kill
devcli stop api --force
```

**Solution**:
```bash
# Force kill
devcli stop api --force

# Manual cleanup if needed (the PID shown by devcli status)
kill -9 <PID>

# Clean up state
devcli monitor
```

### Monitor Daemon Not Starting

**Problem**: no health checks or automatic restarts happen

The monitor is `pm-daemon`, spawned by `start` and `run`. It must be
installed next to the `devcli` binary or be on the `PATH`; without it, apps
still run but nothing watches them. It exits by itself when no process is
tracked.

**Diagnosis**:
```bash
# Check if pm-daemon is running
ps aux | grep pm-daemon

# Check it is installed
ls ~/.devcli/bin/
which pm-daemon

# Check its lock file
ls -l ~/.devcli/processes/.daemon.lock
```

**Solution**:
```bash
# Reinstall devcli with pm-daemon (see Getting Started), then start apps
devcli start api
```

### Lost Process State

**Problem**: `devcli status` shows no processes but apps are running

**Solution**:
```bash
# Find running processes
ps aux | grep node  # or your runtime

# Check which config devcli reads: status only lists apps of that config
devcli --json config list --apps-only
echo $DEVCLI_CONFIG_DIR

# Clean up state
devcli monitor

# Stop the stray process by its PID
kill <PID>

# Restart properly
devcli start api
```

---

## Health Checks

### Health Check Always Failing

**Problem**: Health check marks app as unhealthy but it's working

**Diagnosis**:
```bash
# Run health check manually
devcli health-check api

# Test endpoint directly
curl -v http://localhost:3000/health

# Check timeout
# Default is 5 seconds - might be too short
```

**Solutions**:

**1. Increase timeout**:
```json
{
  "health_check": {
    "type": "http",
    "url": "http://localhost:3000/health",
    "timeout_secs": 10
  }
}
```

**2. Check expected status** (with 200, any 2xx passes; set the exact code
otherwise):
```json
{
  "health_check": {
    "type": "http",
    "url": "http://localhost:3000/health",
    "expected_status": 204
  }
}
```

**3. Verify endpoint**:
```bash
# Should return 200 OK
curl -I http://localhost:3000/health
```

### TCP Health Check Failing

**Problem**: TCP health check fails but service is running

**Diagnosis**:
```bash
# Test TCP connection
nc -zv localhost 5432

# Check if port is open
lsof -i :5432

# Verify host/port in config
devcli config show database
```

**Solutions**:
**Correct host (use `localhost`, not `0.0.0.0`)**:
```json
{
  "health_check": { "type": "tcp", "host": "localhost", "port": 5432 }
}
```

**Increase timeout for slow services**:
```json
{
  "health_check": { "type": "tcp", "host": "localhost", "port": 5432, "timeout_secs": 30 }
}
```

### Health Checks Causing Too Many Restarts

**Problem**: App restarts constantly due to health checks

**Solution**:
```bash
# Temporarily disable health checks
# (edit config, remove health_check section)

# The threshold (3 failures), interval (3s) and start-up grace period (30s)
# are fixed; give a slow check a longer timeout_secs instead

# Check if app is actually healthy
curl http://localhost:3000/health

# Verify health endpoint doesn't have side effects
# (should be idempotent, read-only)
```

---

## Dependencies

### Dependencies Not Starting

**Problem**: `devcli start frontend` doesn't start dependencies

**Diagnosis**:
```bash
# Check dependencies in config
devcli config show frontend

# Check preference
devcli pref show | grep auto-start-deps
```

**Solutions**:
```bash
# Enable auto-start
devcli pref set auto-start-deps true

# Or start with --skip-deps and manually start
devcli start database
devcli start api
devcli start frontend --skip-deps
```

### Dependency Not Found

**Problem**: `Invalid dependency my-project/database: App 'database' not found in project 'my-project'.`

**Solution**: name an app that exists, with its project:
```json
{
  "api": { "dependencies": [{ "project": "my-project", "app": "database" }] }
}
```

```bash
# Check the app exists
devcli config list
```

### Wrong Startup Order

**Problem**: Dependencies start in wrong order

**Diagnosis**:
```bash
# Check the dependencies of an app
devcli config show frontend

# And whether they run
devcli status frontend --deps
```

**Solution**:
```json
{
  "database": {
    "dependencies": []
  },
  "redis": {
    "dependencies": []
  },
  "api": {
    "dependencies": [
      { "project": "shop", "app": "database" },
      { "project": "shop", "app": "redis" }
    ]
  },
  "frontend": {
    "dependencies": [
      { "project": "shop", "app": "api" }
    ]
  }
}
```

---

## Logging & Metrics

### Logs Not Appearing

**Problem**: Can't find logs for started app

**Diagnosis**:
```bash
# Check log directory
ls -lh ~/.devcli/logs/

# Check app was actually started
devcli status

# Check for errors in JSON logs
cat ~/.devcli/logs/devcli.json.* | \
  jq 'select(.level == "ERROR")'
```

**Solution**:
```bash
# Logs are at:
# ~/.devcli/logs/<project>_<app>_<env>_<YYYYMMDD>.log
devcli logs app

# Find by date
ls ~/.devcli/logs/*$(date +%Y%m%d)*

# View in TUI
devcli ui
```

### Metrics API Not Responding

**Problem**: `curl http://localhost:9090/metrics` fails

The metrics API is served only by `devcli monitor --daemon`. `pm-daemon`,
which `start` spawns, has no HTTP server, and `monitor --daemon` refuses to run
while pm-daemon holds the monitor lock (`A monitor (pm-daemon) is already
running for ...`).

**Diagnosis**:
```bash
# Is pm-daemon (no HTTP API) or devcli monitor --daemon running?
ps aux | grep -E "pm-daemon|devcli monitor"

# Check if port 9090 is in use
lsof -i :9090

# Check metrics server logs
cat ~/.devcli/logs/devcli.json.* | \
  jq 'select(.target | contains("metrics"))'
```

**Solutions**:
```bash
# Usually you do not need the API: this computes the same document
devcli --json metrics

# Port conflict: choose another address for monitor --daemon
DEVCLI_METRICS_ADDR=127.0.0.1:9191 devcli monitor --daemon
```

### Too Many Log Files

**Problem**: `~/.devcli/logs/` directory is huge

**Solution**:
```bash
# Remove old logs (older than 30 days)
find ~/.devcli/logs -name "*.log" -mtime +30 -delete
find ~/.devcli/logs -name "devcli.json.*" -mtime +30 -delete

# Or manually clean up
rm ~/.devcli/logs/*_202401*.log

# Set up cron job for automatic cleanup
# crontab -e
# 0 0 * * * find ~/.devcli/logs -mtime +30 -delete
```

---

## Environment Issues

### Docker Commands Failing

**Problem**: Docker commands fail with permission errors

**Solution**:
```bash
# Add user to docker group
sudo usermod -aG docker $USER

# Logout and login again
# Or run: newgrp docker

# Verify
docker ps
```

### Docker Compose Not Found

**Problem**: `docker compose: command not found`

**Solution**:
```bash
# Install Docker Compose (v2)
# macOS: Included with Docker Desktop
# Linux:
sudo apt-get update
sudo apt-get install docker-compose-plugin

# Verify
docker compose version

# Or use docker-compose (v1) if installed
which docker-compose
```

### Env File Not Loading

**Problem**: Environment variables not set in Docker

**Diagnosis**:
```bash
# Check env file exists
ls -l ~/path/to/.env.dev

# Check env file mapping
devcli env list api

# Verify injected command
LOG_LEVEL=debug devcli start api --env docker --stage dev
# Look for: "--env-file .env.dev" in the "Command preparation complete" line
```

`--env-file` is only added to `docker run` / `docker create` commands; a
`docker compose up` command is run as written (pass `--env-file` yourself or
let compose read its own `.env`).

**Solution**:
**Correct path (relative to the app `path`)**:
```json
{
  "env_files": {
    "dev": { "docker": ".env.dev" }
  }
}
```

**Or an absolute path**:
```json
{
  "env_files": {
    "dev": { "docker": "/absolute/path/to/.env.dev" }
  }
}
```

### Kubernetes Commands Failing

**Problem**: `kubectl: command not found`

**Solution**:
```bash
# Install kubectl
# macOS:
brew install kubectl

# Linux:
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Verify
kubectl version --client
```

---

## Performance Problems

### Slow Startup

**Problem**: Apps take long time to start

**Diagnosis**:
```bash
# Check specific app logs
cat ~/.devcli/logs/project_app_*.log

# Time command manually
time devcli start api
```

**Common Causes**:
1. Large dependency installation
2. Database migrations
3. Slow Docker image builds

**Solutions**:
```bash
# Pre-build Docker images
docker compose build

# Use image caching
# (already done by default)

# Optimize app startup
# - Remove unnecessary initialization
# - Lazy-load dependencies
# - Use production mode for faster startup
```

### High CPU Usage

**Problem**: devcli using too much CPU

**Diagnosis**:
```bash
# Check CPU usage
top | grep devcli

# Check what's running
devcli status

# Check monitor loop
cat ~/.devcli/logs/devcli.json.* | \
  jq 'select(.target | contains("monitor"))'
```

**Solution**:
```bash
# Stop unused apps
devcli stop --all

# The health check interval is fixed (3 seconds)

# Check for restart loops
devcli metrics | grep "Restarts"
```

---

## Debugging Tips

### Enable Debug Logging

devcli reads `LOG_LEVEL` (`off`, `error`, `warn`, `info`, `debug`,
`trace`); `RUST_LOG` is ignored and there are no per-module filters.

```bash
# Debug level
LOG_LEVEL=debug devcli start api

# Trace level (very verbose)
LOG_LEVEL=trace devcli start api

# Save to file
LOG_LEVEL=debug devcli start api 2> debug.log
```

### Check Process State

```bash
# What devcli tracks
devcli --json status | jq

# Check specific process
devcli --json status | jq '.processes[] | select(.app == "api")'

# Raw state: one file per process
ls ~/.devcli/processes/
cat ~/.devcli/processes/my-project.api.local.json | jq
```

### Verify Configuration

```bash
# Validate config
devcli config validate

# Show app config
devcli config show api

# List all apps
devcli config list

# Check dependency resolution
devcli status frontend --deps
```

### Clean Slate Restart

```bash
# See what would be stopped
devcli stop --all --dry-run

# Stop everything (force if needed)
devcli stop --all
devcli stop --all --force

# Drop the entries of dead processes
devcli monitor

# Check nothing is left
devcli status

# Restart
devcli start api
```

Do not `pkill -f devcli`: the pattern also matches pm-daemon (installed under
`~/.devcli/bin`) and anything else with `devcli` in its command line, and the
apps' state would be left behind. Stop apps through devcli.

### Check System Resources

```bash
# Check disk space
df -h ~/.devcli

# Check memory
free -h

# Check open files
lsof | grep devcli | wc -l

# Check processes
ps aux | grep -E "devcli|pm-daemon"
```

### Inspect Logs

```bash
# View all errors
cat ~/.devcli/logs/devcli.json.* | \
  jq 'select(.level == "ERROR")'

# View specific app logs
cat ~/.devcli/logs/project_app_*.log
devcli logs app --follow

# Follow logs in real-time
tail -f ~/.devcli/logs/devcli.json.$(date +%Y-%m-%d) | jq

# Search for specific error
grep -r "error message" ~/.devcli/logs/
```

---

## Getting Help

If you're still stuck:

1. **Check GitHub Issues**: https://github.com/xDelph/devcli/issues
2. **Create New Issue**: Include:
   - devcli version (`devcli --version`)
   - OS and version
   - Config snippet (sanitized)
   - Error logs
   - Steps to reproduce

3. **Debug Checklist**:
   - [ ] Config validates (`devcli config validate`)
   - [ ] Dependencies exist
   - [ ] Commands work manually
   - [ ] Ports not in use
   - [ ] Logs checked for errors
   - [ ] Process state checked (`devcli --json status`)

---

## Common Error Messages

### "Failed to bind address: Address already in use"

**Cause**: Port already in use by another process

**Fix**:
```bash
# Find process using port
lsof -i :3000

# Kill it
kill -9 <PID>

# Or change port in your app config
```

### "Permission denied" under `~/.devcli/processes/`

**Cause**: Incorrect file permissions (for example files created by
`sudo devcli`)

**Fix**:
```bash
ls -l ~/.devcli/processes/
sudo chown -R "$USER" ~/.devcli
```

### "A monitor (pm-daemon) is already running for ..."

**Cause**: `devcli monitor --daemon` was started while pm-daemon, spawned by
`start`, already monitors the apps

**Fix**: nothing to do; pm-daemon does the monitoring. Use `devcli metrics`
(computed from the state) instead of the metrics API.

### "`stop --all` is not allowed when devcli runs under an AI agent"

**Cause**: devcli detected an AI agent harness (or `DEVCLI_AGENT=1`). Under an
agent, `stop --all`, `stop --project P` without an app and `APP@*` are
refused

**Fix**: stop the apps by name (`devcli stop api --project P`), or, in a
terminal of your own that devcli mistakes for an agent, set `DEVCLI_AGENT=0`.

### "App my-project/api is not currently running"

**Cause**: App crashed or not started (`health-check`; `restart` says
`Process 'api' is not currently running. Use 'start' command instead.`)

**Fix**:
```bash
# Check status
devcli status

# Check logs
cat ~/.devcli/logs/project_api_*.log

# Restart
devcli start api
```

---

## See Also

- [Getting Started](./getting-started.md) - Quick start guide
- [Configuration Reference](./configuration-reference.md) - Config file format
- [Commands Reference](./commands-reference.md) - All CLI commands
- [Advanced Features](./advanced-features.md) - Health checks, metrics, logging
