//! Configuration manager orchestration.

use crate::core::{ConfigManagerBuilder, Result};
use crate::loader::ConfigLoader;
use crate::validation::{ValidationResult, Validator};
use std::fmt::Debug;
use std::marker::PhantomData;

/// Main configuration manager that orchestrates loading, validation,
/// resolution, and dependency management.
///
/// # Type Parameters
///
/// * `T` - The configuration type
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
///     version: String,
/// }
///
/// fn main() -> Result<()> {
///     let manager = ConfigManager::<MyConfig>::builder()
///         .loader(JsonLoader::new("config.json"))
///         .build()?;
///
///     let config = manager.load()?;
///     println!("Config: {} v{}", config.name, config.version);
///
///     Ok(())
/// }
/// ```
pub struct ConfigManager<T> {
    loader: Box<dyn ConfigLoader<T>>,
    validators: Vec<Box<dyn Validator<T>>>,
    _phantom: PhantomData<T>,
}

impl<T> ConfigManager<T>
where
    T: Send + Sync + Debug,
{
    /// Create a new builder for constructing a ConfigManager.
    pub fn builder() -> ConfigManagerBuilder<T> {
        ConfigManagerBuilder::new()
    }

    /// Create a ConfigManager with just a loader (no validation).
    pub fn with_loader(loader: Box<dyn ConfigLoader<T>>) -> Self {
        Self {
            loader,
            validators: Vec::new(),
            _phantom: PhantomData,
        }
    }

    /// Internal constructor for builder (not public API).
    pub(crate) fn new(
        loader: Box<dyn ConfigLoader<T>>,
        validators: Vec<Box<dyn Validator<T>>>,
    ) -> Self {
        Self {
            loader,
            validators,
            _phantom: PhantomData,
        }
    }

    /// Load configuration from the source.
    ///
    /// # Errors
    ///
    /// Returns `Error::LoadError` if loading fails.
    pub fn load(&self) -> Result<T> {
        self.loader.load()
    }

    /// Save configuration to the source.
    ///
    /// Validates before saving if validators are configured.
    ///
    /// # Errors
    ///
    /// * `Error::ValidationError` - Configuration is invalid
    /// * `Error::SaveError` - Failed to save
    pub fn save(&self, config: &T) -> Result<()> {
        // Validate before saving
        let validation_result = self.validate(config);
        if !validation_result.is_valid() {
            return Err(crate::core::Error::ValidationError(
                validation_result.to_string(),
            ));
        }

        self.loader.save(config)
    }

    /// Reload configuration from the source.
    ///
    /// Alias for `load()`.
    pub fn reload(&self) -> Result<T> {
        self.loader.reload()
    }

    /// Check if the configuration source exists.
    pub fn exists(&self) -> bool {
        self.loader.exists()
    }

    /// Get information about the configuration source.
    pub fn source_info(&self) -> String {
        self.loader.source_info()
    }

    /// Validate configuration.
    ///
    /// Runs all registered validators and returns a combined result.
    pub fn validate(&self, config: &T) -> ValidationResult {
        let mut combined = ValidationResult::new();

        for validator in &self.validators {
            let result = validator.validate(config);
            combined.merge(result);
        }

        combined
    }

    /// Check if configuration is valid (no errors).
    pub fn is_valid(&self, config: &T) -> bool {
        self.validate(config).is_valid()
    }

    /// Add a validator to the manager.
    pub fn add_validator(&mut self, validator: Box<dyn Validator<T>>) {
        self.validators.push(validator);
    }

    /// Load and validate configuration in one step.
    ///
    /// # Errors
    ///
    /// * `Error::LoadError` - Failed to load
    /// * `Error::ValidationError` - Configuration is invalid
    pub fn load_and_validate(&self) -> Result<T> {
        let config = self.load()?;
        let validation_result = self.validate(&config);

        if !validation_result.is_valid() {
            return Err(crate::core::Error::ValidationError(
                validation_result.to_string(),
            ));
        }

        Ok(config)
    }
}

impl<T> Debug for ConfigManager<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigManager")
            .field("source", &self.loader.source_info())
            .field("validators", &self.validators.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::JsonLoader;
    use crate::validation::{ValidationError, Validator};
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestConfig {
        name: String,
        count: u32,
    }

    struct CountValidator;
    impl Validator<TestConfig> for CountValidator {
        fn validate(&self, config: &TestConfig) -> ValidationResult {
            let mut result = ValidationResult::new();
            if config.count == 0 {
                result.add_error(ValidationError::new("count", "Count cannot be 0"));
            }
            result
        }
    }

    #[test]
    fn test_config_manager_load_save() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.json");

        let manager = ConfigManager::builder()
            .loader(JsonLoader::new(&config_path))
            .build()
            .unwrap();

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
        };

        manager.save(&config).unwrap();
        let loaded = manager.load().unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_config_manager_validation() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.json");

        let manager = ConfigManager::builder()
            .loader(JsonLoader::new(&config_path))
            .validator(Box::new(CountValidator))
            .build()
            .unwrap();

        let valid_config = TestConfig {
            name: "test".to_string(),
            count: 42,
        };

        assert!(manager.is_valid(&valid_config));

        let invalid_config = TestConfig {
            name: "test".to_string(),
            count: 0,
        };

        assert!(!manager.is_valid(&invalid_config));

        // Should fail to save invalid config
        let result = manager.save(&invalid_config);
        assert!(matches!(
            result,
            Err(crate::core::Error::ValidationError(_))
        ));
    }

    #[test]
    fn test_config_manager_exists() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.json");

        let manager = ConfigManager::builder()
            .loader(JsonLoader::new(&config_path))
            .build()
            .unwrap();

        assert!(!manager.exists());

        let config = TestConfig {
            name: "test".to_string(),
            count: 42,
        };

        manager.save(&config).unwrap();
        assert!(manager.exists());
    }
}
