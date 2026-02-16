//! Core loader trait definition.

use crate::core::Result;
use std::fmt::Debug;

/// Trait for loading and saving configuration from various sources.
///
/// Implementations can load from files, databases, HTTP endpoints, etc.
///
/// # Type Parameters
///
/// * `T` - The configuration type to load/save
///
/// # Example
///
/// ```rust
/// use config_manager::prelude::*;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct MyConfig {
///     name: String,
/// }
///
/// fn example() -> Result<()> {
///     let loader: JsonLoader = JsonLoader::new("config.json");
///
///     // Check if config exists
///     if ConfigLoader::<MyConfig>::exists(&loader) {
///         let config: MyConfig = loader.load()?;
///         println!("Loaded: {}", config.name);
///     }
///
///     Ok(())
/// }
/// ```
pub trait ConfigLoader<T>: Send + Sync + Debug {
    /// Load configuration from the source.
    ///
    /// # Errors
    ///
    /// Returns `Error::LoadError` if loading fails.
    fn load(&self) -> Result<T>;

    /// Save configuration to the source.
    ///
    /// # Errors
    ///
    /// Returns `Error::SaveError` if saving fails.
    fn save(&self, config: &T) -> Result<()>;

    /// Check if the configuration source exists.
    ///
    /// For file-based loaders, this checks if the file exists.
    /// For network-based loaders, this might check connectivity.
    fn exists(&self) -> bool;

    /// Get a human-readable description of the source.
    ///
    /// Used for error messages and logging.
    ///
    /// # Example
    ///
    /// ```rust
    /// use config_manager::loader::{ConfigLoader, JsonLoader};
    /// use serde::{Deserialize, Serialize};
    ///
    /// #[derive(Deserialize, Serialize)]
    /// struct Config { name: String }
    ///
    /// let loader = JsonLoader::new("/path/to/config.json");
    /// assert_eq!(ConfigLoader::<Config>::source_info(&loader), "/path/to/config.json");
    /// ```
    fn source_info(&self) -> String;

    /// Reload configuration (alias for load).
    ///
    /// Useful for implementing watch/reload functionality.
    fn reload(&self) -> Result<T> {
        self.load()
    }
}
