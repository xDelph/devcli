//! JSON Schema generation and validation.
//!
//! This module provides schema generation from Rust types and
//! validation of configurations against schemas.

use crate::core::{Error, Result};
use crate::validation::{ValidationError, ValidationResult, Validator};
use jsonschema::JSONSchema;
use schemars::{schema::RootSchema, JsonSchema};
use serde::Serialize;

/// Schema generator and validator.
///
/// Generates JSON schemas from Rust types and validates configurations.
///
/// # Examples
///
/// ```rust
/// use config_manager::schema::SchemaValidator;
/// use config_manager::validation::Validator;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// struct MyConfig {
///     #[schemars(range(min = 1, max = 65535))]
///     port: u16,
///
///     #[schemars(regex(pattern = r"^[a-z0-9-]+$"))]
///     name: String,
/// }
///
/// fn main() -> config_manager::core::Result<()> {
///     let validator = SchemaValidator::<MyConfig>::new();
///
///     let config = MyConfig {
///         port: 8080,
///         name: "my-app".to_string(),
///     };
///
///     let result = validator.validate(&config);
///     assert!(result.is_valid());
///
///     Ok(())
/// }
/// ```
pub struct SchemaValidator<T> {
    schema: RootSchema,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> SchemaValidator<T>
where
    T: JsonSchema,
{
    /// Create a new schema validator for type T.
    pub fn new() -> Self {
        let schema = schemars::schema_for!(T);
        Self {
            schema,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the JSON schema as a RootSchema.
    pub fn schema(&self) -> &RootSchema {
        &self.schema
    }

    /// Get the JSON schema as a JSON string.
    pub fn schema_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.schema)
            .map_err(|e| Error::Custom(format!("Failed to serialize schema: {}", e)))
    }
}

impl<T> Default for SchemaValidator<T>
where
    T: JsonSchema,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Validator<T> for SchemaValidator<T>
where
    T: JsonSchema + Serialize + Send + Sync,
{
    fn validate(&self, config: &T) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Convert config to JSON value
        let config_value = match serde_json::to_value(config) {
            Ok(v) => v,
            Err(e) => {
                result.add_error(ValidationError::new(
                    "serialization",
                    format!("Failed to serialize config: {}", e),
                ));
                return result;
            }
        };

        // We need to compile the schema, but validate() takes &self
        // So we'll create a new compiled schema each time
        // In practice, you'd cache this or use a RefCell
        let schema_value = match serde_json::to_value(&self.schema) {
            Ok(v) => v,
            Err(e) => {
                result.add_error(ValidationError::new(
                    "schema",
                    format!("Failed to serialize schema: {}", e),
                ));
                return result;
            }
        };

        let compiled = match JSONSchema::compile(&schema_value) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(ValidationError::new(
                    "schema",
                    format!("Failed to compile schema: {}", e),
                ));
                return result;
            }
        };

        // Validate
        if let Err(errors) = compiled.validate(&config_value) {
            for error in errors {
                result.add_error(ValidationError::new(
                    error.instance_path.to_string(),
                    error.to_string(),
                ));
            }
        }

        result
    }
}

impl<T> std::fmt::Debug for SchemaValidator<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchemaValidator")
            .field("schema", &"<RootSchema>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
    struct TestConfig {
        #[schemars(range(min = 1, max = 65535))]
        port: u16,

        #[schemars(length(min = 1, max = 50))]
        name: String,
    }

    #[test]
    fn test_schema_generation() {
        let validator = SchemaValidator::<TestConfig>::new();
        let schema = validator.schema();

        assert!(schema.schema.metadata.is_some());
    }

    #[test]
    fn test_schema_json() {
        let validator = SchemaValidator::<TestConfig>::new();
        let json = validator.schema_json().unwrap();

        assert!(json.contains("port"));
        assert!(json.contains("name"));
    }

    #[test]
    fn test_valid_config() {
        let validator = SchemaValidator::<TestConfig>::new();

        let config = TestConfig {
            port: 8080,
            name: "test".to_string(),
        };

        let result = validator.validate(&config);
        assert!(result.is_valid());
    }

    #[test]
    fn test_invalid_port_range() {
        let validator = SchemaValidator::<TestConfig>::new();

        let config = TestConfig {
            port: 0, // Invalid: below minimum
            name: "test".to_string(),
        };

        let result = validator.validate(&config);
        assert!(!result.is_valid());
        assert!(!result.errors().is_empty());
    }

    #[test]
    fn test_invalid_name_length() {
        let validator = SchemaValidator::<TestConfig>::new();

        let config = TestConfig {
            port: 8080,
            name: "".to_string(), // Invalid: empty string
        };

        let result = validator.validate(&config);
        assert!(!result.is_valid());
        assert!(!result.errors().is_empty());
    }
}
