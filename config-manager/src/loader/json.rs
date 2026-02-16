//! JSON configuration loader implementation.

use crate::core::{Error, Result};
use crate::loader::ConfigLoader;
use crate::utils::path::expand_all;
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// JSON file-based configuration loader.
///
/// Loads and saves configuration from/to JSON files with optional
/// path expansion (tilde and environment variables).
///
/// # Examples
///
/// ```rust,no_run
/// use config_manager::prelude::*;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct MyConfig {
///     name: String,
///     port: u16,
/// }
///
/// fn main() -> Result<()> {
///     let loader = JsonLoader::new("~/config.json")
///         .with_path_expansion(true)
///         .with_pretty_print(true);
///
///     let config: MyConfig = loader.load()?;
///     println!("Loaded config: {}", config.name);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct JsonLoader {
    path: PathBuf,
    expand_paths: bool,
    pretty_print: bool,
    create_dirs: bool,
}

impl JsonLoader {
    /// Create a new JSON loader for the specified path.
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
    /// Default: true (indented, readable JSON)
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

impl<T> ConfigLoader<T> for JsonLoader
where
    T: DeserializeOwned + Serialize,
{
    fn load(&self) -> Result<T> {
        let path = self.expanded_path();

        if !path.exists() {
            return Err(Error::LoadError(format!(
                "Configuration file not found: {}",
                path.display()
            )));
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| Error::LoadError(format!("Failed to read {}: {}", path.display(), e)))?;

        serde_json::from_str(&content).map_err(|e| {
            Error::LoadError(format!(
                "Failed to parse JSON from {}: {}",
                path.display(),
                e
            ))
        })
    }

    fn save(&self, config: &T) -> Result<()> {
        let path = self.expanded_path();

        // Create parent directories if needed
        if self.create_dirs {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
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
        fs::write(&path, content).map_err(|e| {
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
        count: u32,
    }

    #[test]
    fn test_json_loader_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.json");

        let loader = JsonLoader::new(&config_path);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
        };

        // Save
        loader.save(&config).unwrap();
        assert!(config_path.exists());

        // Load
        let loaded: TestConfig = loader.load().unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_json_loader_not_found() {
        let loader = JsonLoader::new("/nonexistent/config.json");
        let result: Result<TestConfig> = loader.load();
        assert!(matches!(result, Err(Error::LoadError(_))));
    }

    #[test]
    fn test_json_loader_pretty_print() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("pretty.json");

        let loader = JsonLoader::new(&config_path).with_pretty_print(true);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
        };

        loader.save(&config).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains('\n')); // Pretty-printed should have newlines
        assert!(content.contains("  ")); // Should be indented
    }

    #[test]
    fn test_json_loader_no_pretty_print() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("compact.json");

        let loader = JsonLoader::new(&config_path).with_pretty_print(false);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
        };

        loader.save(&config).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        // Compact JSON should be a single line
        assert_eq!(content.lines().count(), 1);
    }

    #[test]
    fn test_json_loader_create_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir
            .path()
            .join("subdir")
            .join("nested")
            .join("config.json");

        let loader = JsonLoader::new(&config_path).with_create_dirs(true);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
        };

        // Should create nested directories automatically
        loader.save(&config).unwrap();
        assert!(config_path.exists());
    }

    #[test]
    fn test_json_loader_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("exists.json");

        let loader: JsonLoader = JsonLoader::new(&config_path);
        assert!(!ConfigLoader::<TestConfig>::exists(&loader));

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
        };

        loader.save(&config).unwrap();
        assert!(ConfigLoader::<TestConfig>::exists(&loader));
    }

    #[test]
    fn test_json_loader_source_info() {
        let loader: JsonLoader = JsonLoader::new("/path/to/config.json");
        assert_eq!(
            ConfigLoader::<TestConfig>::source_info(&loader),
            "/path/to/config.json"
        );
    }
}
