# Rust Concepts Explained

This document explains key Rust concepts used in the RustyCLI project. Use it as a reference when reading the code.

## Ownership and Borrowing

### Ownership
- Every value in Rust has a single owner
- When the owner goes out of scope, the value is dropped (memory freed)
- Values can be "moved" to transfer ownership

```rust
let s1 = String::from("hello");
let s2 = s1;  // s1 is moved to s2, s1 can no longer be used
```

### Borrowing (References)
- `&T` - Immutable reference (can read but not modify)
- `&mut T` - Mutable reference (can read and modify)
- You can have many immutable references OR one mutable reference (not both)

```rust
fn read_data(s: &String) { }  // Borrows immutably
fn modify_data(s: &mut String) { }  // Borrows mutably
```

### Clone
- `.clone()` makes a deep copy of data
- Used when you need the same data in multiple places
- Has a performance cost (copies memory)

## Common Types

### String Types
- `String` - Owned, growable string (heap allocated)
- `&str` - String slice (borrowed reference to string data)
- Use `String` when you need to own/modify, `&str` for reading

### Option<T>
Represents a value that might or might not exist:
- `Some(value)` - Has a value
- `None` - No value

```rust
let maybe_number: Option<i32> = Some(42);
let no_number: Option<i32> = None;

// Safely extract value
if let Some(n) = maybe_number {
    println!("Got: {}", n);
}
```

### Result<T, E>
Represents success or failure:
- `Ok(value)` - Operation succeeded
- `Err(error)` - Operation failed

```rust
fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("Division by zero".to_string())
    } else {
        Ok(a / b)
    }
}

// The ? operator returns error if Result is Err
let result = divide(10, 2)?;
```

### Vec<T>
Dynamic array (like ArrayList in Java or list in Python):
```rust
let mut numbers = Vec::new();  // Empty vector
numbers.push(1);  // Add element
numbers.push(2);
```

### HashMap<K, V>
Key-value map:
```rust
use std::collections::HashMap;

let mut map = HashMap::new();
map.insert("key", "value");
```

## Async/Await

### Async Functions
Functions that can be paused and resumed:
```rust
async fn fetch_data() -> Result<String> {
    // .await pauses until operation completes
    let response = http_get("url").await?;
    Ok(response)
}
```

### Why Async?
- Allows handling many operations concurrently without blocking
- More efficient than threads for I/O-heavy operations
- Used for file I/O, network calls, process management

### Tokio Runtime
- `#[tokio::main]` sets up the async runtime
- Manages async task execution
- Required to run async functions

## Smart Pointers

### Arc<T>
Atomic Reference Counted - allows multiple ownership:
- Thread-safe reference counting
- Allows sharing data across threads/tasks
- Automatically freed when last reference is dropped

```rust
use std::sync::Arc;

let data = Arc::new(vec![1, 2, 3]);
let data_clone = data.clone();  // Increases reference count
// Both data and data_clone point to the same memory
```

### Mutex<T>
Mutual Exclusion - ensures only one thread accesses data at a time:
```rust
use tokio::sync::Mutex;

let data = Mutex::new(5);
let mut guard = data.lock().await;  // Wait for exclusive access
*guard += 1;  // Modify data
// Lock automatically released when guard is dropped
```

### Arc<Mutex<T>>
Combination for sharing mutable data across async tasks:
```rust
let shared = Arc::new(Mutex::new(vec![]));
let shared_clone = shared.clone();

tokio::spawn(async move {
    let mut data = shared_clone.lock().await;
    data.push(1);
});
```

## Pattern Matching

### if let
Convenient way to match one pattern:
```rust
if let Some(value) = maybe_value {
    println!("Got: {}", value);
}

if let Ok(result) = operation() {
    println!("Success: {}", result);
}
```

### match
Like switch but more powerful:
```rust
match result {
    Ok(value) => println!("Success: {}", value),
    Err(e) => println!("Error: {}", e),
}
```

## Traits

Traits are like interfaces - they define shared behavior:

```rust
// Debug trait allows printing with {:?}
#[derive(Debug)]
struct Point { x: i32, y: i32 }

// Clone trait allows .clone()
#[derive(Clone)]
struct Data { value: String }
```

### Common Traits
- `Debug` - Can be printed with `{:?}`
- `Clone` - Can be copied with `.clone()`
- `Default` - Has a default value
- `Serialize/Deserialize` - Can be converted to/from JSON

## Error Handling

### The ? Operator
Automatically returns errors:
```rust
fn process() -> Result<()> {
    let file = open_file()?;  // If error, return immediately
    let data = read_file(file)?;
    Ok(())
}
```

### Context
Add helpful error messages:
```rust
use anyhow::Context;

let file = open_file("data.txt")
    .context("Failed to open data file")?;
```

### bail!
Return an error immediately:
```rust
if invalid_input {
    anyhow::bail!("Input must be positive");
}
```

## Iterators

Rust uses iterators extensively:

```rust
let numbers = vec![1, 2, 3, 4, 5];

// .iter() creates an iterator
let sum: i32 = numbers.iter().sum();

// .filter() keeps matching items
let evens: Vec<_> = numbers
    .iter()
    .filter(|n| *n % 2 == 0)
    .collect();

// .map() transforms each item
let doubled: Vec<_> = numbers
    .iter()
    .map(|n| n * 2)
    .collect();
```

## Closures

Anonymous functions:
```rust
// With parameters
let add = |a, b| a + b;
let result = add(5, 3);  // 8

// Capturing environment
let multiplier = 10;
let multiply = |x| x * multiplier;
let result = multiply(5);  // 50
```

## Macros

Functions that operate on code:
- `println!` - Print to stdout
- `eprintln!` - Print to stderr
- `format!` - Create formatted string
- `vec!` - Create vector: `vec![1, 2, 3]`

## Module System

```rust
// Declare a module
pub mod commands {
    pub fn start() { }
}

// Use items from module
use crate::commands::start;

// Re-export for convenience
pub use commands::start;
```

## Key Differences from Other Languages

### No Null
Rust doesn't have null - use `Option<T>` instead:
```rust
// JavaScript: let x = null;
// Rust: let x: Option<i32> = None;
```

### No Exceptions
Use `Result<T, E>` instead:
```rust
// JavaScript: throw new Error("failed");
// Rust: return Err("failed".to_string());
```

### Memory Safety
Rust prevents:
- Use after free
- Double free
- Data races
- Null pointer dereferences

All at compile time, with no garbage collector!

## Tips for Learning

1. **Read Compiler Errors** - Rust's compiler gives excellent error messages
2. **Use Clippy** - `cargo clippy` suggests improvements
3. **Check Documentation** - `cargo doc --open` opens local docs
4. **The Book** - https://doc.rust-lang.org/book/ is the best resource

## Common Patterns in RustyCLI

### Creating a Result
```rust
pub fn do_something() -> Result<()> {
    // Do work
    Ok(())  // Success with no return value
}
```

### Async File Operations
```rust
pub async fn write_file() -> Result<()> {
    tokio::fs::write("file.txt", "content").await?;
    Ok(())
}
```

### Sharing Data Across Tasks
```rust
let shared = Arc::new(Mutex::new(data));
let clone = shared.clone();

tokio::spawn(async move {
    let mut guard = clone.lock().await;
    // Use data
});
```

