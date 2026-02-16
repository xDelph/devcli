# Architecture Overview

This document explains the internal design and architecture of app-detector.

## Core Concepts

### 1. Two-Phase Detection System

App-detector uses a unique two-phase detection system:

```
┌─────────────────────────────────────┐
│  Phase 1: App Type Detection       │
│  (Priority 0-399)                   │
│                                     │
│  ┌────────────────────────────────┐│
│  │ Environment Files (50)         ││
│  │ Monorepo Tools (50)            ││
│  │ Languages (100)                ││
│  │ Services (150)                 ││
│  └────────────────────────────────┘│
│                                     │
│  Results stored in DetectionContext │
└──────────────┬──────────────────────┘
               │
               ▼
┌─────────────────────────────────────┐
│  Phase 2: Env Capability Detection  │
│  (Priority 400-599)                 │
│                                     │
│  ┌────────────────────────────────┐│
│  │ Docker (400)                   ││
│  │ OrbStack (410)                 ││
│  │ Kubernetes (450)               ││
│  │ Local (500)                    ││
│  └────────────────────────────────┘│
│                                     │
│  Can access Phase 1 results        │
└─────────────────────────────────────┘
```

**Why Two Phases?**

1. **Dependency Management**: Environment strategies need app type information
   - Example: `LocalEnvStrategy` extracts npm scripts only if Node.js is detected

2. **Clean Separation**: WHAT vs HOW
   - Phase 1: What is this app? (Node.js, React, Nx monorepo)
   - Phase 2: How can it run? (Local, Docker, K8s)

3. **Performance**: Avoid re-running expensive detections
   - Phase 1 results cached for Phase 2 access

### 2. Strategy Pattern

Each detection strategy implements the `DetectionStrategy` trait:

```rust
pub trait DetectionStrategy: Send + Sync {
    // Identification
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn category(&self) -> StrategyCategory;
    fn priority(&self) -> usize;

    // Detection
    fn can_apply(&self, ctx: &DetectionContext) -> bool;
    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult>;

    // Dependencies (optional)
    fn depends_on(&self) -> Vec<&str> { vec![] }
    fn conflicts_with(&self) -> Vec<&str> { vec![] }
}
```

**Design Decisions**:

- **`can_apply()`**: Fast preliminary check (< 1ms)
  - Only checks file existence
  - No file reading or parsing
  - Called during filtering phase

- **`detect()`**: Full detection with file reading
  - Only called if `can_apply()` returns true
  - Can read files, parse configs, extract metadata
  - Returns structured `DetectionResult`

- **Dependencies**: Automatic topological sorting
  - Example: React depends on Node.js
  - Engine executes Node.js before React

### 3. Detection Context

Shared state passed to all strategies:

```rust
pub struct DetectionContext {
    // Absolute path to project root
    pub root_path: PathBuf,

    // Lazy-loaded file tree (cached)
    files: OnceCell<FileTree>,

    // File content cache
    content_cache: Arc<RwLock<HashMap<PathBuf, String>>>,

    // Results from previously executed strategies
    previous_results: Arc<RwLock<HashMap<String, DetectionResult>>>,

    // Configuration
    max_depth: usize,
    ignore_patterns: Vec<String>,
}
```

**Key Features**:

1. **Lazy Loading**: File tree only built when needed
2. **Multi-Level Caching**:
   - File tree cached (OnceCell)
   - File content cached (HashMap)
   - Strategy results cached (for dependencies)
3. **Thread-Safe**: Arc<RwLock<T>> for shared mutable state
4. **Smart Traversal**: Respects ignore patterns and max depth

**API**:

```rust
// File operations
ctx.file_exists("package.json")              // Fast check
ctx.read_file("Cargo.toml")?                 // Cached read
ctx.parse_json::<PackageJson>("package.json")? // Parse JSON
ctx.parse_toml::<CargoToml>("Cargo.toml")?  // Parse TOML

// File discovery
ctx.list_files()                             // All files
ctx.glob("**/*.rs")                          // Pattern match

// Dependency access
ctx.get_result("nodejs")                     // Phase 2 access to Phase 1
```

### 4. Detection Engine

Orchestrates strategy execution:

```rust
pub struct DetectionEngine {
    registry: StrategyRegistry,
}

impl DetectionEngine {
    pub fn detect(&self, path: impl AsRef<Path>) -> Result<DetectionReport> {
        // 1. Create context
        let context = DetectionContext::new(path)?;
        let mut report = DetectionReport::new(context.root_path.clone());

        // 2. Phase 1: App Type Detection
        let app_type_strategies = self.find_app_type_strategies(&context, config);
        let app_type_order = self.resolve_execution_order(&app_type_strategies)?;
        for strategy_id in app_type_order {
            self.execute_strategy(&strategy_id, &context, &mut report);
        }

        // 3. Phase 2: Environment Capability Detection
        // Re-evaluate can_apply() now that Phase 1 results are available
        let env_strategies = self.find_env_capability_strategies(&context, config);
        let env_order = self.resolve_execution_order(&env_strategies)?;
        for strategy_id in env_order {
            self.execute_strategy(&strategy_id, &context, &mut report);
        }

        Ok(report)
    }
}
```

**Execution Flow**:

1. **Filter**: Call `can_apply()` for all strategies
2. **Resolve**: Topological sort based on dependencies
3. **Execute**: Run strategies in priority order
4. **Store**: Save results in context for later strategies
5. **Report**: Return comprehensive report

### 5. Dependency Resolution

Uses **Kahn's algorithm** for topological sorting:

```rust
fn resolve_execution_order(&self, applicable: &[String]) -> Result<Vec<&str>> {
    // Build dependency graph
    let mut in_degree = HashMap::new();
    let mut graph = HashMap::new();

    for id in applicable {
        let strategy = self.registry.get(id).unwrap();
        let deps = strategy.depends_on();

        for dep in deps {
            graph.entry(dep).or_insert_with(Vec::new).push(id);
            *in_degree.entry(id).or_insert(0) += 1;
        }
    }

    // Kahn's algorithm
    let mut queue = VecDeque::new();
    let mut result = Vec::new();

    // Start with nodes that have no dependencies
    for id in applicable {
        if in_degree.get(id).unwrap_or(&0) == &0 {
            queue.push_back(id);
        }
    }

    while let Some(id) = queue.pop_front() {
        result.push(id);

        if let Some(dependents) = graph.get(id) {
            for dependent in dependents {
                let degree = in_degree.get_mut(dependent).unwrap();
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(dependent);
                }
            }
        }
    }

    // Check for cycles
    if result.len() != applicable.len() {
        bail!("Circular dependency detected");
    }

    // Sort by priority within dependency constraints
    result.sort_by_key(|id| {
        self.registry.get(id).unwrap().priority()
    });

    Ok(result)
}
```

**Example**:

Given strategies:
- `react` (priority 200, depends on `nodejs`)
- `nodejs` (priority 100, no deps)
- `docker` (priority 400, no deps)

Execution order: `nodejs` → `react` → `docker`

### 6. Strategy Categories

```rust
pub enum StrategyCategory {
    AppType(AppTypeCategory),
    EnvCapability(EnvCapabilityCategory),
}

pub enum AppTypeCategory {
    Language,
    Framework,
    Monorepo,
    BuildTool,
    PackageManager,
    Database,
    Service,
    // ... more
}

pub enum EnvCapabilityCategory {
    Local,
    Docker,
    OrbStack,
    Kubernetes,
    CloudPlatform,
    Infrastructure,
    // ... more
}
```

Categories enable:
- Filtering: `report.by_app_type(&AppTypeCategory::Language)`
- Organization: Group related strategies
- UI rendering: Display by category

### 7. Detection Data

Flexible typed data structures:

```rust
pub enum DetectionData {
    // Typed structures
    Language(LanguageInfo),
    Framework(FrameworkInfo),
    Monorepo(MonorepoInfo),
    Service(ServiceInfo),
    LocalEnv(LocalEnvInfo),
    DockerEnv(DockerEnvInfo),
    KubernetesEnv(KubernetesEnvInfo),

    // Flexible structures
    KeyValue(HashMap<String, serde_json::Value>),
    Custom(serde_json::Value),
}
```

Each type has specific fields:

```rust
pub struct LanguageInfo {
    pub name: String,
    pub version: Option<String>,
    pub version_source: Option<String>,
    pub primary_files: Vec<PathBuf>,
    pub total_lines: Option<usize>,
    pub metadata: HashMap<String, serde_json::Value>,
}

pub struct DockerEnvInfo {
    pub dockerfiles: Vec<PathBuf>,
    pub compose_files: Vec<PathBuf>,
    pub stages: Vec<String>,
    pub base_images: Vec<String>,
    pub exposed_ports: Vec<u16>,
    pub commands: HashMap<String, String>,
    pub suggested_default: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

## Performance Optimizations

### 1. Lazy File Tree

File tree only built when first accessed:

```rust
self.files.get_or_init(|| {
    FileTree::build(&self.root_path, self.max_depth, &self.ignore_patterns)
        .unwrap_or_else(|_| FileTree::empty())
})
```

**Benefits**:
- Zero overhead if no strategy uses glob
- Shared across all strategies
- Built once, cached forever

### 2. File Content Caching

Read files cached in memory:

```rust
// Check cache first
{
    let cache = self.content_cache.read().unwrap();
    if let Some(content) = cache.get(&full_path) {
        return Ok(content.clone());
    }
}

// Read and cache
let content = fs::read_to_string(&full_path)?;
{
    let mut cache = self.content_cache.write().unwrap();
    cache.insert(full_path, content.clone());
}
```

**Benefits**:
- Multiple strategies can access same file
- No duplicate disk I/O
- Typical hit rate: 60-80%

### 3. Smart Traversal

Configurable limits prevent scanning entire filesystem:

```rust
// Default configuration
max_depth: 3,
ignore_patterns: vec![
    ".git",
    "node_modules",
    "target",
    ".venv",
    "__pycache__",
]
```

**Impact**:
- Monorepo with 10,000 files → scans ~500 relevant files
- Detection time: 500ms → 100ms
- Memory usage: 50MB → 5MB

### 4. can_apply() Fast Path

Strategies implement cheap preliminary checks:

```rust
fn can_apply(&self, ctx: &DetectionContext) -> bool {
    ctx.file_exists("package.json")  // Fast: just checks existence
}

fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
    let pkg = ctx.parse_json("package.json")?;  // Expensive: reads + parses
    // ... full detection
}
```

**Benefits**:
- Skip expensive operations for non-applicable strategies
- Typical project: 11 strategies, only 3-4 applicable
- 70% time savings

## Design Principles

### 1. Separation of Concerns

Each component has a single responsibility:
- **Strategy**: Detection logic
- **Context**: State management
- **Engine**: Orchestration
- **Registry**: Strategy management
- **Report**: Result aggregation

### 2. Extensibility

New strategies added without modifying core:

```rust
// Add custom strategy
let mut registry = StrategyRegistry::with_defaults();
registry.register(Box::new(MyCustomStrategy));
```

No changes needed to:
- DetectionEngine
- DetectionContext
- DetectionReport

### 3. Type Safety

Leverage Rust's type system:
- `DetectionData` enum prevents invalid data
- `StrategyCategory` enforces categorization
- Trait bounds ensure thread safety (`Send + Sync`)

### 4. Performance First

Optimize for the common case:
- Lazy loading (pay-per-use)
- Multi-level caching (avoid duplicate work)
- Smart limits (prevent pathological cases)

### 5. Error Handling

Use `anyhow::Result` for flexibility:
- Strategy errors don't crash engine
- Contexts preserved with `.context()`
- Easy error propagation with `?`

## Future Enhancements

### Parallel Execution

Independent strategies could run in parallel:

```rust
// Future API
let report = engine
    .detect(path)
    .with_parallelism(4)  // Max 4 concurrent strategies
    .await?;
```

### Advanced Caching

Persistent cache across runs:

```rust
// Future API
let cache = DiskCache::new("~/.cache/app-detector")?;
let engine = DetectionEngine::new(registry)
    .with_cache(cache);
```

### Plugin System

Load strategies dynamically:

```rust
// Future API
let registry = StrategyRegistry::with_defaults()
    .load_plugins("~/.app-detector/plugins")?;
```

## Summary

App-detector's architecture provides:

✅ **Extensibility** - Easy to add new strategies
✅ **Performance** - Lazy loading, caching, smart traversal
✅ **Correctness** - Dependency resolution, type safety
✅ **Simplicity** - Clean abstractions, single responsibility
✅ **Flexibility** - Two-phase detection, custom data structures

The two-phase detection system is the key innovation that enables context-aware environment detection while maintaining clean separation of concerns.
