# Docker Platform Feature

## Overview

Added support for configuring the Docker platform architecture via preferences. This allows users to specify which platform (e.g., `linux/amd64`, `linux/arm64`) should be used when running Docker and OrbStack commands.

## Changes Made

### 1. Preferences Model (`rustycli-core/src/config/models.rs`)

- Added `docker_platform: String` field to the `Preferences` struct
- Default value: `"linux/amd64"`
- Added `default_docker_platform()` helper function
- Updated `Default` implementation for `Preferences`

### 2. Preferences Command (`rustycli-core/src/commands/preferences.rs`)

- Added support for `docker-platform` preference key
- Validation: ensures platform format is `os/arch` (e.g., `linux/amd64`, `linux/arm64`)
- Updated `pref show` to display the docker-platform setting
- Updated `pref reset` to include docker-platform in output
- Updated error messages to include `docker-platform` in valid keys list

### 3. Command Utility (`rustycli-core/src/utils/command.rs`)

- Created new utility module for command processing
- Implemented `inject_docker_platform()` function that:
  - Detects docker commands (`docker run`, `docker build`, `docker create`, `docker pull`)
  - Injects `--platform <value>` flag after the subcommand
  - Skips injection if `--platform` is already present
  - Only affects docker/orbstack environments
- Added comprehensive unit tests

### 4. Start Command (`rustycli-core/src/commands/start/executor.rs`)

- Modified `start_single_app_process()` to inject platform flag for docker/orbstack environments
- Platform flag is injected before spawning the process
- Updated command display to show the modified command with platform flag

### 5. Run Command (`rustycli-core/src/commands/run.rs`)

- Modified `run_command()` to inject platform flag for docker/orbstack environments
- Platform flag is injected before spawning the process
- Updated command display and process info to use the modified command

### 6. Restart Command (`rustycli-core/src/commands/restart.rs`)

- Modified `restart_command()` to inject platform flag for docker/orbstack environments
- Platform flag is injected before spawning the process
- Updated command display and process info to use the modified command

### 7. Test Updates

- Updated `rustycli-core/src/config/config_tests.rs` to include `docker_platform` field
- Updated `rustycli-core/tests/integration_tests.rs` to include `docker_platform` field
- All existing tests pass
- New unit tests for platform injection pass

## Usage

### View Current Platform Setting

```bash
rustycli pref show
```

Output:

```
Current preferences:
  default-env: local
  detached-mode: false
  auto-start-deps: true
  docker-platform: linux/amd64
```

### Change Platform Setting

```bash
rustycli pref set docker-platform linux/arm64
```

Output:

```
✓ Set docker-platform to 'linux/arm64'
```

### Common Platform Values

- `linux/amd64` - Intel/AMD 64-bit (default)
- `linux/arm64` - ARM 64-bit (Apple Silicon, AWS Graviton)
- `linux/arm/v7` - ARM 32-bit
- `darwin/amd64` - macOS Intel
- `darwin/arm64` - macOS Apple Silicon

### Reset to Defaults

```bash
rustycli pref reset
```

## How It Works

When you run a docker or orbstack command, the CLI automatically injects the `--platform` flag:

**Before:**

```bash
docker run myimage
```

**After (with platform injection):**

```bash
docker run --platform linux/amd64 myimage
```

This happens automatically for:

- `docker run`
- `docker build`
- `docker create`
- `docker pull`

The platform flag is NOT injected if:

- The command already has a `--platform` flag
- The command is not a docker command
- The docker subcommand doesn't support `--platform`
- The environment is not docker or orbstack

## Testing

All tests pass:

```bash
cargo test --package rustycli-core
```

Specific platform injection tests:

```bash
cargo test --package rustycli-core utils::command::tests
```

Results:

- ✅ 5 platform injection tests pass
- ✅ 291 total unit tests pass
- ✅ 10 integration tests pass
