//! Convenience re-exports for common usage patterns.
//!
//! ```rust
//! use config_manager::prelude::*;
//! ```

pub use crate::core::error::{Error, Result};
pub use crate::core::{ConfigManager, ConfigManagerBuilder};
pub use crate::dependencies::{DependencyGraph, DependencyProvider};
pub use crate::loader::{ConfigLoader, JsonLoader};
pub use crate::resolver::Resolver;
pub use crate::validation::Validator;

// Feature-gated loaders
#[cfg(feature = "toml")]
pub use crate::loader::TomlLoader;
#[cfg(feature = "yaml")]
pub use crate::loader::YamlLoader;

// Layered config support
pub use crate::loader::{LayeredLoader, MergeStrategy};

// Re-export serde for convenience
pub use serde::{Deserialize, Serialize};
