# Getting Started with app-detector

This guide will walk you through installing and using app-detector in your Rust project.

## Installation

Add app-detector to your `Cargo.toml`:

```toml
[dependencies]
app-detector = "0.1"
```

Or use cargo add:

```bash
cargo add app-detector
```

## Basic Usage

### 1. Detect a Project

The simplest way to use app-detector is with the default strategy registry:

```rust
use app_detector::{DetectionEngine, StrategyRegistry};

fn main() -> anyhow::Result<()> {
    // Create engine with all built-in strategies
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    // Detect current directory
    let report = engine.detect(".")?;

    println!("Detection complete!");
    println!("Found {} results", report.results.len());

    Ok(())
}
```

### 2. Inspect Results

The `DetectionReport` provides several helper methods:

```rust
// Get app name
if let Some(name) = report.app_name() {
    println!("App name: {}", name);
}

// Get primary language
if let Some(lang) = report.primary_language() {
    println!("Primary language: {}", lang.strategy_id);
    println!("Confidence: {}", lang.confidence);
}

// Get all languages
for lang in report.languages() {
    println!("Language: {}", lang.strategy_id);
}

// Get all frameworks
for framework in report.frameworks() {
    println!("Framework: {}", framework.strategy_id);
}

// Check if specific tech was detected
if report.has("docker") {
    println!("Docker support detected!");
}

// Get specific result
if let Some(result) = report.get("nodejs") {
    println!("Node.js detected with confidence: {}", result.confidence);
}
```

### 3. Access Typed Data

Each detection result contains structured data:

```rust
use app_detector::types::{DetectionData, AppTypeCategory};

// Get all language detections
for result in report.by_app_type(&AppTypeCategory::Language) {
    match &result.data {
        DetectionData::Language(info) => {
            println!("Language: {}", info.name);
            println!("Version: {:?}", info.version);
            println!("Files: {:?}", info.primary_files);

            // Access metadata
            if let Some(pkg_name) = info.metadata.get("package_name") {
                println!("Package: {}", pkg_name);
            }
        }
        _ => {}
    }
}
```

### 4. Extract Commands

Environment capability strategies extract runnable commands:

```rust
use app_detector::types::EnvCapabilityCategory;

// Get local development commands
for result in report.by_env_capability(&EnvCapabilityCategory::Local) {
    if let DetectionData::LocalEnv(info) = &result.data {
        println!("\nLocal commands:");
        for (name, cmd) in &info.commands {
            println!("  {}: {}", name, cmd);
        }

        if let Some(default) = &info.suggested_default {
            println!("  Default: {}", default);
        }
    }
}

// Get Docker commands
for result in report.by_env_capability(&EnvCapabilityCategory::Docker) {
    if let DetectionData::DockerEnv(info) = &result.data {
        println!("\nDocker info:");
        println!("  Base images: {:?}", info.base_images);
        println!("  Stages: {:?}", info.stages);
        println!("  Ports: {:?}", info.exposed_ports);

        println!("\nDocker commands:");
        for (name, cmd) in &info.commands {
            println!("  {}: {}", name, cmd);
        }
    }
}
```

## Common Patterns

### Pattern 1: CLI Tool

Build a CLI tool that analyzes projects:

```rust
use app_detector::{DetectionEngine, StrategyRegistry};
use std::env;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let path = args.get(1).map(|s| s.as_str()).unwrap_or(".");

    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    println!("Analyzing: {}\n", path);
    let report = engine.detect(path)?;

    if let Some(name) = report.app_name() {
        println!("📦 {}", name);
    }

    if let Some(lang) = report.primary_language() {
        println!("🔤 {}", lang.strategy_id);
    }

    println!("\n🚀 Available commands:");
    for env in report.env_capabilities() {
        println!("  [{}]", env.strategy_id);
    }

    Ok(())
}
```

### Pattern 2: Build Tool Integration

Auto-detect how to build/run a project:

```rust
use app_detector::{DetectionEngine, StrategyRegistry};
use app_detector::types::{EnvCapabilityCategory, DetectionData};

fn get_start_command(project_path: &str) -> anyhow::Result<Option<String>> {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);
    let report = engine.detect(project_path)?;

    // Try local commands first
    for result in report.by_env_capability(&EnvCapabilityCategory::Local) {
        if let DetectionData::LocalEnv(info) = &result.data {
            if let Some(default) = &info.suggested_default {
                if let Some(cmd) = info.commands.get(default) {
                    return Ok(Some(cmd.clone()));
                }
            }
        }
    }

    // Fall back to Docker
    for result in report.by_env_capability(&EnvCapabilityCategory::Docker) {
        if let DetectionData::DockerEnv(info) = &result.data {
            if let Some(default) = &info.suggested_default {
                if let Some(cmd) = info.commands.get(default) {
                    return Ok(Some(cmd.clone()));
                }
            }
        }
    }

    Ok(None)
}
```

### Pattern 3: Custom Registry

Create a registry with only specific strategies:

```rust
use app_detector::{DetectionEngine, StrategyRegistry};
use app_detector::strategies::{RustStrategy, NodeJsStrategy, DockerStrategy};

let mut registry = StrategyRegistry::new();

// Add only the strategies you need
registry.register(Box::new(RustStrategy));
registry.register(Box::new(NodeJsStrategy));
registry.register(Box::new(DockerStrategy));

let engine = DetectionEngine::new(registry);
let report = engine.detect(".")?;
```

## Configuration

### Ignore Patterns

The `DetectionContext` has default ignore patterns:
- `.git`
- `node_modules`
- `target`
- `.venv`
- `__pycache__`

These are automatically excluded from file traversal to improve performance.

### Max Depth

File tree traversal has a default max depth of 3 levels to prevent scanning very deep directory structures.

### Caching

App-detector uses multi-level caching:

1. **File Tree Cache**: Lazy-loaded and cached per context
2. **File Content Cache**: Read files cached in memory
3. **Result Cache**: Strategy results stored for dependency access

## Error Handling

App-detector uses `anyhow::Result` for error handling:

```rust
use app_detector::{DetectionEngine, StrategyRegistry};

match engine.detect(path) {
    Ok(report) => {
        // Process report
        println!("Success!");
    }
    Err(e) => {
        eprintln!("Detection failed: {}", e);
        // Check error context
        eprintln!("Context: {:?}", e);
    }
}
```

Common errors:
- **Path does not exist**: Invalid project path
- **Path is not a directory**: Path points to a file
- **IO errors**: Permission issues, filesystem errors
- **Parse errors**: Invalid JSON/TOML in config files

## Next Steps

- **[Architecture Overview](architecture.md)** - Understand how app-detector works
- **[Strategy Development](strategy-development.md)** - Create custom strategies
- **[Examples](examples.md)** - More usage patterns
- **[API Reference](https://docs.rs/app-detector)** - Full API documentation

## Quick Reference

```rust
// Create engine
let registry = StrategyRegistry::with_defaults();
let engine = DetectionEngine::new(registry);

// Detect project
let report = engine.detect(path)?;

// Helper methods
report.app_name()                    // Get app name
report.primary_language()            // Get primary language
report.languages()                   // All languages
report.frameworks()                  // All frameworks
report.monorepos()                   // All monorepos
report.services()                    // All services
report.env_capabilities()            // All environments
report.has("strategy-id")            // Check if detected
report.get("strategy-id")            // Get specific result
report.by_app_type(&category)        // Filter by app type
report.by_env_capability(&category)  // Filter by environment
```
