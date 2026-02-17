# Changelog

All notable changes to app-detector will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### 2026-02-17 - Hierarchical Monorepo Detection (Phase 2)

**Summary**: Full tree-based detection for monorepos. Nx workspaces are now detected independently, each getting their own `DetectionReport`. Docker is scoped to individual workspaces.

**Architecture Changes**:

1. **`DetectionReport.children`** (`src/types.rs`):
   - New `children: Vec<DetectionReport>` field (serde-skipped if empty → backward compatible JSON)
   - `add_child()`, `is_workspace_root()`, `all_reports()` methods

2. **`DetectionScope`** (`src/context.rs`):
   - Tracks recursion depth, parent paths (cycle detection), scope type (Root vs Workspace)
   - `DetectionContext` now carries a `DetectionScope`
   - `is_workspace()` helper for strategies to query their context

3. **`WorkspaceInfo`** (`src/types.rs`):
   - Structured workspace metadata (`path`, `name`, `should_detect`) in `MonorepoInfo`
   - Legacy `workspaces: Vec<String>` kept for backward compat

4. **Engine Phase 1.5** (`src/engine.rs`):
   - `detect_with_config()` now delegates to internal `detect_scoped(path, config, scope)`
   - Phase 1.5 runs between app type detection and env capability detection
   - `extract_workspaces(report)` reads `WorkspaceInfo` from `MonorepoInfo` results
   - `detect_workspaces(...)` loops workspaces, creates child scopes, recurses via `detect_scoped()`
   - `DetectionConfig` gains `enable_workspace_detection: bool` and `max_workspace_depth: usize`

5. **Nx Strategy** (`src/strategies/app_types/nx.rs`):
   - `detect_nx_workspace_info()` returns `Vec<WorkspaceInfo>` (scans `apps/`, `libs/`, `packages/`)
   - Deterministic ordering (sorted by dir name) for reproducible results

6. **Docker Scope Awareness** (`src/strategies/env_capabilities/docker.rs`):
   - At monorepo root (Nx detected + not a workspace context): only detect root-level Dockerfiles
   - At workspace level: full recursive detection (scoped to workspace path)
   - This replaces the old temporary hack with proper scope awareness

**CLI Output** (`src/main.rs`):
- `output_report_human()` recursively renders workspace tree
- Root shows "Detection Report", children show "Workspace: <path>"
- JSON output is unchanged (children included automatically)

**Test Fixtures** (`tests/fixtures/nx-monorepo/`):
- `apps/web/package.json` — React web app with 4 npm scripts
- `apps/web/Dockerfile` — Multi-stage (builder + production)
- `apps/api/package.json` — Express API with 4 npm scripts
- `apps/api/Dockerfile` — Single-stage API container

**Tests Added** (`tests/hierarchical_test.rs`):
- 10 new integration tests:
  - Nx monorepo creates child reports (2 workspaces)
  - Node.js detected per workspace (not at root)
  - Docker scoped to workspace (1 Dockerfile each, not mixed)
  - Multi-stage Docker commands in web workspace (build-builder, build-production)
  - `all_reports()` flat traversal (root + 2 children = 3)
  - `enable_workspace_detection: false` disables recursion
  - Flat projects have no children
  - Depth limit prevents deep recursion
  - JSON hierarchical serialization (children included)
  - JSON backward compat (flat project omits children)

**Result**:
```
Detection Report
Path: /monorepo

App Types:
  • nx (100%)
    Tool: nx
    Workspaces: 2

Workspaces: (2 found)

  Workspace: /monorepo/apps/web

  App Types:
    • nodejs (100%)

  Environment Capabilities:
    • docker (100%)
      Dockerfiles: 1
      Commands: 8
    • local-env (100%)
      Commands: 4

  Workspace: /monorepo/apps/api

  App Types:
    • nodejs (100%)

  Environment Capabilities:
    • docker (100%)
      Dockerfiles: 1
      Commands: 4
    • local-env (100%)
      Commands: 4
```

**Additional Polish**:
- OrbStack `can_apply()` now mirrors Docker scope awareness (prevents spurious failure at monorepo root)
- Cross-phase dependencies (e.g., `local-env` depends on `nodejs`) now log at `debug` instead of `warn` level — they are resolved at runtime via `ctx.get_result()`, not via dependency ordering

**Test Count**: 81 tests (54 unit + 10 hierarchical + 11 integration + 6 doc)

**Files Changed**:
- `src/types.rs` — WorkspaceInfo, children, add_child, is_workspace_root, all_reports
- `src/context.rs` — ScopeType, DetectionScope, DetectionContext.scope, is_workspace()
- `src/engine.rs` — detect_scoped, extract_workspaces, detect_workspaces, Phase 1.5
- `src/strategies/app_types/nx.rs` — detect_nx_workspace_info returns WorkspaceInfo
- `src/strategies/env_capabilities/docker.rs` — scope-aware can_apply
- `src/main.rs` — output_report_human hierarchical output, print_result_indented
- `tests/hierarchical_test.rs` — NEW: 10 integration tests
- `tests/fixtures/nx-monorepo/apps/web/package.json` — NEW
- `tests/fixtures/nx-monorepo/apps/web/Dockerfile` — NEW
- `tests/fixtures/nx-monorepo/apps/api/package.json` — NEW
- `tests/fixtures/nx-monorepo/apps/api/Dockerfile` — NEW

---

### Fixed

#### 2026-02-17 - Monorepo Detection Conflicts

**Problem**: When detecting an Nx monorepo, both Nx and Node.js strategies were reported, even though Nx (monorepo tool) should suppress root-level Node.js detection. Additionally, Docker was detecting ALL Dockerfiles across the monorepo and generating broken command sets.

**Impact**:
- Confusing output showing both "nx" and "nodejs" at root level
- Docker commands mixed files from different apps in the monorepo
- No way to distinguish monorepo-level detection from app-level detection

**Root Cause**:
1. Engine's `resolve_execution_order()` only logged conflicts but didn't filter them out
2. No conflict resolution rule (higher priority should win)
3. Docker wasn't aware of monorepo context

**Fix**:

1. **Conflict Resolution** (`src/engine.rs:231-271`):
   ```rust
   // Filter out conflicting strategies (higher priority wins)
   for id in applicable {
       for conflict in strategy.conflicts_with() {
           if strategy.priority() < conflict_strategy.priority() {
               // Higher priority (lower number) wins
               to_remove.insert(conflict.to_string());
           }
       }
   }
   ```

2. **Nx Conflicts with Node.js** (`src/strategies/app_types/nx.rs:112-116`):
   ```rust
   fn conflicts_with(&self) -> Vec<&str> {
       vec!["nodejs"]  // Nx suppresses root-level Node.js
   }
   ```

3. **Docker Monorepo Awareness** (`src/strategies/env_capabilities/docker.rs:32-42`):
   ```rust
   // Skip Docker detection at monorepo root level
   if ctx.get_result("nx").is_some() {
       // Only detect root-level Dockerfile, not apps/**/Dockerfile
       return ctx.file_exists("Dockerfile") || ...;
   }
   ```

**Result**:
- Nx monorepo shows only "nx", not "nodejs"
- Docker skips nested Dockerfiles in monorepos (temporary fix)
- Priority-based conflict resolution: Nx (priority 50) beats Node.js (priority 100)

**Tests**:
- Updated `test_detect_nx_monorepo` to expect Nx without Node.js
- All 71 tests passing

**TODO - Phase 2 (Hierarchical Detection)**:
This is a temporary fix. Full monorepo support requires:
1. Hierarchical `DetectionReport` (root + children for each workspace)
2. Nx triggering sub-detection for each app in `apps/` and `libs/`
3. Docker scoped to individual apps with context-aware commands
4. Per-app detection results in tree structure

See GitHub issue #XXX for hierarchical detection architecture.

**Files Changed**:
- `src/engine.rs:231-297` - Conflict resolution logic
- `src/strategies/app_types/nx.rs:112-116` - Added conflicts_with
- `src/strategies/env_capabilities/docker.rs:32-42` - Monorepo awareness
- `tests/integration_test.rs:206-209` - Updated test expectations

---

#### 2026-02-16 - OrbStack Should Reuse Docker Detection

**Problem**: OrbStack and Docker strategies were detecting the same files independently and generating different command sets. Since OrbStack is a Docker-compatible runtime (just a better macOS integration layer), it should provide the exact same commands as Docker.

**Impact**:
- OrbStack commands didn't match Docker commands
- Multi-stage Dockerfile commands missing from OrbStack
- Duplicate detection logic between strategies
- Inconsistent user experience

**Root Cause**:
OrbStack strategy (`src/strategies/env_capabilities/orbstack_env.rs`) was reimplementing Docker detection logic instead of reusing Docker's results.

**Fix**:
Made OrbStack depend on Docker and reuse its detection data:
```rust
fn depends_on(&self) -> Vec<&str> {
    vec!["docker"]  // OrbStack depends on Docker detection
}

fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
    // Reuse Docker's detection results
    let docker_result = ctx.get_result("docker")?;
    let (commands, metadata, suggested_default) = match &docker_result.data {
        DetectionData::DockerEnv(info) => {
            (info.commands.clone(), info.metadata.clone(), info.suggested_default.clone())
        }
        _ => ...
    };
    // Return same commands with orbstack_compatible flag
}
```

**Architecture Note**: OrbStack's `can_apply()` still checks for Dockerfiles directly (not for Docker result existence) because the engine evaluates all `can_apply()` checks at the start of Phase 2, before any strategies run. The `depends_on()` ensures Docker runs first, then OrbStack reuses its data.

**Tests Added**:
- `test_orbstack_with_multistage_dockerfile` - Verifies OrbStack gets stage-specific commands from Docker

**Files Changed**:
- `src/strategies/env_capabilities/orbstack_env.rs` - Reuse Docker detection
- `src/main.rs:424-436` - Added OrbStackEnv and KubernetesEnv to human-readable output
- `tests/integration_test.rs:316-353` - Added test to verify Docker and OrbStack commands match
- Tests updated to verify OrbStack matches Docker output

**CLI Output Fix**: The CLI's human-readable output was missing the `print_result` case for OrbStackEnv, so it wasn't displaying command counts. Now shows:
```
• docker (100%)
  Commands: 6
• orbstack-env (100%)
  Commands: 6
```

---

#### 2026-02-16 - Multi-Stage Dockerfile Build Commands

**Problem**: Docker strategy parsed multi-stage Dockerfiles (extracting stage names like "builder", "production") but only generated generic build commands. Users with multi-stage builds couldn't build specific stages.

**Impact**:
- Missing stage-specific build commands
- Users couldn't build intermediate stages for testing/debugging
- Metadata about stages was collected but not used

**Example**: For a Dockerfile with stages "builder" and "production", users only got:
```bash
build: docker build -t app .
```

But needed:
```bash
build: docker build -t app .
build-builder: docker build --target builder -t app:builder .
build-production: docker build --target production -t app:production .
```

**Root Cause**:
In `src/strategies/env_capabilities/docker.rs:90-108`, the strategy parsed stages (line 69) but didn't use them to generate commands.

**Fix**:
Generate stage-specific build commands:
```rust
// Generic build command
commands.insert("build".to_string(), format!("docker build {}-t app .", dockerfile_flag));

// Stage-specific build commands
if !stages.is_empty() {
    for stage in &stages {
        let stage_lower = stage.to_lowercase();
        commands.insert(
            format!("build-{}", stage_lower),
            format!("docker build {}--target {} -t app:{} .",
                    dockerfile_flag, stage, stage_lower)
        );
    }
    metadata.insert("has_stages".to_string(), serde_json::json!(true));
    metadata.insert("stages".to_string(), serde_json::json!(stages));
}
```

**Tests Added**:
- `test_multistage_dockerfile_commands` - Verifies stage-specific build commands are generated
  - Checks `build-builder` and `build-production` commands exist
  - Verifies correct `--target` flag usage
  - Validates stage metadata

**Files Changed**:
- `src/strategies/env_capabilities/docker.rs:90-120` - Added stage-specific command generation
- `src/strategies/env_capabilities/docker.rs:295-347` - Added comprehensive multi-stage test

---

#### 2026-02-16 - Glob Pattern Wildcard Matching

**Problem**: The `glob()` function in `DetectionContext` incorrectly handled wildcard patterns like `**/Dockerfile*`. It treated the trailing `*` as a literal character instead of a wildcard, causing the Docker strategy to fail detecting Dockerfiles in subdirectories.

**Impact**:
- Docker strategy could not detect `docker/Dockerfile`, `Dockerfile.dev`, or any Dockerfile variants
- Affected any strategy using wildcard glob patterns

**Root Cause**:
In `src/context.rs`, the glob implementation for `**/pattern*` patterns was checking:
```rust
path_str.ends_with("Dockerfile*")  // Literal string match, not wildcard
```

**Fix**:
Added proper wildcard handling in `src/context.rs:142-155`:
```rust
else if suffix.ends_with('*') {
    // Pattern like **/Dockerfile* - match files starting with prefix
    let prefix = suffix.trim_end_matches('*');
    path_str.split('/').any(|part| part.starts_with(prefix))
} else {
    // Pattern like **/Dockerfile - exact filename match
    path_str.split('/').any(|part| part == suffix)
}
```

**Tests Added**:
- `test_glob_wildcard_pattern` - Verifies `**/Dockerfile*` matches:
  - `docker/Dockerfile`
  - `docker/Dockerfile.dev`
  - `Dockerfile.prod`
- `test_dockerfile_in_subdirectory` - End-to-end test for Docker strategy detecting files in subdirectories

**Files Changed**:
- `src/context.rs:142-155` - Fixed glob pattern matching
- `src/context.rs:298-320` - Added test for wildcard patterns
- `src/strategies/env_capabilities/docker.rs:265-294` - Added test for subdirectory detection

---

#### 2026-02-16 - Node.js Strategy File Bloat

**Problem**: The `NodeJsStrategy` was globbing all JavaScript/TypeScript files in the project (`**/*.js`, `**/*.ts`, `**/*.jsx`, `**/*.tsx`), returning hundreds or thousands of files in `primary_files`. This was:
- Wasteful (listing every source file)
- Irrelevant (file lists aren't useful for language detection)
- Bloating output (massive JSON responses)

**Impact**:
- Slow detection for large Node.js projects
- Excessive memory usage
- Confusing output with hundreds of files listed

**Root Cause**:
In `src/strategies/app_types/nodejs.rs:59-63`, the strategy was collecting all source files:
```rust
let mut js_files = ctx.glob("**/*.js");
js_files.extend(ctx.glob("**/*.ts"));
js_files.extend(ctx.glob("**/*.jsx"));
js_files.extend(ctx.glob("**/*.tsx"));
```

**Fix**:
Changed to only track key configuration files:
```rust
// Only track key configuration files, not all source files
let key_files = vec!["package.json".into()];
```

**Rationale**:
- Language detection only needs to identify the language, not list every file
- `package.json` is sufficient to identify a Node.js project
- File counts can be added to metadata if needed, without returning full paths

**Tests Updated**:
- `test_nodejs_detection` - Now verifies only key config files are returned:
  ```rust
  assert_eq!(info.primary_files.len(), 1);
  assert!(info.primary_files[0].to_string_lossy().contains("package.json"));
  ```

**Files Changed**:
- `src/strategies/app_types/nodejs.rs:59-63` - Removed file globbing
- `src/strategies/app_types/nodejs.rs:99` - Changed to use `key_files`
- `src/strategies/app_types/nodejs.rs:179-183` - Updated test expectations

---

### Architecture Notes

#### Separation of Concerns: App Type vs Environment Capability

**App Type Strategies** (Phase 1):
- Detect WHAT the application is (language, framework, service)
- Return metadata about the application
- Should NOT list all source files
- Should NOT extract runnable commands

**Environment Capability Strategies** (Phase 2):
- Detect HOW the application can run (local, docker, kubernetes)
- Extract runnable commands based on detected app types
- Depend on Phase 1 results for context

**Example**:
- `nodejs` strategy → Detects Node.js, returns package manager, TypeScript support, dependency counts
- `local-env` strategy → Extracts `npm run dev`, `npm run test` commands from package.json

This separation ensures:
1. Clean architectural boundaries
2. Reusable app type detection
3. Environment-specific command generation
4. Better testability

---

## [0.1.0] - Initial Release

### Added
- Core strategy-based detection system
- Two-phase detection architecture (App Types → Environment Capabilities)
- 10 built-in strategies:
  - **App Types**: Nx, Rust, Node.js, Python, Redis, Traefik
  - **Environments**: Docker, OrbStack, Kubernetes, Local
- Multi-level caching (file content, file trees)
- Dependency resolution with topological sort
- 68 comprehensive tests (51 unit, 11 integration, 6 doc tests)
- Full API documentation
- Example implementations

### Performance
- Detection speed: < 100ms for typical projects
- Memory usage: < 10MB with full caching
- Handles monorepos with 1000+ files

[Unreleased]: https://github.com/yourusername/devcli/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/devcli/releases/tag/v0.1.0
