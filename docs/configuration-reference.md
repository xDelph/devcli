# Configuration Reference

Complete reference for the devcli configuration file (`~/.devcli/config.yaml`).

## Configuration File Location

- **Default**: `~/.devcli/config.yaml`
- **Override**: Set `DEVCLI_CONFIG` environment variable

```bash
export DEVCLI_CONFIG=~/my-custom-config.yaml
devcli start api
```

## File Structure

```yaml
projects:
  project-name:
    apps:
      app-name:
        # App configuration...
```

## Full Example

```yaml
projects:
  awesome-monorepo:
    apps:
      database:
        app_type: docker
        path: ~/code/awesome-monorepo/docker
        commands:
          local:
            start: docker compose up postgres
            stop: docker compose stop postgres
          docker:
            start: docker compose up postgres
            stop: docker compose stop postgres
        health_check:
          tcp:
            host: localhost
            port: 5432
            timeout_secs: 10

      api:
        app_type: nodejs
        path: ~/code/awesome-monorepo/apps/api
        commands:
          local:
            default: start
            start: npm run dev
            test: npm test
            build: npm run build
            build:production: npm run build:prod
          docker:
            default: start
            start: docker compose up api
            build: docker compose build api
          k8s:
            default: apply
            apply: kubectl apply -f k8s/
            delete: kubectl delete -f k8s/
            restart: kubectl rollout restart deployment/api
        dependencies:
          - database
          - redis
        health_check:
          http:
            url: http://localhost:3000/health
            expected_status: 200
            timeout_secs: 5
        restart_policy:
          max_restarts: 5
          restart_window_secs: 300
          backoff_secs: 5
        env_files:
          dev:
            local: .env.dev
            docker: .env.docker.dev
          qa:
            docker: .env.qa
            k8s: k8s/env/qa.yaml
          prod:
            k8s: k8s/env/prod.yaml

      worker:
        app_type: nodejs
        path: ~/code/awesome-monorepo/apps/worker
        commands:
          local:
            default: start
            start: npm run worker
          docker:
            default: start
            start: docker compose up worker
        dependencies:
          - database
          - redis
          - api
        restart_policy:
          max_restarts: 10
          restart_window_secs: 600

      frontend:
        app_type: nodejs
        path: ~/code/awesome-monorepo/apps/frontend
        commands:
          local:
            default: start
            start: npm run dev
            build: npm run build
            preview: npm run preview
          docker:
            default: start
            start: docker compose up frontend
        dependencies:
          - api
        health_check:
          http:
            url: http://localhost:5173
            expected_status: 200
```

## App Fields

### Required Fields

#### `app_type`
**Type**: String
**Required**: Yes
**Description**: Type of application

**Supported Values**:
- `nodejs` - Node.js applications
- `docker` - Docker/Docker Compose
- `k8s` - Kubernetes applications
- `rust` - Rust applications
- `go` - Go applications
- `python` - Python applications
- `generic` - Any other type

**Example**:
```yaml
app_type: nodejs
```

#### `path`
**Type**: String (absolute or `~` expanded)
**Required**: Yes
**Description**: Working directory for the app

**Example**:
```yaml
path: ~/code/my-project/apps/api
path: /home/user/projects/api
```

#### `commands`
**Type**: Object
**Required**: Yes
**Description**: Commands to run in different environments

**Structure**:
```yaml
commands:
  <environment>:
    <command-name>: <command-string>
    default: <default-command-name>
```

**Environments**: `local`, `docker`, `orbstack`, `k8s`

**Example**:
```yaml
commands:
  local:
    default: start
    start: npm run dev
    test: npm test
    build: npm run build
    build:production: npm run build --mode production
  docker:
    default: start
    start: docker compose up api
    build: docker compose build api
```

### Optional Fields

#### `dependencies`
**Type**: Array of strings
**Default**: `[]`
**Description**: Apps that must be started before this app

**Features**:
- Automatic dependency resolution
- Transitive dependencies supported
- Circular dependency detection
- Can be disabled with `--skip-deps`

**Example**:
```yaml
dependencies:
  - database
  - redis
  - auth-service
```

**Behavior**:
```bash
# Starting frontend also starts api and database
devcli start frontend
# → Starts: database → api → frontend
```

#### `health_check`
**Type**: Object
**Default**: None
**Description**: Health check configuration

**Types**: `http`, `tcp`, `command`

**HTTP Health Check**:
```yaml
health_check:
  http:
    url: http://localhost:3000/health
    expected_status: 200
    timeout_secs: 5
```

**TCP Health Check**:
```yaml
health_check:
  tcp:
    host: localhost
    port: 5432
    timeout_secs: 10
```

**Command Health Check**:
```yaml
health_check:
  command:
    cmd: curl -f http://localhost:3000/health
    timeout_secs: 5
```

**Features**:
- Monitor daemon runs checks every 3 seconds
- Tracks failure count
- Triggers restart after 3 consecutive failures
- Resets on successful check

#### `restart_policy`
**Type**: Object
**Default**: None
**Description**: Automatic restart configuration

**Fields**:
```yaml
restart_policy:
  max_restarts: 5           # Maximum restart attempts
  restart_window_secs: 300  # Time window (5 minutes)
  backoff_secs: 5           # Initial backoff delay
```

**Behavior**:
- Restarts process on crash (non-zero exit code)
- Exponential backoff: 5s, 10s, 20s, 40s, 80s...
- Resets restart count after window expires
- Stops restarting after max_restarts reached

**Example**:
```yaml
# Restart up to 10 times within 10 minutes
restart_policy:
  max_restarts: 10
  restart_window_secs: 600
  backoff_secs: 3
```

#### `env_files`
**Type**: Object
**Default**: `{}`
**Description**: Environment files for different stages

**Structure**:
```yaml
env_files:
  <stage>:
    <context>: <file-path>
```

**Stages**: Any string (e.g., `dev`, `qa`, `staging`, `prod`)
**Contexts**: `local`, `docker`, `orbstack`, `k8s`

**Example**:
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
    default_context: k8s  # Default context for this stage
```

**Usage**:
```bash
# Use dev stage (local context)
devcli start api --stage dev

# Use qa stage (docker context)
devcli start api --stage qa --env docker
```

**File Paths**:
- Relative to app's `path` directory
- Absolute paths supported
- `~` expansion supported

## Command Variants

Commands can have **variants** for different build targets, test suites, etc.

### Syntax

```yaml
commands:
  local:
    <command>: <simple-command>
    <command>:<variant>: <variant-command>
```

### Examples

**Build Variants**:
```yaml
commands:
  local:
    build: npm run build
    build:development: npm run build --mode development
    build:production: npm run build --mode production
    build:staging: npm run build --mode staging
```

**Test Variants**:
```yaml
commands:
  local:
    test: npm test
    test:unit: npm run test:unit
    test:integration: npm run test:integration
    test:e2e: npm run test:e2e
```

**Docker Variants**:
```yaml
commands:
  docker:
    build: docker compose build
    build:no-cache: docker compose build --no-cache
    build:dev: docker compose -f docker-compose.dev.yml build
```

### Usage

```bash
# Run specific variant
devcli run api build:production
devcli run api test:integration

# Default variant
devcli run api build
```

## Default Commands

Each environment should have a `default` command:

```yaml
commands:
  local:
    default: start
    start: npm run dev
    test: npm test
```

**Used When**:
```bash
devcli start api          # Uses 'default' → 'start'
devcli start api --env docker  # Uses docker's 'default'
```

## App Types & Auto-Detection

devcli can **auto-detect** app types and generate commands:

### Supported Auto-Detection

| Type | Detection | Generated Commands |
|------|-----------|-------------------|
| **Node.js** | `package.json` | start, test, build (from scripts) |
| **Nx Monorepo** | `nx.json` | Per-app targets |
| **Docker** | `Dockerfile` | build, run (single/multi-stage) |
| **Docker Compose** | `docker-compose.yml` | up, down, build |
| **Kubernetes** | `k8s/*.yaml` | apply, delete, restart |
| **Rust** | `Cargo.toml` | run, test, build, check |
| **Go** | `go.mod` | run, test, build |
| **Python** | `setup.py`/`pyproject.toml` | Based on project type |

### Auto-Add Command

```bash
cd ~/my-app
devcli auto-add

# Detects app type and adds to config
# You can edit ~/.devcli/config.yaml after
```

## Environment Variables

### In Commands

```yaml
commands:
  local:
    start: PORT=3000 npm run dev
    test: NODE_ENV=test npm test
```

### Env Files (Docker)

Docker commands automatically inject `--env-file`:

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

### Env Files (Kubernetes)

K8s commands use env files as ConfigMaps/Secrets (manual setup required).

## Validation

### Validate Config

```bash
# Check for errors
devcli config validate

# Output:
# ✓ Config is valid
# ✓ 5 projects, 12 apps
# ✓ No circular dependencies
```

### Common Errors

**Missing required fields**:
```
Error: App 'api' missing required field 'app_type'
```

**Invalid dependencies**:
```
Error: App 'api' depends on non-existent app 'database'
```

**Circular dependencies**:
```
Error: Circular dependency detected: api → worker → api
```

## Best Practices

### 1. Use Projects to Group Related Apps

```yaml
projects:
  main-app:
    apps:
      api: ...
      worker: ...
      frontend: ...

  monitoring:
    apps:
      prometheus: ...
      grafana: ...
```

### 2. Define Dependencies Explicitly

```yaml
# Frontend depends on API, API depends on DB
frontend:
  dependencies: [api]

api:
  dependencies: [database, redis]
```

### 3. Add Health Checks for Critical Services

```yaml
database:
  health_check:
    tcp:
      host: localhost
      port: 5432

api:
  health_check:
    http:
      url: http://localhost:3000/health
```

### 4. Use Restart Policies for Long-Running Services

```yaml
# Background worker - aggressive restarts
worker:
  restart_policy:
    max_restarts: 10
    restart_window_secs: 600

# Database - conservative restarts
database:
  restart_policy:
    max_restarts: 3
    restart_window_secs: 300
```

### 5. Organize Env Files by Stage

```
apps/api/
├── .env.dev          # Local development
├── .env.docker.dev   # Docker development
├── .env.qa           # QA environment
└── .env.staging      # Staging environment
```

### 6. Use Command Variants for Build Targets

```yaml
commands:
  local:
    build:dev: npm run build:dev
    build:prod: npm run build:prod
    build:analyze: npm run build:analyze
```

## Configuration Management

### Editing Config

```bash
# Open in $EDITOR
devcli config edit

# Or edit manually
vim ~/.devcli/config.yaml
```

### Viewing Config

```bash
# List all projects and apps
devcli config list

# Show specific app
devcli config show api

# Filter by project
devcli config list --project awesome-monorepo
```

### Adding/Removing Apps

**Via Commands**:
```bash
# Auto-detect and add
devcli auto-add

# Remove app
# (Manual edit required, or use config remove)
```

**Manual Edit**:
Just edit `~/.devcli/config.yaml` directly.

### Backup Config

```bash
# Backup
cp ~/.devcli/config.yaml ~/.devcli/config.yaml.backup

# Restore
cp ~/.devcli/config.yaml.backup ~/.devcli/config.yaml
```

## Advanced Configuration

### Custom Dockerfile Paths

```yaml
commands:
  docker:
    build: docker build -f docker/Dockerfile.dev .
    start: docker run -p 3000:3000 my-image
```

### Multiple Compose Files

```yaml
commands:
  docker:
    start:dev: docker compose -f docker-compose.yml -f docker-compose.dev.yml up
    start:prod: docker compose -f docker-compose.yml -f docker-compose.prod.yml up
```

### Kubernetes Namespaces

```yaml
commands:
  k8s:
    apply: kubectl apply -f k8s/ -n my-namespace
    delete: kubectl delete -f k8s/ -n my-namespace
```

### OrbStack Optimization

```yaml
commands:
  orbstack:
    start: docker compose up api
    # OrbStack auto-injects DOCKER_HOST
```

## See Also

- [Commands Reference](./commands-reference.md) - All CLI commands
- [Advanced Features](./advanced-features.md) - Health checks, metrics, logging
- [Getting Started](./getting-started.md) - Quick start guide
