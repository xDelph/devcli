//! Core validator trait definition.

use super::result::ValidationResult;

/// Trait for validating configuration.
///
/// Validators check configuration for correctness according to custom rules.
///
/// # Type Parameters
///
/// * `T` - The configuration type to validate
///
/// # Example
///
/// ```rust
/// use config_manager::validation::{Validator, ValidationResult, ValidationError};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct Config {
///     port: u16,
/// }
///
/// struct PortValidator;
///
/// impl Validator<Config> for PortValidator {
///     fn validate(&self, config: &Config) -> ValidationResult {
///         let mut result = ValidationResult::new();
///
///         if config.port == 0 {
///             result.add_error(ValidationError::new(
///                 "port",
///                 "Port cannot be 0"
///             ));
///         }
///
///         result
///     }
/// }
/// ```
pub trait Validator<T>: Send + Sync {
    /// Validate the entire configuration.
    ///
    /// Returns a ValidationResult containing any errors or warnings found.
    fn validate(&self, config: &T) -> ValidationResult;

    /// Validate a specific field (optional).
    ///
    /// Default implementation validates the entire config.
    fn validate_field(&self, config: &T, _field: &str) -> ValidationResult {
        self.validate(config)
    }

    /// Check if configuration is valid (no errors).
    ///
    /// Default implementation checks if validate() returns no errors.
    fn is_valid(&self, config: &T) -> bool {
        self.validate(config).is_valid()
    }
}
