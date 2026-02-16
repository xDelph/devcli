# Strategy Development Guide

This guide walks you through creating custom detection strategies for app-detector.

## Table of Contents

1. [Strategy Basics](#strategy-basics)
2. [Implementation Steps](#implementation-steps)
3. [Best Practices](#best-practices)
4. [Testing Strategies](#testing-strategies)
5. [Real-World Examples](#real-world-examples)
6. [Common Patterns](#common-patterns)

## Strategy Basics

### What is a Strategy?

A strategy is a self-contained detector that:
- Identifies ONE specific technology (language, framework, tool, etc.)
- Decides if it applies to a project (`can_apply`)
- Extracts detailed information (`detect`)
- Returns structured data

### Strategy Trait

```rust
pub trait DetectionStrategy: Send + Sync {
    // Required methods
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn category(&self) -> StrategyCategory;
    fn priority(&self) -> usize;
    fn can_apply(&self, ctx: &DetectionContext) -> bool;
    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult>;

    // Optional methods
    fn depends_on(&self) -> Vec<&str> { vec![] }
    fn conflicts_with(&self) -> Vec<&str> { vec![] }
}
```

## Implementation Steps

### Step 1: Define Your Strategy Struct

```rust
use app_detector::{DetectionStrategy, DetectionContext, DetectionResult};
use app_detector::types::{StrategyCategory, AppTypeCategory};

/// Detects Go projects via go.mod
#[derive(Default)]
pub struct GoStrategy;
```

**Tips**:
- Use `#[derive(Default)]` for unit structs
- Add doc comments explaining what it detects
- Keep it stateless (no fields unless needed)

### Step 2: Implement Required Methods

#### `id()` - Unique Identifier

```rust
fn id(&self) -> &str {
    "go"  // lowercase, alphanumeric with hyphens
}
```

**Rules**:
- Lowercase only
- Use hyphens for multi-word: `"react-native"`
- Must be unique across all strategies
- Used for dependencies: `depends_on: vec!["go"]`

#### `name()` - Display Name

```rust
fn name(&self) -> &str {
    "Go"  // Human-readable
}
```

**Rules**:
- Proper capitalization
- Used in UI/logging
- Can include version: `"Node.js 18"`

#### `category()` - Classification

```rust
fn category(&self) -> StrategyCategory {
    StrategyCategory::AppType(AppTypeCategory::Language)
}
```

**App Type Categories**:
- `Language` - Programming languages (Go, Rust, Python)
- `Framework` - Web frameworks (React, Django)
- `Monorepo` - Monorepo tools (Nx, Turborepo)
- `BuildTool` - Build systems (webpack, vite)
- `PackageManager` - Package managers (npm, cargo)
- `Service` - Infrastructure (Redis, PostgreSQL)
- `Custom(String)` - Other categories

**Environment Capability Categories**:
- `Local` - Local development
- `Docker` - Container environments
- `Kubernetes` - Orchestration
- `CloudPlatform` - Cloud services

#### `priority()` - Execution Order

```rust
fn priority(&self) -> usize {
    100  // Same as other languages
}
```

**Priority Ranges**:
- **0-99**: Core infrastructure (package managers, runtimes)
- **100-199**: Languages
- **200-299**: Frameworks
- **300-399**: Build tools
- **400-499**: Containers/orchestration
- **500+**: Everything else

**Within Same Priority**:
- Dependencies honored: `react` runs after `nodejs` even if same priority
- Otherwise: alphabetical by ID

#### `can_apply()` - Fast Preliminary Check

```rust
fn can_apply(&self, ctx: &DetectionContext) -> bool {
    ctx.file_exists("go.mod")
}
```

**Rules**:
- MUST be fast (< 1ms)
- Only check file existence
- NO file reading or parsing
- Return `true` if detection might succeed

**Good**:
```rust
ctx.file_exists("Cargo.toml")
ctx.file_exists("package.json") && ctx.file_exists("tsconfig.json")
!ctx.glob("**/*.rs").is_empty()
```

**Bad** (too slow):
```rust
ctx.read_file("go.mod")?.contains("go 1.21")  // Don't read files here!
```

#### `detect()` - Full Detection

```rust
fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
    // 1. Read necessary files
    let go_mod = ctx.read_file("go.mod")?;

    // 2. Parse/extract information
    let module_name = extract_module_name(&go_mod);
    let go_version = extract_go_version(&go_mod);

    // 3. Build metadata
    let mut metadata = HashMap::new();
    if let Some(name) = module_name {
        metadata.insert("package_name".to_string(), serde_json::json!(name));
    }
    if let Some(version) = go_version {
        metadata.insert("go_version".to_string(), serde_json::json!(version));
    }

    // 4. Return structured result
    Ok(DetectionResult {
        strategy_id: self.id().to_string(),
        category: self.category(),
        confidence: 1.0,  // 0.0 - 1.0
        data: DetectionData::Language(LanguageInfo {
            name: "Go".to_string(),
            version: go_version,
            version_source: Some("go.mod".to_string()),
            primary_files: vec![PathBuf::from("go.mod")],
            total_lines: None,
            metadata,
        }),
        suggested_strategies: vec![],  // Optional: suggest other strategies
    })
}
```

### Step 3: Add Optional Methods (if needed)

#### Dependencies

```rust
fn depends_on(&self) -> Vec<&str> {
    vec!["nodejs"]  // This strategy needs Node.js to be detected first
}
```

**Use Cases**:
- Framework depends on language: `react` → `nodejs`
- Environment depends on app type: `local` → `nodejs`, `python`, `rust`

#### Conflicts

```rust
fn conflicts_with(&self) -> Vec<&str> {
    vec!["npm", "yarn"]  // Can't have both npm and yarn as primary
}
```

**Use Cases**:
- Mutually exclusive package managers
- Conflicting frameworks

### Step 4: Write Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_go_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create go.mod
        fs::write(
            temp_dir.path().join("go.mod"),
            "module github.com/user/repo\n\ngo 1.21\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = GoStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "go");
        assert_eq!(result.confidence, 1.0);

        // Check data
        match result.data {
            DetectionData::Language(info) => {
                assert_eq!(info.name, "Go");
                assert_eq!(info.version, Some("1.21".to_string()));
                assert_eq!(
                    info.metadata.get("package_name").unwrap(),
                    &serde_json::json!("github.com/user/repo")
                );
            }
            _ => panic!("Expected Language data"),
        }
    }

    #[test]
    fn test_go_not_applicable() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = GoStrategy;

        assert!(!strategy.can_apply(&ctx));
    }
}
```

## Best Practices

### 1. Keep `can_apply()` Fast

✅ **Good**:
```rust
fn can_apply(&self, ctx: &DetectionContext) -> bool {
    ctx.file_exists("package.json")
}
```

❌ **Bad**:
```rust
fn can_apply(&self, ctx: &DetectionContext) -> bool {
    if let Ok(content) = ctx.read_file("package.json") {
        content.contains("\"react\"")
    } else {
        false
    }
}
```

### 2. Use Structured Data

✅ **Good**: Use typed structures
```rust
DetectionData::Language(LanguageInfo { ... })
```

❌ **Bad**: Use generic KeyValue for common types
```rust
DetectionData::KeyValue(HashMap::from([
    ("type", "language"),
    ("name", "Go"),
]))
```

### 3. Extract Package Names

Always include `package_name` in metadata:

```rust
let mut metadata = HashMap::new();
metadata.insert("package_name".to_string(), serde_json::json!(name));
```

This enables `report.app_name()` helper.

### 4. Set Appropriate Confidence

```rust
confidence: 1.0,  // Definitive (has definitive marker file)
confidence: 0.8,  // Very likely (multiple indicators)
confidence: 0.5,  // Maybe (weak indicators)
```

### 5. Suggest Related Strategies

```rust
suggested_strategies: vec!["react", "typescript"],
```

Helps discover related technologies.

### 6. Handle Errors Gracefully

```rust
fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
    let content = ctx.read_file("go.mod")
        .context("Failed to read go.mod")?;

    let module = extract_module(&content)
        .ok_or_else(|| anyhow!("Invalid go.mod format"))?;

    Ok(result)
}
```

### 7. Cache Expensive Operations

Use the context cache:

```rust
// First call: reads from disk
let pkg = ctx.parse_json::<PackageJson>("package.json")?;

// Second call (same or different strategy): uses cache
let pkg = ctx.parse_json::<PackageJson>("package.json")?;
```

## Testing Strategies

### Unit Tests

Test individual strategy logic:

```rust
#[test]
fn test_detection_with_fixture() {
    let temp = create_go_project();  // Helper function
    let ctx = DetectionContext::new(temp.path()).unwrap();
    let strategy = GoStrategy;

    let result = strategy.detect(&ctx).unwrap();

    assert_eq!(result.strategy_id, "go");
    // ... more assertions
}
```

### Integration Tests

Test within full engine:

```rust
#[test]
fn test_go_with_engine() {
    let temp = create_go_project();

    let mut registry = StrategyRegistry::new();
    registry.register(Box::new(GoStrategy));

    let engine = DetectionEngine::new(registry);
    let report = engine.detect(temp.path()).unwrap();

    assert!(report.has("go"));
}
```

### Test Fixtures

Create realistic project structures:

```
tests/
  fixtures/
    go-project/
      go.mod
      go.sum
      main.go
    go-with-docker/
      go.mod
      Dockerfile
```

## Real-World Examples

### Example 1: Ruby Strategy

```rust
use app_detector::{
    DetectionStrategy, DetectionContext, DetectionResult,
    StrategyCategory, DetectionData,
};
use app_detector::types::{AppTypeCategory, LanguageInfo};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default)]
pub struct RubyStrategy;

impl DetectionStrategy for RubyStrategy {
    fn id(&self) -> &str {
        "ruby"
    }

    fn name(&self) -> &str {
        "Ruby"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::Language)
    }

    fn priority(&self) -> usize {
        100
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.file_exists("Gemfile") || ctx.file_exists("*.gemspec")
    }

    fn detect(&self, ctx: &DetectionContext) -> app_detector::Result<DetectionResult> {
        let mut metadata = HashMap::new();
        let mut primary_files = Vec::new();
        let mut version = None;

        // Check Gemfile
        if ctx.file_exists("Gemfile") {
            primary_files.push(PathBuf::from("Gemfile"));

            // Extract Ruby version if specified
            if let Ok(content) = ctx.read_file("Gemfile") {
                if let Some(v) = extract_ruby_version(&content) {
                    version = Some(v);
                    metadata.insert("version_source".to_string(),
                                  serde_json::json!("Gemfile"));
                }
            }
        }

        // Check .ruby-version
        if ctx.file_exists(".ruby-version") {
            if let Ok(content) = ctx.read_file(".ruby-version") {
                version = Some(content.trim().to_string());
                metadata.insert("version_source".to_string(),
                              serde_json::json!(".ruby-version"));
            }
        }

        // Suggest Rails if present
        let mut suggested = vec![];
        if ctx.file_exists("config/application.rb") {
            suggested.push("rails".to_string());
        }

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::Language(LanguageInfo {
                name: "Ruby".to_string(),
                version,
                version_source: metadata.get("version_source")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                primary_files,
                total_lines: None,
                metadata,
            }),
            suggested_strategies: suggested,
        })
    }
}

fn extract_ruby_version(gemfile: &str) -> Option<String> {
    for line in gemfile.lines() {
        let line = line.trim();
        if line.starts_with("ruby ") {
            return line
                .strip_prefix("ruby ")?
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string()
                .into();
        }
    }
    None
}
```

### Example 2: React Framework Strategy

```rust
#[derive(Default)]
pub struct ReactStrategy;

impl DetectionStrategy for ReactStrategy {
    fn id(&self) -> &str {
        "react"
    }

    fn name(&self) -> &str {
        "React"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::Framework)
    }

    fn priority(&self) -> usize {
        200  // Frameworks run after languages
    }

    fn depends_on(&self) -> Vec<&str> {
        vec!["nodejs"]  // Requires Node.js
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.file_exists("package.json")
    }

    fn detect(&self, ctx: &DetectionContext) -> app_detector::Result<DetectionResult> {
        #[derive(serde::Deserialize)]
        struct PackageJson {
            dependencies: Option<HashMap<String, String>>,
            #[serde(rename = "devDependencies")]
            dev_dependencies: Option<HashMap<String, String>>,
        }

        let pkg: PackageJson = ctx.parse_json("package.json")?;

        // Check if React is in dependencies
        let react_version = pkg.dependencies
            .as_ref()
            .and_then(|deps| deps.get("react"))
            .or_else(|| {
                pkg.dev_dependencies
                    .as_ref()
                    .and_then(|deps| deps.get("react"))
            });

        if react_version.is_none() {
            anyhow::bail!("React not found in package.json");
        }

        let mut metadata = HashMap::new();
        metadata.insert("version".to_string(),
                       serde_json::json!(react_version.unwrap()));

        // Detect if Next.js is also present
        let mut suggested = vec![];
        if pkg.dependencies.as_ref()
            .map(|d| d.contains_key("next"))
            .unwrap_or(false)
        {
            suggested.push("nextjs".to_string());
        }

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::Framework(FrameworkInfo {
                name: "React".to_string(),
                version: Some(react_version.unwrap().clone()),
                based_on_language: "nodejs".to_string(),
                config_files: vec![PathBuf::from("package.json")],
                metadata,
            }),
            suggested_strategies: suggested,
        })
    }
}
```

## Common Patterns

### Pattern 1: Multi-File Detection

```rust
fn can_apply(&self, ctx: &DetectionContext) -> bool {
    ctx.file_exists("Cargo.toml") ||
    ctx.file_exists("rust-toolchain.toml") ||
    !ctx.glob("**/*.rs").is_empty()
}
```

### Pattern 2: Version Extraction

```rust
fn extract_version(content: &str, pattern: &str) -> Option<String> {
    content.lines()
        .find(|line| line.contains(pattern))?
        .split('=')
        .nth(1)?
        .trim()
        .trim_matches('"')
        .to_string()
        .into()
}
```

### Pattern 3: Conditional Suggestions

```rust
let mut suggested = vec![];

if has_typescript {
    suggested.push("typescript".to_string());
}

if has_jest_config {
    suggested.push("jest".to_string());
}

// ...in result
suggested_strategies: suggested,
```

### Pattern 4: Accessing Phase 1 Results

```rust
fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
    // This is an environment strategy that depends on app type
    let mut commands = HashMap::new();

    // Check if Node.js was detected
    if let Some(nodejs) = ctx.get_result("nodejs") {
        if let DetectionData::Language(info) = &nodejs.data {
            // Extract npm scripts
            // ...
        }
    }

    // ...
}
```

## Registration

```rust
// In your application
let mut registry = StrategyRegistry::with_defaults();
registry.register(Box::new(GoStrategy));
registry.register(Box::new(RubyStrategy));
registry.register(Box::new(ReactStrategy));

let engine = DetectionEngine::new(registry);
```

## Next Steps

- Review [existing strategies](../src/strategies) for inspiration
- Read [architecture.md](architecture.md) for internals
- Check [examples/](../examples) for full examples
- Submit your strategy as a PR!

## Getting Help

- Check [API docs](https://docs.rs/app-detector)
- Look at built-in strategies in `src/strategies/`
- Open an issue on GitHub
- Ask in discussions

Happy strategy development! 🚀
