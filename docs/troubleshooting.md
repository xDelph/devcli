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

**Problem**: `Failed to load config: No such file or directory`

**Solution**:
```bash
# Initialize config
devcli config init

# Verify location
ls -l ~/.devcli/config.yaml

# Check DEVCLI_CONFIG override
echo $DEVCLI_CONFIG
```

### Invalid YAML Syntax

**Problem**: `Failed to parse config: invalid YAML`

**Solution**:
```bash
# Validate YAML syntax
devcli config validate

# Use online YAML validator
cat ~/.devcli/config.yaml | pbcopy
# Paste into https://www.yamllint.com/

# Common issues:
# - Incorrect indentation (use spaces, not tabs)
# - Missing colons
# - Unquoted special characters
```

**Example Fix**:
```yaml
# WRONG (tabs used)
commands:
	local:
		start: npm start

# CORRECT (spaces used)
commands:
  local:
    start: npm start
```

### App Not Found

**Problem**: `App 'api' not found in config`

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

**Problem**: `Circular dependency detected: api → worker → api`

**Solution**:
```yaml
# Remove circular dependency
# WRONG:
api:
  dependencies: [worker]
worker:
  dependencies: [api]

# CORRECT:
api:
  dependencies: [database]
worker:
  dependencies: [api]
```

---

## Process Management

### Process Won't Start

**Problem**: App starts but immediately exits

**Diagnosis**:
```bash
# Check logs
cat ~/.devcli/logs/project_app_*.log

# Check JSON logs for errors
cat ~/.devcli/logs/devcli.*.json | jq 'select(.level == "ERROR")'

# Run command manually to see error
cd ~/path/to/app
npm start  # or whatever the command is
```

**Common Causes**:
1. **Missing dependencies**: `npm install`, `cargo build`, etc.
2. **Port already in use**: Check with `lsof -i :3000`
3. **Environment variables missing**: Check `.env` file
4. **Permissions**: Check file/directory permissions

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

# Manual cleanup if needed
kill -9 <PID>

# Clean up state
devcli monitor
```

### Monitor Daemon Not Starting

**Problem**: Monitor doesn't start with first app

**Diagnosis**:
```bash
# Check if monitor is running
ps aux | grep "devcli monitor --daemon"

# Check monitor logs
cat ~/.devcli/logs/devcli.*.json | \
  jq 'select(.target | contains("monitor"))'
```

**Solution**:
```bash
# Start monitor manually
devcli monitor --daemon

# Then start apps
devcli start api
```

### Lost Process State

**Problem**: `devcli status` shows no processes but apps are running

**Solution**:
```bash
# Find running processes
ps aux | grep node  # or your runtime

# Clean up state
devcli monitor

# Stop all manually
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
```yaml
health_check:
  http:
    url: http://localhost:3000/health
    timeout_secs: 10  # Increase from 5
```

**2. Check expected status**:
```yaml
health_check:
  http:
    expected_status: 200  # Must match actual response
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
```yaml
# Correct host (use localhost, not 0.0.0.0)
health_check:
  tcp:
    host: localhost
    port: 5432

# Increase timeout for slow services
health_check:
  tcp:
    timeout_secs: 30
```

### Health Checks Causing Too Many Restarts

**Problem**: App restarts constantly due to health checks

**Solution**:
```bash
# Temporarily disable health checks
# (edit config, remove health_check section)

# Or increase failure threshold
# (future feature - currently hardcoded to 3)

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

**Problem**: `App 'database' not found (dependency of 'api')`

**Solution**:
```yaml
# Fix app name in dependencies
api:
  dependencies:
    - database  # Must match exact app name

# Check app exists
devcli config list
```

### Wrong Startup Order

**Problem**: Dependencies start in wrong order

**Diagnosis**:
```bash
# Check dependency chain
devcli config show --deps frontend
```

**Solution**:
```yaml
# Define explicit order via dependencies
database:
  dependencies: []

redis:
  dependencies: []

api:
  dependencies:
    - database
    - redis

frontend:
  dependencies:
    - api
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
cat ~/.devcli/logs/devcli.*.json | \
  jq 'select(.level == "ERROR")'
```

**Solution**:
```bash
# Logs are at:
# ~/.devcli/logs/<project>_<app>_<date>.log

# Find by date
ls ~/.devcli/logs/*$(date +%Y%m%d)*

# View in TUI
devcli ui
```

### Metrics API Not Responding

**Problem**: `curl http://localhost:9090/metrics` fails

**Diagnosis**:
```bash
# Check if monitor is running
ps aux | grep "devcli monitor --daemon"

# Check if port 9090 is in use
lsof -i :9090

# Check metrics server logs
cat ~/.devcli/logs/devcli.*.json | \
  jq 'select(.target | contains("metrics"))'
```

**Solutions**:
```bash
# Start monitor
devcli monitor --daemon

# Wait a few seconds for server to start
sleep 3
curl http://localhost:9090/metrics

# Check for port conflicts
# If port 9090 is used, you'll need to modify source code
# (future: make port configurable)
```

### Too Many Log Files

**Problem**: `~/.devcli/logs/` directory is huge

**Solution**:
```bash
# Remove old logs (older than 30 days)
find ~/.devcli/logs -name "*.log" -mtime +30 -delete
find ~/.devcli/logs -name "*.json" -mtime +30 -delete

# Or manually clean up
rm ~/.devcli/logs/*2024-01-*.log

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
RUST_LOG=debug devcli start api --env docker
# Look for: "--env-file .env.dev" in logs
```

**Solution**:
```yaml
# Correct path (relative to app path)
env_files:
  dev:
    docker: .env.dev  # Relative to app path

# Or absolute path
env_files:
  dev:
    docker: /absolute/path/to/.env.dev
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
# Check startup times in metrics
devcli metrics | grep "Avg Startup Time"

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
cat ~/.devcli/logs/devcli.*.json | \
  jq 'select(.target | contains("monitor"))'
```

**Solution**:
```bash
# Stop unused apps
devcli stop --all

# Reduce health check frequency
# (future feature - currently 3 seconds)

# Check for restart loops
devcli metrics | grep "Restarts"
```

---

## Debugging Tips

### Enable Debug Logging

```bash
# Debug level
RUST_LOG=debug devcli start api

# Trace level (very verbose)
RUST_LOG=trace devcli start api

# Module-specific
RUST_LOG=devcli_core::commands::start=debug devcli start api

# Save to file
RUST_LOG=debug devcli start api 2> debug.log
```

### Check Process State

```bash
# View internal state
cat ~/.devcli/state.json | jq

# Check specific process
cat ~/.devcli/state.json | \
  jq '.processes[] | select(.app_name == "api")'
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
devcli config show frontend --deps
```

### Clean Slate Restart

```bash
# Stop everything
devcli stop --all

# Clean up state
rm ~/.devcli/state.json

# Kill any lingering processes
pkill -f "devcli"

# Restart
devcli start api
```

### Check System Resources

```bash
# Check disk space
df -h ~/.devcli

# Check memory
free -h

# Check open files
lsof | grep devcli | wc -l

# Check processes
ps aux | grep devcli
```

### Inspect Logs

```bash
# View all errors
cat ~/.devcli/logs/devcli.*.json | \
  jq 'select(.level == "ERROR")'

# View specific app logs
cat ~/.devcli/logs/project_app_*.log

# Follow logs in real-time
tail -f ~/.devcli/logs/devcli.$(date +%Y-%m-%d).json | jq

# Search for specific error
grep -r "error message" ~/.devcli/logs/
```

---

## Getting Help

If you're still stuck:

1. **Check GitHub Issues**: https://github.com/YOUR_USERNAME/devcli/issues
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
   - [ ] State file checked

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

### "Permission denied: .devcli/state.json"

**Cause**: Incorrect file permissions

**Fix**:
```bash
chmod 644 ~/.devcli/state.json
chmod 755 ~/.devcli
```

### "Failed to connect to metrics API"

**Cause**: Monitor daemon not running

**Fix**:
```bash
devcli monitor --daemon
```

### "App 'api' is not running"

**Cause**: App crashed or not started

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
