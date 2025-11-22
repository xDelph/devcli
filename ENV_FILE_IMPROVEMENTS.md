# Environment File Improvements

## Overview

Enhanced the .env file handling with two important improvements:

1. Empty environment variable values are replaced with `XXX` placeholder
2. .env files at the Dockerfile level are prioritized over root-level .env files

## Problems Solved

### Problem 1: Empty Environment Variables

When a .env file contains empty values like `APM_SECRET=`, the variable would be set to an empty string, which can cause issues in some applications.

**Solution:** Replace empty values with `XXX` placeholder to make it obvious that a value needs to be set.

### Problem 2: .env File Priority

When a Dockerfile exists in a subdirectory (e.g., `docker/Dockerfile`), the .env file at that level should take priority over the root .env file, but this wasn't happening.

**Solution:** Implement a priority system that checks for .env files in this order:

1. .env file at the Dockerfile level (if Dockerfile exists in subdirectory)
2. .env file at the root level

## Changes Made

### 1. Empty Value Handling (`devcli-core/src/detection/environments/orbstack.rs`)

**Updated `parse_env_file()` function:**

```rust
// Replace empty values with XXX placeholder
let value = if value.is_empty() {
    "XXX".to_string()
} else {
    value
};
```

**Examples:**

- `APM_SECRET=` → `APM_SECRET=XXX`
- `API_KEY= ` → `API_KEY=XXX`
- `TOKEN=""` → `TOKEN=XXX`
- `PORT=3000` → `PORT=3000` (unchanged)

### 2. .env File Priority (`devcli-core/src/detection/environments/orbstack.rs`)

**Added `find_env_file()` function:**

- Searches for Dockerfile first
- If Dockerfile is in a subdirectory, checks for .env at that level
- Falls back to root .env if not found at Dockerfile level
- Returns relative path for Docker compatibility

**Updated `load_env_vars_for_runtime()` function:**

- Uses the same priority logic
- Loads variables from the prioritized .env file
- Returns HashMap for OrbStack environment

### 3. Command Updates

**Updated all command files to use the new priority system:**

- `devcli-core/src/commands/start/executor.rs`
- `devcli-core/src/commands/run.rs`
- `devcli-core/src/commands/restart.rs`

**Docker environment:**

```rust
if let Ok(Some(env_file_path)) = crate::detection::find_env_file(&working_dir) {
    final_command = crate::utils::command::inject_docker_env_file(
        &final_command,
        &env_file_path  // Uses prioritized path
    );
}
```

**OrbStack environment:**

```rust
if let Ok(runtime_env_vars) = crate::detection::load_env_vars_for_runtime(&working_dir) {
    final_command = crate::utils::command::inject_orbstack_env_vars(&final_command, &runtime_env_vars);
}
```

## Examples

### Example 1: Empty Values

**Before:**

```bash
# .env file:
NODE_ENV=production
APM_SECRET=
API_KEY=

# OrbStack command:
NODE_ENV=production APM_SECRET= API_KEY= docker run myimage
```

**After:**

```bash
# .env file:
NODE_ENV=production
APM_SECRET=
API_KEY=

# OrbStack command:
NODE_ENV=production APM_SECRET=XXX API_KEY=XXX docker run myimage
```

### Example 2: Dockerfile-Level .env Priority

**Directory structure:**

```
my-app/
├── .env                    # Contains: PORT=8080
├── docker/
│   ├── Dockerfile
│   └── .env               # Contains: PORT=3000, NODE_ENV=production
└── src/
```

**Before:**

```bash
# Would use root .env (PORT=8080)
docker run --env-file .env myimage
```

**After:**

```bash
# Uses docker/.env (PORT=3000, NODE_ENV=production)
docker run --env-file docker/.env myimage

# OrbStack:
NODE_ENV=production PORT=3000 docker run myimage
```

## Testing

### New Tests Added

**Empty value handling test:**

```rust
#[test]
fn test_parse_env_file_empty_values() {
    // Tests that empty values are replaced with XXX
}
```

### Test Results

```bash
cargo test --package devcli-core parse_env_file
```

✅ 2 tests pass (parse_env_file, parse_env_file_empty_values)

```bash
cargo test --package devcli-core utils::command::tests
```

✅ 12 tests pass (all command utility tests)

## Usage

No configuration needed! The improvements work automatically:

### For Empty Values

1. Create a .env file with empty values:

```bash
NODE_ENV=production
APM_SECRET=
API_KEY=
```

2. Run your app:

```bash
devcli start myapp --env orbstack
```

3. Empty values are automatically replaced with `XXX`:

```bash
NODE_ENV=production APM_SECRET=XXX API_KEY=XXX docker run myimage
```

### For Dockerfile-Level .env

1. Create directory structure:

```
my-app/
├── .env                    # Root level
├── docker/
│   ├── Dockerfile
│   └── .env               # Dockerfile level (prioritized)
```

2. Run your app:

```bash
devcli start myapp --env docker
# or
devcli start myapp --env orbstack
```

3. The CLI automatically uses `docker/.env` instead of root `.env`

## Technical Details

### Priority Logic

1. Find Dockerfile using `find_dockerfile()` (searches up to 2 levels deep)
2. If Dockerfile found in subdirectory, check for `.env` in that directory
3. If found, use that .env file (return relative path)
4. Otherwise, fall back to root `.env` file

### Empty Value Detection

- Checks if value is empty after trimming whitespace
- Checks if value is empty after removing quotes (`""` or `''`)
- Replaces with `XXX` to make it obvious a value is missing

### Path Handling

- For Docker: Returns relative path (e.g., `docker/.env`)
- For OrbStack: Loads and parses the file, returns HashMap
- Paths are relative to working directory for Docker compatibility
