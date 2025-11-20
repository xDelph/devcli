# Docker Environment Variables Feature

## Overview

Fixed the handling of runtime environment variables for Docker and OrbStack commands. Environment variables from `.env` files are now properly passed into containers using the appropriate method for each environment.

## Problem

Previously, environment variables loaded from `.env` files were being added to the process environment but not passed into Docker containers. This caused issues like `NODE_ENV is not defined` when running Docker/OrbStack commands.

## Solution

Implemented environment-specific approaches:

### Docker Environment

- Uses `--env-file .env` flag
- Docker handles parsing the .env file
- Simpler and more efficient
- Example: `docker run --env-file .env myimage`

### OrbStack Environment

- Prefixes command with `KEY=VALUE` pairs (shell-style)
- Variables are parsed from .env and injected as shell environment
- Example: `DEBUG=true NODE_ENV=production docker run myimage`
- Standard OrbStack approach for passing environment variables

## Changes Made

### 1. Command Utility (`rustycli-core/src/utils/command.rs`)

- Added `inject_docker_env_file()` function for Docker environment
  - Injects `--env-file` flag into docker run/create commands
  - Checks if flag already exists to avoid duplication
- Added `inject_orbstack_env_vars()` function for OrbStack environment
  - Prefixes command with `KEY=VALUE` pairs
  - Sorts variables alphabetically for consistent output
- Added comprehensive unit tests (12 tests total)

### 2. Start Command (`rustycli-core/src/commands/start/executor.rs`)

- Docker: Injects `--env-file .env` if .env file exists
- OrbStack: Loads .env and prefixes command with variables
- Command display shows the full modified command

### 3. Run Command (`rustycli-core/src/commands/run.rs`)

- Docker: Injects `--env-file .env` if .env file exists
- OrbStack: Loads .env and prefixes command with variables

### 4. Restart Command (`rustycli-core/src/commands/restart.rs`)

- Docker: Injects `--env-file .env` if .env file exists
- OrbStack: Reloads .env and prefixes command with variables

## Examples

### Docker Environment

**Before:**

```bash
# .env file contains:
NODE_ENV=production
PORT=3000

# Command executed:
docker run --platform linux/amd64 myimage

# Result: Environment variables not available in container
```

**After:**

```bash
# .env file contains:
NODE_ENV=production
PORT=3000

# Command executed:
docker run --platform linux/amd64 --env-file .env myimage

# Result: Docker loads .env file into container
```

### OrbStack Environment

**Before:**

```bash
# .env file contains:
NODE_ENV=production
PORT=3000

# Command executed:
docker run --platform linux/amd64 myimage

# Result: NODE_ENV is not defined
```

**After:**

```bash
# .env file contains:
NODE_ENV=production
PORT=3000

# Command executed:
NODE_ENV=production PORT=3000 docker run --platform linux/amd64 myimage

# Result: Environment variables are available in container
```

## Testing

All tests pass:

```bash
cargo test --package rustycli-core utils::command::tests
```

Results:

- ✅ 4 docker env-file injection tests pass
- ✅ 3 orbstack env var injection tests pass
- ✅ 5 platform injection tests pass
- ✅ 12 total command utility tests pass
- ✅ 294+ total unit tests pass

## Usage

No configuration needed! The feature works automatically:

1. Create a `.env` file in your app directory:

```bash
NODE_ENV=production
API_KEY=secret123
PORT=3000
```

2. Run your app with docker or orbstack environment:

```bash
# Docker - uses --env-file flag
rustycli start myapp --env docker

# OrbStack - uses shell-style prefix
rustycli start myapp --env orbstack
```

3. The CLI automatically:
   - Detects the .env file
   - Applies the appropriate injection method for the environment
   - Displays the full command with env vars

## Technical Details

### Docker Approach

- Uses `--env-file` flag (Docker native feature)
- Docker parses the .env file internally
- More efficient - no parsing needed in Rust
- Supports all Docker .env file features

### OrbStack Approach

- Parses .env file in Rust
- Prefixes command with `KEY=VALUE` pairs
- Shell-style environment variable passing
- Variables sorted alphabetically for consistency

### Supported Commands

- `docker run` - Both approaches supported
- `docker create` - Both approaches supported
- Other docker commands - No injection (not applicable)

### File Detection

- Checks for `.env` file in app's working directory
- Only injects if file exists
- For OrbStack: parses file and extracts variables
- For Docker: passes file path to Docker
