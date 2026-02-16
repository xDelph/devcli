//! YAML configuration loader implementation.

use crate::core::{Error, Result};
use crate::loader::ConfigLoader;
use crate::utils::path::expand_all;
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// YAML file-based configuration loader.
///
/// Loads and saves configuration from/to YAML files with optional
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
///     let loader = YamlLoader::new("~/config.yaml")
///         .with_path_expansion(true);
///
///     let config: MyConfig = loader.load()?;
///     println!("Loaded config: {}", config.name);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct YamlLoader {
    path: PathBuf,
    expand_paths: bool,
    create_dirs: bool,
}

impl YamlLoader {
    /// Create a new YAML loader for the specified path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the YAML file (supports ~ and $ENV expansion)
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            expand_paths: true,
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

impl<T> ConfigLoader<T> for YamlLoader
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

        serde_yaml::from_str(&content).map_err(|e| {
            Error::LoadError(format!(
                "Failed to parse YAML from {}: {}",
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

        // Serialize to YAML
        let content = serde_yaml::to_string(config)
            .map_err(|e| Error::SaveError(format!("Failed to serialize YAML: {}", e)))?;

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
        items: Vec<String>,
    }

    #[test]
    fn test_yaml_loader_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.yaml");

        let loader = YamlLoader::new(&config_path);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
            items: vec!["item1".to_string(), "item2".to_string()],
        };

        // Save
        loader.save(&config).unwrap();
        assert!(config_path.exists());

        // Load
        let loaded: TestConfig = loader.load().unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_yaml_loader_not_found() {
        let loader = YamlLoader::new("/nonexistent/config.yaml");
        let result: Result<TestConfig> = loader.load();
        assert!(matches!(result, Err(Error::LoadError(_))));
    }

    #[test]
    fn test_yaml_loader_format() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("format.yaml");

        let loader = YamlLoader::new(&config_path);

        let config = TestConfig {
            name: "my-app".to_string(),
            count: 100,
            items: vec!["one".to_string(), "two".to_string()],
        };

        loader.save(&config).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();

        // Verify YAML structure
        assert!(content.contains("name:"));
        assert!(content.contains("count:"));
        assert!(content.contains("items:"));
        assert!(content.contains("- one"));
        assert!(content.contains("- two"));
    }

    #[test]
    fn test_yaml_loader_create_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir
            .path()
            .join("subdir")
            .join("nested")
            .join("config.yaml");

        let loader = YamlLoader::new(&config_path).with_create_dirs(true);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
            items: vec![],
        };

        // Should create nested directories automatically
        loader.save(&config).unwrap();
        assert!(config_path.exists());
    }

    #[test]
    fn test_yaml_loader_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("exists.yaml");

        let loader: YamlLoader = YamlLoader::new(&config_path);
        assert!(!ConfigLoader::<TestConfig>::exists(&loader));

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
            items: vec![],
        };

        loader.save(&config).unwrap();
        assert!(ConfigLoader::<TestConfig>::exists(&loader));
    }

    #[test]
    fn test_yaml_loader_source_info() {
        let loader: YamlLoader = YamlLoader::new("/path/to/config.yaml");
        assert_eq!(
            ConfigLoader::<TestConfig>::source_info(&loader),
            "/path/to/config.yaml"
        );
    }

    #[test]
    fn test_yaml_loader_with_yml_extension() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.yml");

        let loader = YamlLoader::new(&config_path);

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
            items: vec!["a".to_string()],
        };

        loader.save(&config).unwrap();
        let loaded: TestConfig = loader.load().unwrap();
        assert_eq!(loaded, config);
    }
}
