//! TOML configuration loader implementation.

use crate::core::{Error, Result};
use crate::loader::ConfigLoader;
use crate::utils::path::expand_all;
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// TOML file-based configuration loader.
///
/// Loads and saves configuration from/to TOML files with optional
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
///     let loader = TomlLoader::new("~/config.toml")
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
pub struct TomlLoader {
    path: PathBuf,
    expand_paths: bool,
    pretty_print: bool,
    create_dirs: bool,
}

impl TomlLoader {
    /// Create a new TOML loader for the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the TOML file (supports ~ and $ENV expansion)
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
    /// Default: true (formatted, readable TOML)
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

impl<T> ConfigLoader<T> for TomlLoader
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

        toml::from_str(&content).map_err(|e| {
            Error::LoadError(format!(
                "Failed to parse TOML from {}: {}",
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

        // Serialize to TOML
        let content = if self.pretty_print {
            toml::to_string_pretty(config)
        } else {
            toml::to_string(config)
        }
        .map_err(|e| Error::SaveError(format!("Failed to serialize TOML: {}", e)))?;

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
        enabled: bool,
    }

    #[test]
    fn test_toml_loader_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.toml");

        let loader = TomlLoader::new(&config_path);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
            enabled: true,
        };

        // Save
        loader.save(&config).unwrap();
        assert!(config_path.exists());

        // Load
        let loaded: TestConfig = loader.load().unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_toml_loader_not_found() {
        let loader = TomlLoader::new("/nonexistent/config.toml");
        let result: Result<TestConfig> = loader.load();
        assert!(matches!(result, Err(Error::LoadError(_))));
    }

    #[test]
    fn test_toml_loader_pretty_print() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("pretty.toml");

        let loader = TomlLoader::new(&config_path).with_pretty_print(true);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
            enabled: true,
        };

        loader.save(&config).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();

        // Pretty-printed TOML should have multiple lines
        assert!(content.lines().count() > 1);

        // Check for TOML format
        assert!(content.contains("name ="));
        assert!(content.contains("count ="));
        assert!(content.contains("enabled ="));
    }

    #[test]
    fn test_toml_loader_create_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir
            .path()
            .join("subdir")
            .join("nested")
            .join("config.toml");

        let loader = TomlLoader::new(&config_path).with_create_dirs(true);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
            enabled: true,
        };

        // Should create nested directories automatically
        loader.save(&config).unwrap();
        assert!(config_path.exists());
    }

    #[test]
    fn test_toml_loader_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("exists.toml");

        let loader: TomlLoader = TomlLoader::new(&config_path);
        assert!(!ConfigLoader::<TestConfig>::exists(&loader));

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
            enabled: true,
        };

        loader.save(&config).unwrap();
        assert!(ConfigLoader::<TestConfig>::exists(&loader));
    }

    #[test]
    fn test_toml_loader_source_info() {
        let loader: TomlLoader = TomlLoader::new("/path/to/config.toml");
        assert_eq!(
            ConfigLoader::<TestConfig>::source_info(&loader),
            "/path/to/config.toml"
        );
    }

    #[test]
    fn test_toml_format_preserved() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("format.toml");

        let loader = TomlLoader::new(&config_path);

        let config = TestConfig {
            name: "my-app".to_string(),
            count: 100,
            enabled: false,
        };

        loader.save(&config).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();

        // Verify TOML structure
        assert!(content.contains(r#"name = "my-app""#));
        assert!(content.contains("count = 100"));
        assert!(content.contains("enabled = false"));
    }
}
