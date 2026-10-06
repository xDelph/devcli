# Examples Collection

Common usage patterns and recipes for app-detector.

## Table of Contents

1. [Basic Detection](#basic-detection)
2. [Filtering Results](#filtering-results)
3. [Command Extraction](#command-extraction)
4. [Environment Files](#environment-files)
5. [Custom Strategies](#custom-strategies)
6. [Error Handling](#error-handling)
7. [CLI Tools](#cli-tools)
8. [Integration Examples](#integration-examples)

## Basic Detection

### Simple Project Analysis

```rust
use app_detector::{DetectionEngine, StrategyRegistry};

fn main() -> anyhow::Result<()> {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let report = engine.detect(".")?;

    println!("Detected {} technologies", report.results.len());

    for result in &report.results {
        println!("- {} ({})", result.strategy_id, result.confidence);
    }

    Ok(())
}
```

### Get App Name

```rust
let report = engine.detect("/path/to/project")?;

match report.app_name() {
    Some(name) => println!("📦 {}", name),
    None => println!("No package name found"),
}
```

### Primary Language Detection

```rust
let report = engine.detect(".")?;

if let Some(lang) = report.primary_language() {
    println!("Primary language: {}", lang.strategy_id);

    if let DetectionData::Language(info) = &lang.data {
        println!("  Name: {}", info.name);
        if let Some(version) = &info.version {
            println!("  Version: {}", version);
        }
        println!("  Files: {:?}", info.primary_files);
    }
}
```

## Filtering Results

### By Category

```rust
use app_detector::types::{AppTypeCategory, EnvCapabilityCategory};

// Get all languages
for lang in report.languages() {
    println!("Language: {}", lang.strategy_id);
}

// Get all frameworks
for framework in report.frameworks() {
    println!("Framework: {}", framework.strategy_id);
}

// Get all monorepos
for monorepo in report.monorepos() {
    println!("Monorepo: {}", monorepo.strategy_id);
}

// Get all services
for service in report.services() {
    println!("Service: {}", service.strategy_id);
}

// Get all environment capabilities
for env in report.env_capabilities() {
    println!("Environment: {}", env.strategy_id);
}
```

### By Specific Category

```rust
// Get specific app type category
for result in report.by_app_type(&AppTypeCategory::Language) {
    println!("Language: {}", result.strategy_id);
}

// Get specific environment capability
for result in report.by_env_capability(&EnvCapabilityCategory::Docker) {
    println!("Docker: {}", result.strategy_id);
}
```

### Check for Specific Technology

```rust
if report.has("docker") {
    println!("✅ Docker support detected");
}

if report.has("kubernetes") {
    println!("✅ Kubernetes manifests found");
}

if report.has("nodejs") && report.has("typescript") {
    println!("✅ TypeScript + Node.js project");
}
```

## Command Extraction

### Local Development Commands

```rust
use app_detector::types::{EnvCapabilityCategory, DetectionData};

for result in report.by_env_capability(&EnvCapabilityCategory::Local) {
    if let DetectionData::LocalEnv(info) = &result.data {
        println!("Local commands:");
        for (name, cmd) in &info.commands {
            println!("  {}: {}", name, cmd);
        }

        if let Some(default) = &info.suggested_default {
            println!("\nSuggested: {}", default);
        }
    }
}
```

### Docker Commands

```rust
for result in report.by_env_capability(&EnvCapabilityCategory::Docker) {
    if let DetectionData::DockerEnv(info) = &result.data {
        println!("\n🐳 Docker Information:");
        println!("  Base images: {:?}", info.base_images);
        println!("  Build stages: {:?}", info.stages);
        println!("  Exposed ports: {:?}", info.exposed_ports);

        println!("\nCommands:");
        for (name, cmd) in &info.commands {
            let marker = if Some(name) == info.suggested_default.as_ref() {
                "⭐"
            } else {
                "  "
            };
            println!("  {} {}: {}", marker, name, cmd);
        }
    }
}
```

### Kubernetes Commands

```rust
for result in report.by_env_capability(&EnvCapabilityCategory::Kubernetes) {
    if let DetectionData::KubernetesEnv(info) = &result.data {
        println!("\n☸️  Kubernetes:");
        println!("  Manifests: {:?}", info.manifests);
        println!("  Helm charts: {:?}", info.helm_charts);

        println!("\nCommands:");
        for (name, cmd) in &info.commands {
            println!("  {}: {}", name, cmd);
        }
    }
}
```

### Get Runnable Command

```rust
fn get_start_command(report: &DetectionReport) -> Option<String> {
    // Try local first
    for result in report.by_env_capability(&EnvCapabilityCategory::Local) {
        if let DetectionData::LocalEnv(info) = &result.data {
            if let Some(default) = &info.suggested_default {
                return info.commands.get(default).cloned();
            }
            // Fallback to first command
            return info.commands.values().next().cloned();
        }
    }

    // Try Docker
    for result in report.by_env_capability(&EnvCapabilityCategory::Docker) {
        if let DetectionData::DockerEnv(info) = &result.data {
            if let Some(default) = &info.suggested_default {
                return info.commands.get(default).cloned();
            }
        }
    }

    None
}
```

## Custom Strategies

### Minimal Custom Strategy

```rust
use app_detector::{
    DetectionStrategy, DetectionContext, DetectionResult,
    StrategyCategory, DetectionData,
};
use app_detector::types::{AppTypeCategory, LanguageInfo};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default)]
struct PhpStrategy;

impl DetectionStrategy for PhpStrategy {
    fn id(&self) -> &str {
        "php"
    }

    fn name(&self) -> &str {
        "PHP"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::Language)
    }

    fn priority(&self) -> usize {
        100
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.file_exists("composer.json")
    }

    fn detect(&self, ctx: &DetectionContext) -> app_detector::Result<DetectionResult> {
        let content = ctx.read_file("composer.json")?;

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::Language(LanguageInfo {
                name: "PHP".to_string(),
                version: None,
                version_source: None,
                primary_files: vec![PathBuf::from("composer.json")],
                total_lines: None,
                metadata: HashMap::new(),
            }),
            suggested_strategies: vec![],
        })
    }
}

// Use it
let mut registry = StrategyRegistry::new();
registry.register(Box::new(PhpStrategy));
```

### Strategy with Dependencies

```rust
#[derive(Default)]
struct LaravelStrategy;

impl DetectionStrategy for LaravelStrategy {
    fn id(&self) -> &str {
        "laravel"
    }

    fn name(&self) -> &str {
        "Laravel"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::Framework)
    }

    fn priority(&self) -> usize {
        200
    }

    fn depends_on(&self) -> Vec<&str> {
        vec!["php"]  // Requires PHP to be detected first
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.file_exists("artisan") && ctx.file_exists("composer.json")
    }

    fn detect(&self, ctx: &DetectionContext) -> app_detector::Result<DetectionResult> {
        // Implementation...
        todo!()
    }
}
```

## Error Handling

### Graceful Degradation

```rust
use anyhow::Context;

fn analyze_project(path: &str) {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    match engine.detect(path) {
        Ok(report) => {
            println!("✅ Detection successful");
            print_report(&report);
        }
        Err(e) => {
            eprintln!("❌ Detection failed: {}", e);

            // Print error chain
            for cause in e.chain().skip(1) {
                eprintln!("  Caused by: {}", cause);
            }

            // Provide helpful message
            if path == "." {
                eprintln!("\nTip: Make sure you're in a project directory");
            } else {
                eprintln!("\nTip: Check that the path exists and is readable");
            }
        }
    }
}
```

### Validate Path First

```rust
use std::path::Path;

fn detect_with_validation(path: &str) -> anyhow::Result<DetectionReport> {
    let path = Path::new(path);

    if !path.exists() {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    if !path.is_dir() {
        anyhow::bail!("Path is not a directory: {}", path.display());
    }

    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    engine.detect(path)
        .context("Failed to detect project type")
}
```

## CLI Tools

### Basic CLI

```rust
use clap::Parser;

#[derive(Parser)]
struct Args {
    /// Path to project directory
    #[arg(default_value = ".")]
    path: String,

    /// Show detailed information
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);

    let report = engine.detect(&args.path)?;

    if args.verbose {
        println!("{:#?}", report);
    } else {
        print_summary(&report);
    }

    Ok(())
}

fn print_summary(report: &DetectionReport) {
    if let Some(name) = report.app_name() {
        println!("📦 {}", name);
    }

    if let Some(lang) = report.primary_language() {
        println!("🔤 {}", lang.strategy_id);
    }

    println!("\n🚀 Environments:");
    for env in report.env_capabilities() {
        println!("  - {}", env.strategy_id);
    }
}
```

### JSON Output

```rust
use serde_json;

#[derive(Parser)]
struct Args {
    path: String,

    /// Output format
    #[arg(short, long, default_value = "text")]
    format: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);
    let report = engine.detect(&args.path)?;

    match args.format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&report)?;
            println!("{}", json);
        }
        "text" => {
            print_summary(&report);
        }
        _ => {
            anyhow::bail!("Unknown format: {}", args.format);
        }
    }

    Ok(())
}
```

## Integration Examples

### Build Tool Integration

```rust
/// Auto-detect and run the project
fn auto_run(project_dir: &str) -> anyhow::Result<()> {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);
    let report = engine.detect(project_dir)?;

    // Get start command
    let cmd = get_start_command(&report)
        .ok_or_else(|| anyhow!("No runnable command found"))?;

    println!("Running: {}", cmd);

    // Execute command
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .current_dir(project_dir)
        .status()?;

    Ok(())
}
```

### CI/CD Pipeline Detection

```rust
fn generate_ci_config(project_dir: &str) -> anyhow::Result<String> {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);
    let report = engine.detect(project_dir)?;

    let mut config = String::new();

    // Detect language for base image
    if let Some(lang) = report.primary_language() {
        match lang.strategy_id.as_str() {
            "nodejs" => config.push_str("image: node:18\n"),
            "rust" => config.push_str("image: rust:latest\n"),
            "python" => config.push_str("image: python:3.11\n"),
            _ => {}
        }
    }

    // Add build steps
    config.push_str("\nscript:\n");

    for result in report.by_env_capability(&EnvCapabilityCategory::Local) {
        if let DetectionData::LocalEnv(info) = &result.data {
            if let Some(build) = info.commands.get("build") {
                config.push_str(&format!("  - {}\n", build));
            }
            if let Some(test) = info.commands.get("test") {
                config.push_str(&format!("  - {}\n", test));
            }
        }
    }

    Ok(config)
}
```

### IDE Plugin

```rust
struct ProjectContext {
    path: String,
    technologies: Vec<String>,
    commands: HashMap<String, String>,
}

fn analyze_for_ide(project_path: &str) -> anyhow::Result<ProjectContext> {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);
    let report = engine.detect(project_path)?;

    let technologies = report.results
        .iter()
        .map(|r| r.strategy_id.clone())
        .collect();

    let mut commands = HashMap::new();
    for result in report.by_env_capability(&EnvCapabilityCategory::Local) {
        if let DetectionData::LocalEnv(info) = &result.data {
            commands.extend(info.commands.clone());
        }
    }

    Ok(ProjectContext {
        path: project_path.to_string(),
        technologies,
        commands,
    })
}
```

## Performance Tips

### Reuse Engine

```rust
// Good: Reuse engine for multiple detections
let registry = StrategyRegistry::with_defaults();
let engine = DetectionEngine::new(registry);

for project in projects {
    let report = engine.detect(&project)?;
    // Process report...
}

// Bad: Create new engine each time
for project in projects {
    let registry = StrategyRegistry::with_defaults();
    let engine = DetectionEngine::new(registry);
    let report = engine.detect(&project)?;
}
```

### Custom Registry for Specific Use Cases

```rust
// If you only need language detection
let mut registry = StrategyRegistry::new();
registry.register(Box::new(RustStrategy));
registry.register(Box::new(NodeJsStrategy));
registry.register(Box::new(PythonStrategy));

let engine = DetectionEngine::new(registry);

// Faster than with_defaults() which includes all strategies
```

## Next Steps

- [Getting Started Guide](getting-started.md) for basics
- [Strategy Development](strategy-development.md) for custom strategies
- [Architecture](architecture.md) for internals
- [API Reference](https://docs.rs/app-detector) for full docs
