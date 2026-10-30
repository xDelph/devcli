# Configuration Reference

Complete reference for the devcli configuration file (`config.json`).

## Configuration File Location

DevCLI looks for configuration files in the following priority order:

1. **Environment Variable**: `DEVCLI_CONFIG_DIR` (for testing/override; the
   former name `devcli_CONFIG_DIR` still works, an empty value is ignored)
2. **Local Project Config**: `./.devcli/config.json` (current directory)
3. **Global Config**: `~/.devcli/config.json` (home directory)

This allows you to have:
- **Global config** at `~/.devcli/config.json` for all your personal projects
- **Project-specific config** at `./.devcli/config.json` for team projects or specific workflows

```bash
# Override for testing
export DEVCLI_CONFIG_DIR=/tmp/test-config
devcli start api

# Or use local project config
cd ~/my-project
mkdir .devcli
# Create .devcli/config.json
devcli start api  # Uses ./.devcli/config.json
```

### Preferences File

Similarly, preferences are loaded from:
1. `DEVCLI_CONFIG_DIR/preferences.json` (if env var set)
2. `./.devcli/preferences.json` (current directory)
3. `~/.devcli/preferences.json` (home directory)

The directory chosen this way also holds process state (`processes/`) and
logs (`logs/`).

## File Structure

```json
{
  "projects": {
    "project-name": {
      "apps": {
        "app-name": {
          "type": "nodejs",
          "path": "~/code/app-name",
          "commands": {
            "local": { "start": "npm start" }
          },
          "defaults": { "local": "start" }
        }
      }
    }
  }
}
```

## Full Example

```json
{
  "projects": {
    "awesome-monorepo": {
      "alternative_name": "ProjectA",
      "apps": {
        "database": {
          "type": "docker",
          "path": "~/code/awesome-monorepo/docker",
          "commands": {
            "local": {
              "start": "docker compose up postgres",
              "stop": "docker compose stop postgres"
            },
            "docker": {
              "start": "docker compose up postgres",
              "stop": "docker compose stop postgres"
            }
          },
          "defaults": {
            "local": "start",
            "docker": "start"
          },
          "health_check": {
            "type": "tcp",
            "host": "localhost",
            "port": 5432,
            "timeout_secs": 10
          }
        },
        "api": {
          "type": "nodejs",
          "alternative_name": "Backend",
          "path": "~/code/awesome-monorepo/apps/api",
          "port": 3000,
          "commands": {
            "local": {
              "start": "npm run dev",
              "test": "npm test",
              "build": "npm run build",
              "build:production": "npm run build:prod"
            },
            "docker": {
              "start": "docker compose up api",
              "build": "docker compose build api"
            },
            "k8s": {
              "apply": "kubectl apply -f k8s/",
              "delete": "kubectl delete -f k8s/",
              "restart": "kubectl rollout restart deployment/api"
            }
          },
          "defaults": {
            "local": "start",
            "docker": "start",
            "k8s": "apply"
          },
          "dependencies": [
            { "project": "awesome-monorepo", "app": "database" }
          ],
          "health_check": {
            "type": "http",
            "url": "http://localhost:3000/health",
            "expected_status": 200,
            "timeout_secs": 5
          },
          "restart_policy": {
            "max_restarts": 5,
            "restart_window_secs": 300,
            "initial_backoff_secs": 2,
            "max_backoff_secs": 60
          },
          "env_files": {
            "dev": {
              "local": ".env.dev",
              "docker": ".env.docker.dev"
            },
            "qa": {
              "docker": ".env.qa",
              "k8s": "k8s/env/qa.env"
            },
            "prod": {
              "k8s": "k8s/env/prod.env"
            }
          },
          "default_stages": {
            "local": "dev",
            "docker": "qa"
          }
        },
        "worker": {
          "type": "nodejs",
          "path": "~/code/awesome-monorepo/apps/worker",
          "commands": {
            "local": {
              "start": "npm run worker"
            },
            "docker": {
              "start": "docker compose up worker"
            }
          },
          "defaults": {
            "local": "start",
            "docker": "start"
          },
          "dependencies": [
            { "project": "awesome-monorepo", "app": "database" },
            { "project": "awesome-monorepo", "app": "api" }
          ],
          "restart_policy": {
            "max_restarts": 10,
            "restart_window_secs": 600
          }
        },
        "frontend": {
          "type": "nodejs",
          "path": "~/code/awesome-monorepo/apps/frontend",
          "commands": {
            "local": {
              "start": "npm run dev",
              "build": "npm run build",
              "preview": "npm run preview"
            },
            "docker": {
              "start": "docker compose up frontend"
            }
          },
          "defaults": {
            "local": "start",
            "docker": "start"
          },
          "dependencies": [
            { "project": "awesome-monorepo", "app": "api" }
          ],
          "health_check": {
            "type": "http",
            "url": "http://localhost:5173"
          }
        }
      }
    }
  }
}
```

## App Fields

### Required Fields

#### `type`
**Type**: String
**Required**: Yes
**Description**: Type of application. devcli treats it as a free-form label
(shown in `status`, the TUI and `config show`); it does not change how
commands are run.

**Values produced by auto-detection** (and accepted by `auto-add --type`):
`nodejs`, `nx`, `nx-workspace`, `rust`, `python`, `docker`, `redis`,
`traefik`. Any other string is accepted when you write the config by hand.

**Example**:
```json
{
  "type": "nodejs"
}
```

#### `path`
**Type**: String (absolute or `~` expanded)
**Required**: Yes
**Description**: Working directory for the app

**Example**:
```json
{
  "path": "~/code/my-project/apps/api"
}
```

#### `commands`
**Type**: Object
**Required**: Yes
**Description**: Commands to run in different environments

**Structure**:
```json
{
  "commands": {
    "<environment>": {
      "<command-name>": "<command-string>"
    }
  },
  "defaults": {
    "<environment>": "<default-command-name>"
  }
}
```

**Environments**: `local`, `docker`, `orbstack`, `docker-compose`, `k8s`,
`ci`. Each one is optional; define only the environments the app uses, but at
least one. On the command line, `--env compose` is accepted for
`docker-compose`.

**How commands run**: a command string is split into arguments the way a
shell would (quotes are honoured), then executed directly, without a shell.
Pipes, `&&`, redirections, `$VAR` expansion and `VAR=value` prefixes therefore
do not work as such; wrap them in `sh -c '...'`:

```json
{
  "commands": {
    "local": {
      "start": "sh -c 'npm install && npm run dev'"
    }
  }
}
```

The default command of each environment lives in the app-level `defaults`
object (see [Default Commands](#default-commands)), not inside `commands`.

**Example**:
```json
{
  "commands": {
    "local": {
      "start": "npm run dev",
      "test": "npm test",
      "build": "npm run build",
      "build:production": "npm run build --mode production"
    },
    "docker": {
      "start": "docker compose up api",
      "build": "docker compose build api"
    }
  },
  "defaults": {
    "local": "start",
    "docker": "start"
  }
}
```

### Optional Fields

#### `alternative_name`
**Type**: String
**Default**: None
**Description**: Privacy-friendly display name for screenshots or sharing
**Applies to**: Both projects and apps

**Features**:
- Displayed in TUI by default when present
- Can be used in commands (start/stop/restart)
- Shown as `actualName(alternativeName)` in terminal commands
- Hides real names in screenshots and demonstrations

**Example (Project)**:
```json
{
  "projects": {
    "my-company-internal-project": {
      "alternative_name": "ProjectX",
      "apps": {
        "internal-api": {
          "alternative_name": "API",
          "type": "nodejs",
          "path": "~/code/internal-api",
          "commands": { "local": { "start": "npm start" } },
          "defaults": { "local": "start" }
        }
      }
    }
  }
}
```

**Example (App)**:
```json
{
  "apps": {
    "sensitive-app-name": {
      "alternative_name": "MyApp",
      "type": "nodejs",
      "path": "~/code/sensitive-app-name",
      "commands": { "local": { "start": "npm start" } },
      "defaults": { "local": "start" }
    }
  }
}
```

**Usage**:
```bash
# Start using alternative name
devcli start MyApp

# TUI displays "MyApp" instead of "sensitive-app-name"
devcli ui

# Config commands show both
devcli config show sensitive-app-name
# Output: App: sensitive-app-name(MyApp)
#         Project: my-company-internal-project(ProjectX)
```

**Use Cases**:
- Recording screencasts without revealing internal names
- Sharing configuration examples publicly
- Presentations and demos
- Training materials

#### `dependencies`
**Type**: Array of `{ "project": "<project>", "app": "<app>" }` objects
**Default**: `[]`
**Description**: Apps that must be started before this app. Each entry names
the project and the app (a dependency can live in another project). Plain
strings such as `"database"` are rejected when the config is loaded.

**Features**:
- Automatic dependency resolution
- Transitive dependencies supported
- Circular dependency detection
- Can be disabled with `--skip-deps`
- With the `auto-start-deps` preference set to `false`, missing dependencies
  are an error instead of being started

**Example**:
```json
{
  "dependencies": [
    { "project": "my-project", "app": "database" },
    { "project": "infrastructure", "app": "redis" },
    { "project": "my-project", "app": "auth-service" }
  ]
}
```

**Behavior**:
```bash
# Starting frontend also starts api and database
devcli start frontend
# → Starts: database → api → frontend
```

#### `port`
**Default**: None
**Description**: Port the app listens on, when it reads it from `$PORT`.
devcli exports `PORT` and `DEVCLI_PORT` to the app. In a git worktree they
are shifted by `DEVCLI_PORT_OFFSET` (100 per worktree slot), so the copies of
several worktrees run side by side; health checks on this port are shifted
too. A `PORT` set by the app's env files wins over `port` (and is the one
shifted in worktrees).

```json
{
  "port": 3000
}
```

Every app also gets `DEVCLI_PORT_<APP>` for each of its dependencies that
declares a `port` (as that dependency runs for this app, shifted when it runs in
the same worktree), and `DEVCLI_WORKTREE` (the worktree's slug, empty in the
configured checkout), `DEVCLI_WORKTREE_INDEX` and `DEVCLI_PORT_OFFSET`, plus
`DEVCLI_WORKTREE_PATH` and `DEVCLI_WORKTREE_BRANCH` in a worktree. Health
check URLs, hosts and commands can use `${DEVCLI_PORT}` and the other
variables.

#### `health_check`
**Type**: Object, tagged by `type`
**Default**: None (only "the process is alive" is checked)
**Description**: How the monitor (pm-daemon) decides that a running app is
healthy

**Types**: `http`, `tcp`, `command`, `process`

**HTTP Health Check** (a GET on `url`; `timeout_secs` defaults to 5,
`expected_status` to 200, and with 200 any 2xx status passes):
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

**TCP Health Check** (a connection to `host:port`; `timeout_secs` defaults
to 5):
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

**Command Health Check** (run with `sh -c`; healthy when it exits with
`expected_exit_code`, default 0; `timeout_secs` defaults to 5):
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

**Process Health Check** (healthy as long as the process runs; the same as no
`health_check`):
```json
{
  "health_check": {
    "type": "process"
  }
}
```

URLs, hosts and commands can use `${DEVCLI_PORT}`, `${PORT}` and the other
`DEVCLI_*` variables (see [`port`](#port)).

**Features**:
- The monitor runs checks every 3 seconds
- Failures are not counted during the first 30 seconds after a (re)start
- Tracks failure count
- Triggers a restart after 3 consecutive failures (when the app has an
  enabled `restart_policy`; otherwise the app is stopped)
- Resets on successful check

#### `restart_policy`
**Type**: Object
**Default**: None (no automatic restart)
**Description**: Automatic restart configuration

**Fields** (all optional; defaults shown):
```json
{
  "restart_policy": {
    "enabled": true,
    "max_restarts": 5,
    "restart_window_secs": 300,
    "initial_backoff_secs": 1,
    "max_backoff_secs": 60,
    "backoff_multiplier": 2.0,
    "restart_on_exit_codes": [1, 137]
  }
}
```

- `enabled`: `false` keeps the policy but never restarts
- `max_restarts`: restarts allowed within `restart_window_secs` (`0` = unlimited)
- `restart_window_secs`: the sliding window restarts are counted in
- `initial_backoff_secs`, `backoff_multiplier`, `max_backoff_secs`: delay
  before a restart
- `restart_on_exit_codes`: restart only on these exit codes; without it, any
  non-zero exit code restarts and exit code 0 does not

**Behavior**:
- Restarts the process when it dies or fails 3 health checks in a row (the
  exit code of the first instance started by `devcli start` is not
  observable, so any exit of it counts as a crash)
- Exponential backoff: `initial_backoff_secs × backoff_multiplier^(n-1)`,
  capped at `max_backoff_secs`; with the defaults 1s, 1s, 2s, 4s, 8s, ... up
  to 60s
- Only restarts within the last `restart_window_secs` count towards
  `max_restarts`
- Once `max_restarts` is reached, the process is stopped and no longer
  tracked

**Example**:
```json
{
  "restart_policy": {
    "max_restarts": 10,
    "restart_window_secs": 600,
    "initial_backoff_secs": 3
  }
}
```

#### `env_files`
**Type**: Object
**Default**: `{}`
**Description**: Environment files for different stages

**Structure**:
```json
{
  "env_files": {
    "<stage>": {
      "<context>": "<file-path>"
    }
  }
}
```

**Stages**: `dev`, `qa`, `preprod`, `prod`, or `base` for a file used by every
stage (when the selected stage has no file for that context, or no stage is
selected)
**Contexts**: the environment names: `local`, `docker`, `orbstack`,
`docker-compose`, `k8s`, `ci`

Without `env_files`, devcli looks for `.env` files in the app directory
(`.env`, `.env.<stage>`, ...) and layers them.

**Example**:
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
    },
    "base": {
      "local": ".env"
    }
  }
}
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
- `~` is not expanded here

#### `default_stages`
**Type**: Object (environment → stage)
**Default**: None
**Description**: The stage `start` uses in each environment when `--stage` is
not given; falls back to the `default-stage` preference. Set it with
`devcli env set-default APP --context ENV --stage STAGE`.

```json
{
  "default_stages": {
    "local": "dev",
    "docker": "qa"
  }
}
```

#### `dockerfile_path`
**Type**: String
**Default**: None
**Description**: Dockerfile path relative to the app's `path`. In the
`docker` and `orbstack` environments it is passed as `-f` to `docker build`
commands that have no `-f`/`--file`, and `.env` files next to it are also
looked for. Set by `auto-add` when it finds a Dockerfile.

## Command Variants

Commands can have **variants** for different build targets, test suites, etc.

### Syntax

```json
{
  "commands": {
    "local": {
      "<command>": "<simple-command>",
      "<command>:<variant>": "<variant-command>"
    }
  }
}
```

### Examples

**Build Variants**:
```json
{
  "commands": {
    "local": {
      "build": "npm run build",
      "build:development": "npm run build --mode development",
      "build:production": "npm run build --mode production",
      "build:preprod": "npm run build --mode preprod"
    }
  }
}
```

**Test Variants**:
```json
{
  "commands": {
    "local": {
      "test": "npm test",
      "test:unit": "npm run test:unit",
      "test:integration": "npm run test:integration",
      "test:e2e": "npm run test:e2e"
    }
  }
}
```

**Docker Variants**:
```json
{
  "commands": {
    "docker": {
      "build": "docker compose build",
      "build:no-cache": "docker compose build --no-cache",
      "build:dev": "docker compose -f docker-compose.dev.yml build"
    }
  }
}
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

`defaults` maps an environment to the name of one of its commands. `start`
and `restart` run that command when no variant is given. `defaults` is
optional: an app without it can still run named commands with
`devcli run APP COMMAND`, but `devcli start APP --env ENV` fails when
`defaults.ENV` is missing, and `config validate` reports an environment that
has commands but no default as an error.

```json
{
  "commands": {
    "local": {
      "start": "npm run dev",
      "test": "npm test"
    }
  },
  "defaults": {
    "local": "start"
  }
}
```

**Used When**:
```bash
devcli start api               # Uses defaults.local → 'start'
devcli start api --env docker  # Uses defaults.docker
```

## App Types & Auto-Detection

devcli can **auto-detect** app types and generate commands:

### Supported Auto-Detection

| Type | Detection | Generated Commands |
|------|-----------|-------------------|
| **Node.js** | `package.json` | start, test, build (from scripts) |
| **Nx Monorepo** | `nx.json` | Per-app targets |
| **Docker** | `Dockerfile` | build, run (single/multi-stage) |
| **Docker Compose** | `docker-compose.yml`, `compose.yaml` (and variants) | up, down, build |
| **Kubernetes** | `k8s/*.yaml` | apply, delete, restart |
| **Rust** | `Cargo.toml` (crates and workspaces) | run, test, build, check |
| **Python** | `setup.py`/`pyproject.toml` (incl. uv workspaces) | Based on project type |
| **Redis** | `redis.conf` (or a `redis/` directory) | start (`redis-server`) |
| **Traefik** | `traefik.yml`/`traefik.yaml`/`traefik.toml` | start (`traefik`) |

### Auto-Add Command

```bash
cd ~/my-app
devcli auto-add

# Detects app type and adds to config
# You can edit ~/.devcli/config.json after
```

## Environment Variables

### In Commands

Commands run without a shell, so a `VAR=value` prefix needs `sh -c` (or set
the variable in an env file, or use the app's [`port`](#port) for `PORT`):

```json
{
  "commands": {
    "local": {
      "start": "sh -c 'PORT=3000 npm run dev'",
      "test": "sh -c 'NODE_ENV=test npm test'"
    }
  }
}
```

### Env Files (local, k8s, ci)

The selected env file (or the detected `.env` layers) is loaded into the
process environment of the command.

### Env Files (docker, orbstack, docker-compose)

`--env-file <file>` is added to `docker run` and `docker create` commands
(before the image name, unless the command already has `--env-file`). The file
is the one pinned in `env_files` (an error if it does not exist) or, without
one, the detected `.env` layers merged into
`~/.devcli/env-files/<project>_<app>_<env>.env`. Other commands, such as
`docker compose up`, are left unchanged.

```json
{
  "commands": {
    "docker": {
      "start": "docker run --rm -p 3000:3000 my-api"
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
docker run --platform linux/amd64 --rm -p 3000:3000 --env-file .env.dev my-api
```

`docker run`, `build`, `create` and `pull` commands in these environments also
get `--platform` from the `docker-platform` preference (unless they have one),
and `DOCKER_CONTEXT` is set to `default` (docker) or `orbstack` (orbstack).

## Validation

### Validate Config

```bash
# Check for errors
devcli config validate

# Output:
# Validating configuration...
#
# ✓ Configuration is valid!
#   - 5 projects
#   - 12 apps
```

Errors make it exit with code 1; warnings (an unknown field, a missing env
file) do not.

### Common Errors

**Missing required fields** (the file does not load at all):
```
Error: Failed to load configuration: Failed to parse JSON from ~/.devcli/config.json: missing field `type` at line 12 column 5
```

**Invalid dependencies**:
```
  - [my-project/api] Invalid dependency my-project/database: App 'database' not found in project 'my-project'.
```

**Circular dependencies**:
```
  - [my-project/api] Dependency resolution error: Circular dependency detected: my-project/api -> my-project/worker -> my-project/api. Check app dependencies in config.
```

**Unknown field** (warning):
```
  - [my-project/api] Unknown field 'dependecies' is ignored. Did you mean 'dependencies'?
```

## Best Practices

### 1. Use Projects to Group Related Apps

```text
{
  "projects": {
    "main-app": {
      "apps": { "api": { ... }, "worker": { ... }, "frontend": { ... } }
    },
    "monitoring": {
      "apps": { "prometheus": { ... }, "grafana": { ... } }
    }
  }
}
```

### 2. Define Dependencies Explicitly

```json
{
  "frontend": {
    "dependencies": [
      { "project": "main-app", "app": "api" }
    ]
  },
  "api": {
    "dependencies": [
      { "project": "main-app", "app": "database" },
      { "project": "main-app", "app": "redis" }
    ]
  }
}
```

### 3. Add Health Checks for Critical Services

```json
{
  "database": {
    "health_check": {
      "type": "tcp",
      "host": "localhost",
      "port": 5432
    }
  },
  "api": {
    "health_check": {
      "type": "http",
      "url": "http://localhost:3000/health"
    }
  }
}
```

### 4. Use Restart Policies for Long-Running Services

```json
{
  "worker": {
    "restart_policy": {
      "max_restarts": 10,
      "restart_window_secs": 600
    }
  },
  "database": {
    "restart_policy": {
      "max_restarts": 3,
      "restart_window_secs": 300
    }
  }
}
```

### 5. Organize Env Files by Stage

```
apps/api/
├── .env.dev          # Local development
├── .env.docker.dev   # Docker development
├── .env.qa           # QA environment
└── .env.preprod      # Pre-production environment
```

### 6. Use Command Variants for Build Targets

```json
{
  "commands": {
    "local": {
      "build:dev": "npm run build:dev",
      "build:prod": "npm run build:prod",
      "build:analyze": "npm run build:analyze"
    }
  }
}
```

## Configuration Management

### Editing Config

```bash
# Open in $EDITOR
devcli config edit

# Or edit manually
vim ~/.devcli/config.json
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
# (manual edit required: there is no remove command)
```

**Manual Edit**:
Just edit `~/.devcli/config.json` directly.

### Backup Config

```bash
# Backup
cp ~/.devcli/config.json ~/.devcli/config.json.backup

# Restore
cp ~/.devcli/config.json.backup ~/.devcli/config.json
```

## Advanced Configuration

### Custom Dockerfile Paths

```json
{
  "commands": {
    "docker": {
      "build": "docker build -f docker/Dockerfile.dev .",
      "start": "docker run -p 3000:3000 my-image"
    }
  }
}
```

### Multiple Compose Files

```json
{
  "commands": {
    "docker": {
      "start:dev": "docker compose -f docker-compose.yml -f docker-compose.dev.yml up",
      "start:prod": "docker compose -f docker-compose.yml -f docker-compose.prod.yml up"
    }
  }
}
```

### Kubernetes Namespaces

```json
{
  "commands": {
    "k8s": {
      "apply": "kubectl apply -f k8s/ -n my-namespace",
      "delete": "kubectl delete -f k8s/ -n my-namespace"
    }
  }
}
```

### OrbStack Optimization

```json
{
  "commands": {
    "orbstack": {
      "start": "docker compose up api"
    }
  }
}
```

## See Also

- [Commands Reference](./commands-reference.md) - All CLI commands
- [Advanced Features](./advanced-features.md) - Health checks, metrics, logging
- [Getting Started](./getting-started.md) - Quick start guide
