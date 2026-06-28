//! Layered configuration loader for merging multiple sources.

use crate::core::{Error, Result};
use crate::loader::ConfigLoader;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// Strategy for merging configurations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeStrategy {
    /// Override: Later sources completely replace earlier ones
    Override,
    /// Deep merge: Merge nested structures (requires serde_json for now)
    DeepMerge,
}

/// Layered configuration loader that merges multiple sources.
///
/// Loads configuration from multiple loaders in order, merging them
/// according to the specified strategy.
///
/// # Examples
///
/// ```rust,no_run
/// use config_manager::prelude::*;
/// use config_manager::loader::{LayeredLoader, MergeStrategy};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct MyConfig {
///     name: String,
///     port: u16,
/// }
///
/// fn main() -> Result<()> {
///     let loader = LayeredLoader::new(MergeStrategy::Override)
///         .add_layer(JsonLoader::new("base.json"))
///         .add_layer(JsonLoader::new("override.json"));
///
///     let config: MyConfig = loader.load()?;
///     Ok(())
/// }
/// ```
pub struct LayeredLoader<T> {
    layers: Vec<Box<dyn ConfigLoader<T>>>,
    strategy: MergeStrategy,
}

impl<T> LayeredLoader<T>
where
    T: DeserializeOwned + Serialize + Clone,
{
    /// Create a new layered loader with the specified merge strategy.
    pub fn new(strategy: MergeStrategy) -> Self {
        Self {
            layers: Vec::new(),
            strategy,
        }
    }

    /// Add a configuration layer.
    ///
    /// Layers are merged in the order they are added.
    pub fn add_layer(mut self, loader: impl ConfigLoader<T> + 'static) -> Self {
        self.layers.push(Box::new(loader));
        self
    }

    /// Add a boxed configuration layer.
    pub fn add_layer_boxed(mut self, loader: Box<dyn ConfigLoader<T>>) -> Self {
        self.layers.push(loader);
        self
    }

    /// Set the merge strategy.
    pub fn with_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Merge two configurations according to the strategy.
    fn merge(&self, base: T, override_config: T) -> Result<T> {
        match self.strategy {
            MergeStrategy::Override => {
                // Simple: override completely replaces base
                Ok(override_config)
            }
            MergeStrategy::DeepMerge => {
                // Deep merge using serde_json
                self.deep_merge(base, override_config)
            }
        }
    }

    /// Perform deep merge of two configurations.
    fn deep_merge(&self, base: T, override_config: T) -> Result<T> {
        // Convert to serde_json::Value for merging
        let mut base_value = serde_json::to_value(&base)
            .map_err(|e| Error::Custom(format!("Failed to convert base to JSON: {}", e)))?;

        let override_value = serde_json::to_value(&override_config)
            .map_err(|e| Error::Custom(format!("Failed to convert override to JSON: {}", e)))?;

        // Merge recursively
        merge_json_values(&mut base_value, override_value);

        // Convert back to T
        serde_json::from_value(base_value)
            .map_err(|e| Error::Custom(format!("Failed to convert merged JSON back: {}", e)))
    }
}

/// Recursively merge two JSON values.
fn merge_json_values(base: &mut serde_json::Value, override_val: serde_json::Value) {
    use serde_json::Value;

    match (base, override_val) {
        (Value::Object(base_map), Value::Object(override_map)) => {
            // Merge objects recursively
            for (key, override_value) in override_map {
                if let Some(base_value) = base_map.get_mut(&key) {
                    // Key exists in both - merge recursively
                    merge_json_values(base_value, override_value);
                } else {
                    // Key only in override - insert
                    base_map.insert(key, override_value);
                }
            }
        }
        (Value::Array(base_arr), Value::Array(override_arr)) => {
            // For arrays, override replaces completely (could be customized)
            *base_arr = override_arr;
        }
        (base_val, override_val) => {
            // For primitives, override wins
            *base_val = override_val;
        }
    }
}

impl<T> ConfigLoader<T> for LayeredLoader<T>
where
    T: DeserializeOwned + Serialize + Clone + Send + Sync,
{
    fn load(&self) -> Result<T> {
        if self.layers.is_empty() {
            return Err(Error::LoadError(
                "No configuration layers defined".to_string(),
            ));
        }

        // Load base layer
        let mut result = self.layers[0].load()?;

        // Merge additional layers
        for loader in &self.layers[1..] {
            if loader.exists() {
                let override_config = loader.load()?;
                result = self.merge(result, override_config)?;
            }
            // Skip layers that don't exist (optional overrides)
        }

        Ok(result)
    }

    fn save(&self, config: &T) -> Result<()> {
        // Save to the last layer (typically the most specific override)
        if let Some(last_layer) = self.layers.last() {
            last_layer.save(config)
        } else {
            Err(Error::SaveError(
                "No configuration layers defined".to_string(),
            ))
        }
    }

    fn exists(&self) -> bool {
        // Exists if at least one layer exists
        self.layers.iter().any(|loader| loader.exists())
    }

    fn source_info(&self) -> String {
        format!(
            "Layered ({} layers, {:?})",
            self.layers.len(),
            self.strategy
        )
    }
}

impl<T> Debug for LayeredLoader<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayeredLoader")
            .field("layers", &self.layers.len())
            .field("strategy", &self.strategy)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::JsonLoader;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestConfig {
        name: String,
        port: u16,
        debug: bool,
    }

    #[test]
    fn test_layered_override_strategy() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join("base.json");
        let override_path = temp_dir.path().join("override.json");

        // Create base config
        let base_config = TestConfig {
            name: "base".to_string(),
            port: 3000,
            debug: false,
        };

        let base_loader = JsonLoader::new(&base_path);
        base_loader.save(&base_config).unwrap();

        // Create override config
        let override_config = TestConfig {
            name: "override".to_string(),
            port: 8080,
            debug: true,
        };

        let override_loader = JsonLoader::new(&override_path);
        override_loader.save(&override_config).unwrap();

        // Load with layered loader
        let layered = LayeredLoader::new(MergeStrategy::Override)
            .add_layer(base_loader)
            .add_layer(override_loader);

        let result: TestConfig = layered.load().unwrap();

        // Override strategy: second layer completely replaces first
        assert_eq!(result, override_config);
    }

    #[test]
    fn test_layered_deep_merge_strategy() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join("base.json");
        let override_path = temp_dir.path().join("override.json");

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct ComplexConfig {
            name: String,
            settings: Settings,
        }

        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct Settings {
            port: u16,
            debug: bool,
        }

        // Create base config
        let base_config = ComplexConfig {
            name: "base".to_string(),
            settings: Settings {
                port: 3000,
                debug: false,
            },
        };

        let base_loader = JsonLoader::new(&base_path);
        base_loader.save(&base_config).unwrap();

        // Create partial override
        let override_config = ComplexConfig {
            name: "override".to_string(),
            settings: Settings {
                port: 8080,
                debug: true,
            },
        };

        let override_loader = JsonLoader::new(&override_path);
        override_loader.save(&override_config).unwrap();

        // Load with deep merge
        let layered = LayeredLoader::new(MergeStrategy::DeepMerge)
            .add_layer(base_loader)
            .add_layer(override_loader);

        let result: ComplexConfig = layered.load().unwrap();

        // Deep merge: override values win
        assert_eq!(result.name, "override");
        assert_eq!(result.settings.port, 8080);
        assert!(result.settings.debug);
    }

    #[test]
    fn test_layered_optional_overrides() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join("base.json");
        let nonexistent_path = temp_dir.path().join("nonexistent.json");

        // Create base config
        let base_config = TestConfig {
            name: "base".to_string(),
            port: 3000,
            debug: false,
        };

        let base_loader = JsonLoader::new(&base_path);
        base_loader.save(&base_config).unwrap();

        // Layer with non-existent override (should skip it)
        let layered = LayeredLoader::new(MergeStrategy::Override)
            .add_layer(base_loader)
            .add_layer(JsonLoader::new(nonexistent_path));

        let result: TestConfig = layered.load().unwrap();

        // Should get base config since override doesn't exist
        assert_eq!(result, base_config);
    }

    #[test]
    fn test_layered_save() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join("base.json");
        let override_path = temp_dir.path().join("override.json");

        let layered = LayeredLoader::new(MergeStrategy::Override)
            .add_layer(JsonLoader::new(&base_path))
            .add_layer(JsonLoader::new(&override_path));

        let config = TestConfig {
            name: "test".to_string(),
            port: 3000,
            debug: false,
        };

        // Save should write to last layer
        layered.save(&config).unwrap();

        // Override file should exist and contain the config
        assert!(override_path.exists());
        assert!(!base_path.exists()); // Base not created

        let override_loader = JsonLoader::new(&override_path);
        let loaded: TestConfig = override_loader.load().unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_layered_no_layers() {
        let layered: LayeredLoader<TestConfig> = LayeredLoader::new(MergeStrategy::Override);

        let result = layered.load();
        assert!(matches!(result, Err(Error::LoadError(_))));
    }

    #[test]
    fn test_layered_exists() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join("base.json");
        let nonexistent_path = temp_dir.path().join("nonexistent.json");

        let base_config = TestConfig {
            name: "test".to_string(),
            port: 3000,
            debug: false,
        };

        let base_loader = JsonLoader::new(&base_path);
        base_loader.save(&base_config).unwrap();

        let layered: LayeredLoader<TestConfig> = LayeredLoader::new(MergeStrategy::Override)
            .add_layer(base_loader)
            .add_layer(JsonLoader::new(nonexistent_path));

        // Should return true if any layer exists
        assert!(layered.exists());
    }
}
