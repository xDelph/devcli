//! Async JSON configuration loader implementation.

use crate::core::{Error, Result};
use crate::loader::AsyncConfigLoader;
use crate::utils::path::expand_all;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Async JSON file-based configuration loader.
///
/// Loads and saves configuration from/to JSON files asynchronously using tokio.
///
/// # Examples
///
/// ```rust,no_run
/// use config_manager::loader::AsyncJsonLoader;
/// use config_manager::loader::AsyncConfigLoader;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct MyConfig {
///     name: String,
///     port: u16,
/// }
///
/// #[tokio::main]
/// async fn main() -> config_manager::core::Result<()> {
///     let loader = AsyncJsonLoader::new("config.json");
///
///     let config = MyConfig {
///         name: "myapp".to_string(),
///         port: 3000,
///     };
///
///     loader.save(&config).await?;
///     let loaded: MyConfig = loader.load().await?;
///
///     println!("Loaded: {}", loaded.name);
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AsyncJsonLoader {
    path: PathBuf,
    expand_paths: bool,
    pretty_print: bool,
    create_dirs: bool,
}

impl AsyncJsonLoader {
    /// Create a new async JSON loader for the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the JSON file (supports ~ and $ENV expansion)
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            expand_paths: true,
            pretty_print: true,
            create_dirs: true,
        }
    }

    /// Enable or disable path expansion (tilde and env vars).
    ///
    /// Default: true
    pub fn with_path_expansion(mut self, enabled: bool) -> Self {
        self.expand_paths = enabled;
        self
    }

    /// Enable or disable pretty-printing when saving.
    ///
    /// Default: true (formatted, readable JSON)
    pub fn with_pretty_print(mut self, enabled: bool) -> Self {
        self.pretty_print = enabled;
        self
    }

    /// Enable or disable automatic directory creation when saving.
    ///
    /// Default: true
    pub fn with_create_dirs(mut self, enabled: bool) -> Self {
        self.create_dirs = enabled;
        self
    }

    /// Get the expanded absolute path.
    fn expanded_path(&self) -> PathBuf {
        if self.expand_paths {
            expand_all(&self.path)
        } else {
            self.path.clone()
        }
    }
}

#[async_trait]
impl<T> AsyncConfigLoader<T> for AsyncJsonLoader
where
    T: DeserializeOwned + Serialize + Send + Sync,
{
    async fn load(&self) -> Result<T> {
        let path = self.expanded_path();

        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Err(Error::LoadError(format!(
                "Configuration file not found: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(&path)
            .await
            .map_err(|e| Error::LoadError(format!("Failed to read {}: {}", path.display(), e)))?;

        serde_json::from_str(&content).map_err(|e| {
            Error::LoadError(format!(
                "Failed to parse JSON from {}: {}",
                path.display(),
                e
            ))
        })
    }

    async fn save(&self, config: &T) -> Result<()> {
        let path = self.expanded_path();

        // Create parent directories if needed
        if self.create_dirs {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await.map_err(|e| {
                    Error::SaveError(format!(
                        "Failed to create directory {}: {}",
                        parent.display(),
                        e
                    ))
                })?;
            }
        }

        // Serialize to JSON
        let content = if self.pretty_print {
            serde_json::to_string_pretty(config)
        } else {
            serde_json::to_string(config)
        }
        .map_err(|e| Error::SaveError(format!("Failed to serialize JSON: {}", e)))?;

        // Write to file
        fs::write(&path, content).await.map_err(|e| {
            Error::SaveError(format!("Failed to write to {}: {}", path.display(), e))
        })?;

        Ok(())
    }

    fn exists(&self) -> bool {
        self.expanded_path().exists()
    }

    fn source_info(&self) -> String {
        self.path.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestConfig {
        name: String,
        value: u32,
    }

    #[tokio::test]
    async fn test_async_json_loader_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.json");

        let loader = AsyncJsonLoader::new(&config_path);

        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };

        // Save
        loader.save(&config).await.unwrap();
        assert!(config_path.exists());

        // Load
        let loaded: TestConfig = loader.load().await.unwrap();
        assert_eq!(loaded, config);
    }

    #[tokio::test]
    async fn test_async_json_loader_not_found() {
        let loader = AsyncJsonLoader::new("/nonexistent/config.json");
        let result: Result<TestConfig> = loader.load().await;
        assert!(matches!(result, Err(Error::LoadError(_))));
    }

    #[tokio::test]
    async fn test_async_json_loader_create_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir
            .path()
            .join("subdir")
            .join("nested")
            .join("config.json");

        let loader = AsyncJsonLoader::new(&config_path).with_create_dirs(true);

        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };

        // Should create nested directories automatically
        loader.save(&config).await.unwrap();
        assert!(config_path.exists());
    }

    #[tokio::test]
    async fn test_async_json_loader_pretty_print() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("pretty.json");

        let loader = AsyncJsonLoader::new(&config_path).with_pretty_print(true);

        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };

        loader.save(&config).await.unwrap();

        let content = fs::read_to_string(&config_path).await.unwrap();

        // Pretty-printed JSON should have multiple lines
        assert!(content.lines().count() > 1);
        assert!(content.contains("\"name\""));
        assert!(content.contains("\"value\""));
    }

    #[tokio::test]
    async fn test_async_json_loader_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("exists.json");

        let loader = AsyncJsonLoader::new(&config_path);
        assert!(!AsyncConfigLoader::<TestConfig>::exists(&loader));

        let config = TestConfig {
            name: "test".to_string(),
            value: 42,
        };

        loader.save(&config).await.unwrap();
        assert!(AsyncConfigLoader::<TestConfig>::exists(&loader));
    }
}
