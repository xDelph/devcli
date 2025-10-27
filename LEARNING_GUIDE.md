# RustyCLI Code Learning Guide

This guide explains the key Rust concepts used in the new config system to help you learn the language.

## Key Rust Concepts Used

### 1. Structs and Derive Macros

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub projects: HashMap<String, Project>,
}
```

**What this means:**
- `struct` - Like a class in other languages, groups related data together
- `#[derive(...)]` - Automatically implements traits (interfaces) for the struct
  - `Debug` - Allows printing with `{:?}` for debugging
  - `Clone` - Allows making copies with `.clone()`
  - `Serialize` - Can convert to JSON (serde does this)
  - `Deserialize` - Can parse from JSON (serde does this)
- `pub` - Makes the struct public (accessible from other modules)
- `HashMap<String, Project>` - A hash map with String keys and Project values

### 2. Option and Result Types

```rust
pub fn load_preferences() -> Result<Preferences> {
    let pref_path = get_preferences_path()?;
    
    if !pref_path.exists() {
        return Ok(Preferences::default());
    }
    // ...
}
```

**What this means:**
- `Result<Preferences>` - Returns either `Ok(Preferences)` or an error
- `Option<T>` - Can be `Some(value)` or `None` (like nullable in other languages)
- `?` operator - If there's an error, return it immediately. Otherwise, unwrap the value
- `Ok(...)` - Wraps a success value in a Result

### 3. Ownership and Borrowing

```rust
pub fn resolve_app(config: &Config, app_name: &str) -> Result<ResolvedApp> {
    // config is "borrowed" (&Config means we don't own it, just reference it)
    // This function can read config but cannot modify or take ownership
}
```

**Borrowing rules:**
- `&T` - Immutable reference (can read, cannot modify)
- `&mut T` - Mutable reference (can read and modify)
- No `&` - Takes ownership (original variable can't be used anymore)

### 4. Pattern Matching

```rust
match environment.as_str() {
    "local" => &resolved_app.app.commands.local,
    "docker" => &resolved_app.app.commands.docker,
    _ => anyhow::bail!("Invalid environment"),
}
```

**What this means:**
- `match` - Like switch/case but more powerful
- Each arm has a pattern => result
- `_` - Catch-all pattern (like `default` in switch)
- Must handle ALL possible cases (compiler enforces this)

### 5. Iterators and Closures

```rust
let all_apps: Vec<_> = config.projects
    .iter()
    .flat_map(|(project_name, project)| {
        project.apps.iter().map(move |(app_name, app)| {
            (project_name.clone(), app_name.clone(), app.clone())
        })
    })
    .collect();
```

**What this means:**
- `.iter()` - Creates an iterator over items
- `.flat_map(|x| ...)` - Map and flatten results (closure/lambda function)
- `|(a, b)|` - Destructuring tuple in closure parameters
- `move` - Closure takes ownership of captured variables
- `.collect()` - Gathers iterator items into a collection

### 6. Error Handling

```rust
anyhow::bail!("Error message");  // Return error immediately
anyhow::anyhow!("Error");        // Create an error
.context("Additional context")?  // Add context to an error
```

**What `anyhow` provides:**
- Easy error creation and propagation
- Error context for better debugging
- Works with `?` operator

### 7. Generics

```rust
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    // ...
}
```

**What this means:**
- `<P: AsRef<Path>>` - Generic type P that implements AsRef<Path>
- This function accepts anything that can be converted to a Path reference
- Works with `&str`, `String`, `PathBuf`, etc.

### 8. Traits

```rust
impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_env: "local".to_string(),
        }
    }
}
```

**What this means:**
- `trait` - Like an interface in other languages
- `impl Trait for Type` - Implements the trait for a specific type
- `Default` trait - Provides a default value for the type
- `Self` - Refers to the type we're implementing for (Preferences)

## File-by-File Explanations

### config/models.rs
Defines the data structures that map to our JSON config file.

**Key points:**
- Structs match JSON structure exactly
- `#[serde(rename = "type")]` - JSON field "type" maps to Rust field "app_type" (type is a keyword)
- `#[serde(default)]` - If field is missing in JSON, use default value
- HashMap for dynamic keys (project names, app names, commands)

### config/loader.rs
Handles reading and writing config files.

**Key points:**
- `get_config_path()` - Builds path to config file
- `load_config()` - Reads JSON, parses into Config struct
- `save_preferences()` - Converts Preferences to JSON, writes to file
- Error handling with context messages

### config/resolver.rs
Finds apps in the config and resolves ambiguities.

**Key points:**
- `resolve_app()` - Searches all projects for an app by name
- Handles ambiguous names (same app name in multiple projects)
- Levenshtein distance for typo suggestions
- Returns `ResolvedApp` with full context (project + app)

### config/dependencies.rs
Manages dependency chains between apps.

**Key points:**
- `resolve_dependency_chain()` - Builds list of dependencies (BFS traversal)
- Detects circular dependencies using `in_progress` set
- `check_dependencies_running()` - Verifies deps are actually running
- Uses ProcessTracker to check PID status

### commands/start.rs
Starts apps using config.

**Flow:**
1. Load config and preferences
2. Resolve app (find it in config)
3. Determine environment (local/docker)
4. Check dependencies are running
5. Expand path (~ and $VAR)
6. Spawn process
7. Track process metadata

### commands/run.rs
Similar to start but runs specific command variants.

**Difference from start:**
- Takes explicit command variant parameter
- Process name includes variant: "app:command-variant"
- No default command lookup needed

### commands/status.rs
Shows running processes grouped by project.

**Key points:**
- Groups processes by `project` field in ProcessInfo
- Filters by project if requested
- Shows dependency status with `--deps` flag
- Calculates uptime from start_time

## Common Patterns You'll See

### 1. The `?` Operator Chain
```rust
let config = load_config()?;
let resolved = resolve_app(&config, &app_name, None)?;
let path = expand_path(&resolved.app.path);
```
Each `?` either unwraps the Ok value or returns the error immediately.

### 2. Option Handling
```rust
let env = args.env
    .clone()
    .unwrap_or_else(|| preferences.default_env.clone());
```
If `args.env` is `Some(value)`, use it. Otherwise, use the closure result.

### 3. String Conversions
```rust
"string literal"           // &str (string slice, borrowed)
.to_string()               // String (owned)
.clone()                   // Make a copy
.as_str()                  // Convert String to &str
format!("template {}", x)  // Create formatted String
```

### 4. Collecting Iterators
```rust
let items: Vec<String> = iterator
    .filter(|x| condition)
    .map(|x| transform(x))
    .collect();
```
Type annotation `: Vec<String>` tells collect() what to build.

## Learning Resources

1. **The Rust Book** - https://doc.rust-lang.org/book/
   - Start here for fundamentals
   
2. **Rust By Example** - https://doc.rust-lang.org/rust-by-example/
   - Learn by seeing code examples
   
3. **Rustlings** - https://github.com/rust-lang/rustlings
   - Interactive exercises

## Next Steps for Learning

1. Read through `config/models.rs` - Understand the data structures
2. Trace through `commands/start.rs` - Follow the flow from args to spawned process
3. Experiment with modifications - Try adding a new field to App struct
4. Read compiler errors carefully - Rust's error messages are very helpful

The code follows common Rust patterns and idioms, so understanding this codebase will help you read most Rust code!
