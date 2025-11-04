# Code Refactoring Design Document

## Overview

This design document outlines the systematic approach for refactoring large Rust source files in the rustycli project. The refactoring will be performed incrementally, one module at a time, with validation at each step to ensure no functionality is lost.

## Architecture

### Current State Analysis

The following files require refactoring due to size and complexity:

1. **detection_tests.rs** (1,100 lines) - Mixed test concerns
2. **commands/config.rs** (1,050 lines) - All config operations in one file
3. **detection/mod.rs** (800 lines) - Multiple detection responsibilities
4. **commands/auto_add.rs** (690 lines) - Single app vs monorepo handling
5. **config/resolver_tests.rs** (656 lines) - Large test file
6. **commands/start.rs** (608 lines) - Complex start logic

### Target Architecture

#### Module Hierarchy

```
rustycli-core/src/
├── detection/
│   ├── mod.rs                    # Public interface + DetectedApp struct
│   ├── app_types.rs             # App type detection (nodejs, python, etc.)
│   ├── nx.rs                    # Nx monorepo specific logic
│   ├── utils.rs                 # File finding helpers
│   └── environments/
│       ├── mod.rs               # Environment detection interface
│       ├── local.rs             # Local command detection
│       ├── docker.rs            # Docker detection + Dockerfile parsing
│       └── k8s.rs               # Kubernetes manifest detection
├── commands/
│   ├── config/
│   │   ├── mod.rs               # Public command interface
│   │   ├── init.rs              # Config initialization
│   │   ├── validate.rs          # Config validation
│   │   ├── list.rs              # List/show operations
│   │   ├── edit.rs              # Interactive editing
│   │   └── prompts.rs           # User interaction helpers
│   ├── auto_add/
│   │   ├── mod.rs               # Main command entry point
│   │   ├── single_app.rs        # Single app detection/addition
│   │   ├── nx_monorepo.rs       # Nx monorepo handling
│   │   ├── interactive.rs       # User prompts and selection
│   │   └── validation.rs        # App name validation
│   └── start/
│       ├── mod.rs               # Main command interface
│       ├── resolver.rs          # App resolution and validation
│       ├── dependencies.rs      # Dependency handling
│       ├── executor.rs          # Process spawning and management
│       └── logging.rs           # Log aggregation and display
└── tests/
    ├── detection/
    │   ├── mod.rs
    │   ├── app_type_tests.rs    # App type detection tests
    │   ├── environment_tests.rs # Environment detection tests
    │   ├── nx_tests.rs          # Nx monorepo tests
    │   └── integration_tests.rs # End-to-end detection tests
    └── config/
        ├── mod.rs
        ├── resolution_tests.rs   # App resolution tests
        ├── dependency_tests.rs   # Dependency chain tests
        └── validation_tests.rs   # Config validation tests
```

## Components and Interfaces

### Detection Module Refactoring

#### Public Interface (detection/mod.rs)

```rust
pub struct DetectedApp { /* existing fields */ }
pub fn detect_app(path: &Path) -> Result<DetectedApp>
pub fn detect_nx_apps(workspace_root: &Path) -> Result<Vec<DetectedApp>>
```

#### Internal Modules

- **app_types.rs**: `detect_app_type()`, `extract_app_name()`
- **environments/local.rs**: `detect_local_commands()`
- **environments/docker.rs**: `detect_docker_commands()`, dockerfile parsing
- **environments/k8s.rs**: `detect_k8s_commands()`
- **nx.rs**: `detect_single_nx_app()`, Nx-specific logic
- **utils.rs**: `find_dockerfile()`, `find_k8s_files()`, path utilities

### Config Commands Refactoring

#### Public Interface (commands/config/mod.rs)

```rust
pub async fn config_init() -> Result<()>
pub async fn config_validate() -> Result<()>
pub async fn config_list(project_filter: Option<String>, apps_only: bool) -> Result<()>
pub async fn config_show(app_name: String, project: Option<String>) -> Result<()>
pub async fn config_edit() -> Result<()>
// ... other config commands
```

#### Internal Modules

- **init.rs**: Template creation, directory setup
- **validate.rs**: Config validation logic, dependency checking
- **list.rs**: App listing, formatting, filtering
- **edit.rs**: Editor launching, file management
- **prompts.rs**: User interaction, inquire wrappers

### Auto-Add Commands Refactoring

#### Public Interface (commands/auto_add/mod.rs)

```rust
pub async fn auto_add_command(path: Option<String>) -> Result<()>
```

#### Internal Modules

- **single_app.rs**: Single app detection and addition
- **nx_monorepo.rs**: Nx workspace handling
- **interactive.rs**: User selection, multi-select prompts
- **validation.rs**: App name validation, conflict checking

### Start Command Refactoring

#### Public Interface (commands/start/mod.rs)

```rust
pub struct StartCommandArgs { /* existing fields */ }
pub async fn start_command(args: StartCommandArgs) -> Result<()>
```

#### Internal Modules

- **resolver.rs**: App resolution, environment validation
- **dependencies.rs**: Dependency chain resolution, missing dependency handling
- **executor.rs**: Process spawning, parallel execution
- **logging.rs**: Log file management, output streaming

## Data Models

### Shared Types

All existing data structures will be preserved:

- `DetectedApp` - remains in detection/mod.rs
- `StartCommandArgs` - remains in commands/start/mod.rs
- Config types - remain in their current locations

### Internal Types

New internal types may be introduced for better organization:

- `DetectionContext` - shared state during detection
- `ValidationResult` - structured validation feedback
- `ExecutionPlan` - planned process execution steps

## Error Handling

### Error Propagation

- All existing error types and handling patterns will be preserved
- Internal modules will use the same `Result<T>` pattern
- Error context will be maintained through the call chain

### Validation Errors

- Config validation errors will be collected and reported together
- Detection errors will provide helpful context about what was attempted
- User input errors will provide clear guidance for correction

## Testing Strategy

### Test Organization

- **Unit Tests**: Each module will have focused unit tests
- **Integration Tests**: Cross-module functionality testing
- **Regression Tests**: Ensure existing behavior is preserved

### Test Migration Strategy

1. **Preserve Existing Tests**: All current tests will be moved, not rewritten
2. **Organize by Feature**: Tests grouped by the functionality they validate
3. **Maintain Coverage**: No reduction in test coverage during refactoring

### Validation Approach

- Build validation after each module refactoring
- Test suite execution after each module refactoring
- Clippy and formatting validation
- Integration test validation

## Implementation Phases

### Phase 1: Detection Module (Lowest Risk)

- Split detection/mod.rs into focused modules
- Move detection tests to organized structure
- Validate all detection functionality works

### Phase 2: Config Commands (Medium Risk)

- Split commands/config.rs into command-specific modules
- Preserve all CLI interfaces
- Validate all config operations work

### Phase 3: Auto-Add Command (Medium Risk)

- Split commands/auto_add.rs by functionality
- Preserve interactive flows
- Validate both single app and monorepo flows

### Phase 4: Start Command (Highest Risk)

- Split commands/start.rs by responsibility
- Preserve complex dependency and execution logic
- Validate parallel execution and logging

### Phase 5: Test Organization (Lowest Risk)

- Move remaining large test files to organized structure
- Ensure all tests continue to pass
- Validate test discovery and execution

## Migration Strategy

### File Movement Process

1. **Create Target Module Structure**: Create new directories and mod.rs files
2. **Move Functions**: Move related functions to appropriate modules
3. **Update Imports**: Update all import statements
4. **Add Re-exports**: Add pub use statements to maintain API compatibility
5. **Validate**: Ensure build and tests pass

### Dependency Management

- Use `pub use` re-exports to maintain existing APIs
- Update internal imports to use new module structure
- Ensure external crates continue to work without changes

### Rollback Strategy

- Each phase will be committed separately
- Git branches will be used for each major refactoring
- Rollback possible at any phase boundary
- Automated validation prevents broken intermediate states

## Performance Considerations

### Compilation Impact

- Module splitting may slightly increase compilation time due to more files
- Parallel compilation should offset this impact
- No runtime performance impact expected

### Memory Usage

- No change in runtime memory usage
- Slightly more metadata due to additional modules
- Overall impact negligible

## Security Considerations

- No security-sensitive code is being modified
- File permissions and access patterns remain unchanged
- No new external dependencies introduced
- Existing security boundaries preserved
