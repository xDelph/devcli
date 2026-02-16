//! Async configuration loader trait.

use crate::core::Result;
use async_trait::async_trait;
use std::fmt::Debug;

/// Async trait for loading and saving configuration from various sources.
///
/// This is the async version of `ConfigLoader`, designed for I/O-bound
/// operations like network requests or large file operations.
///
/// # Type Parameters
///
/// * `T` - The configuration type to load/save
///
/// # Example
///
/// ```rust,no_run
/// use config_manager::loader::AsyncConfigLoader;
/// use config_manager::core::Result;
/// use async_trait::async_trait;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct MyConfig {
///     name: String,
/// }
///
/// #[derive(Debug)]
/// struct MyAsyncLoader;
///
/// #[async_trait]
/// impl AsyncConfigLoader<MyConfig> for MyAsyncLoader {
///     async fn load(&self) -> Result<MyConfig> {
///         // Async loading logic
///         Ok(MyConfig { name: "test".to_string() })
///     }
///
///     async fn save(&self, config: &MyConfig) -> Result<()> {
///         // Async saving logic
///         Ok(())
///     }
///
///     fn exists(&self) -> bool {
///         true
///     }
///
///     fn source_info(&self) -> String {
///         "my-source".to_string()
///     }
/// }
/// ```
#[async_trait]
pub trait AsyncConfigLoader<T>: Send + Sync + Debug {
    /// Asynchronously load configuration from the source.
    ///
    /// # Errors
    ///
    /// Returns `Error::LoadError` if loading fails.
    async fn load(&self) -> Result<T>;

    /// Asynchronously save configuration to the source.
    ///
    /// # Errors
    ///
    /// Returns `Error::SaveError` if saving fails.
    async fn save(&self, config: &T) -> Result<()>;

    /// Check if the configuration source exists.
    ///
    /// For file-based loaders, this checks if the file exists.
    /// For network-based loaders, this might check connectivity.
    fn exists(&self) -> bool;

    /// Get a human-readable description of the source.
    ///
    /// Used for error messages and logging.
    fn source_info(&self) -> String;

    /// Asynchronously reload configuration (alias for load).
    ///
    /// Useful for implementing watch/reload functionality.
    async fn reload(&self) -> Result<T> {
        self.load().await
    }
}
