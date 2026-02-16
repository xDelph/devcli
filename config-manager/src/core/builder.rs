//! Builder pattern for ConfigManager.

use crate::core::{ConfigManager, Error, Result};
use crate::loader::ConfigLoader;
use crate::validation::Validator;
use std::fmt::Debug;
use std::marker::PhantomData;

/// Builder for constructing a ConfigManager with optional components.
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
/// }
///
/// fn main() -> Result<()> {
///     let manager = ConfigManager::<MyConfig>::builder()
///         .loader(JsonLoader::new("config.json"))
///         .build()?;
///
///     Ok(())
/// }
/// ```
pub struct ConfigManagerBuilder<T> {
    loader: Option<Box<dyn ConfigLoader<T>>>,
    validators: Vec<Box<dyn Validator<T>>>,
    _phantom: PhantomData<T>,
}

impl<T> ConfigManagerBuilder<T>
where
    T: Send + Sync + Debug,
{
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            loader: None,
            validators: Vec::new(),
            _phantom: PhantomData,
        }
    }

    /// Set the configuration loader.
    ///
    /// Required before calling `build()`.
    pub fn loader(mut self, loader: impl ConfigLoader<T> + 'static) -> Self {
        self.loader = Some(Box::new(loader));
        self
    }

    /// Set the configuration loader (boxed version).
    pub fn loader_boxed(mut self, loader: Box<dyn ConfigLoader<T>>) -> Self {
        self.loader = Some(loader);
        self
    }

    /// Add a validator.
    ///
    /// Multiple validators can be added and will run in order.
    pub fn validator(mut self, validator: Box<dyn Validator<T>>) -> Self {
        self.validators.push(validator);
        self
    }

    /// Add multiple validators at once.
    pub fn validators(mut self, validators: Vec<Box<dyn Validator<T>>>) -> Self {
        self.validators.extend(validators);
        self
    }

    /// Build the ConfigManager.
    ///
    /// # Errors
    ///
    /// Returns `Error::Custom` if required components (loader) are not set.
    pub fn build(self) -> Result<ConfigManager<T>> {
        let loader = self
            .loader
            .ok_or_else(|| Error::Custom("Loader is required".to_string()))?;

        Ok(ConfigManager::new(loader, self.validators))
    }
}

impl<T> Default for ConfigManagerBuilder<T>
where
    T: Send + Sync + Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::JsonLoader;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestConfig {
        name: String,
    }

    #[test]
    fn test_builder_no_loader() {
        let result = ConfigManager::<TestConfig>::builder().build();
        assert!(matches!(result, Err(Error::Custom(_))));
    }

    #[test]
    fn test_builder_with_loader() {
        let result = ConfigManager::<TestConfig>::builder()
            .loader(JsonLoader::new("/tmp/test.json"))
            .build();

        assert!(result.is_ok());
    }
}
