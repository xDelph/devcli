//! # config-manager
//!
//! A flexible, trait-based configuration management library for Rust applications.
//!
//! ## Features
//!
//! - **Generic Configuration**: Works with any serializable type
//! - **Multiple Formats**: JSON (built-in), TOML, YAML (feature-gated)
//! - **Layered Configs**: Merge multiple sources with different strategies
//! - **Atomic Saves**: Configs are replaced in one step, never left half-written
//! - **Dependency Resolution**: Automatic dependency chain resolution with cycle detection
//! - **Fuzzy Matching**: Typo-tolerant entity resolution
//! - **Validation**: Declarative validation with custom validators
//! - **Benchmarked**: Performance benchmarks for all core operations
//! - **Extensible**: Trait-based design allows custom implementations
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use config_manager::prelude::*;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct MyConfig {
//!     name: String,
//!     version: String,
//! }
//!
//! fn main() -> anyhow::Result<()> {
//!     let manager = ConfigManager::<MyConfig>::builder()
//!         .loader(JsonLoader::new("config.json"))
//!         .build()?;
//!
//!     let config = manager.load()?;
//!     println!("Config: {} v{}", config.name, config.version);
//!
//!     Ok(())
//! }
//! ```

// Public modules
pub mod core;
pub mod dependencies;
pub mod loader;
pub mod resolver;
pub mod utils;
pub mod validation;

// Convenience re-exports
pub mod prelude;

// Re-export core types
pub use core::{ConfigManager, ConfigManagerBuilder};
pub use dependencies::{DependencyGraph, DependencyProvider};
pub use loader::{ConfigLoader, JsonLoader};
pub use resolver::Resolver;
pub use validation::Validator;

// Re-export error types
pub use core::error::{Error, Result};
