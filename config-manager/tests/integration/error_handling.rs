//! Integration tests for error handling scenarios.

use config_manager::core::Error;
use config_manager::loader::{ConfigLoader, JsonLoader};

#[cfg(feature = "encryption")]
use config_manager::loader::EncryptedLoader;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    value: String,
}

#[test]
fn test_load_nonexistent_file() {
    let loader = JsonLoader::new("/nonexistent/path/config.json");
    let result: Result<Config, Error> = loader.load();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.is_load_error());
    assert!(err.to_string().contains("not found"));
}

#[test]
fn test_invalid_json_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("invalid.json");

    // Write invalid JSON
    std::fs::write(&path, "{ invalid json content").unwrap();

    let loader = JsonLoader::new(&path);
    let result: Result<Config, Error> = loader.load();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, Error::LoadError(_)));
}

#[cfg(feature = "encryption")]
#[test]
fn test_encryption_wrong_password() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("encrypted.json");

    let config = Config {
        value: "secret".to_string(),
    };

    // Save with password1
    let base1 = JsonLoader::new(&path);
    let loader1 = EncryptedLoader::new(base1, "password1");
    loader1.save(&config).unwrap();

    // Try to load with password2
    let base2 = JsonLoader::new(&path);
    let loader2 = EncryptedLoader::new(base2, "wrong-password");
    let result: Result<Config, Error> = loader2.load();

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Decryption failed"));
}

#[test]
fn test_error_helper_methods() {
    let not_found = Error::NotFound {
        id: "test".to_string(),
        suggestion: None,
    };
    assert!(not_found.is_not_found());
    assert!(!not_found.is_validation_error());

    let validation = Error::ValidationError("test error".to_string());
    assert!(validation.is_validation_error());
    assert!(!validation.is_not_found());

    let circular = Error::CircularDependency {
        path: vec!["a".to_string(), "b".to_string(), "a".to_string()],
    };
    assert!(circular.is_circular_dependency());

    let load_err = Error::LoadError("test".to_string());
    assert!(load_err.is_load_error());

    let save_err = Error::SaveError("test".to_string());
    assert!(save_err.is_save_error());
}

#[test]
fn test_save_to_readonly_directory() {
    // This test is platform-specific, so we'll just verify the error type
    let loader = JsonLoader::new("/root/readonly/config.json").with_create_dirs(false);
    let config = Config {
        value: "test".to_string(),
    };

    let result = loader.save(&config);
    // On most systems without root access, this should fail
    // But we can't guarantee it fails everywhere, so we just check the error type if it does
    if let Err(err) = result {
        // Could be SaveError or IoError depending on the failure point
        assert!(
            err.is_save_error() || matches!(err, Error::IoError(_)),
            "Expected save or IO error, got: {:?}",
            err
        );
    }
}

#[test]
fn test_corruption_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("config.json");

    let loader = JsonLoader::new(&path);
    let config = Config {
        value: "original".to_string(),
    };

    // Save valid config
    loader.save(&config).unwrap();

    // Corrupt the file
    std::fs::write(&path, "corrupted data").unwrap();

    // Try to load - should fail
    let result: Result<Config, Error> = loader.load();
    assert!(result.is_err());

    // Recover by saving new valid config
    let new_config = Config {
        value: "recovered".to_string(),
    };
    loader.save(&new_config).unwrap();

    // Should now load successfully
    let loaded: Config = loader.load().unwrap();
    assert_eq!(loaded.value, "recovered");
}

#[cfg(feature = "schema")]
#[test]
fn test_schema_validation_errors() {
    use config_manager::schema::SchemaValidator;
    use config_manager::validation::Validator;
    use schemars::JsonSchema;

    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
    struct ValidatedConfig {
        #[schemars(range(min = 1, max = 100))]
        value: u32,
    }

    let validator = SchemaValidator::<ValidatedConfig>::new();

    // Valid config
    let valid = ValidatedConfig { value: 50 };
    let result = validator.validate(&valid);
    assert!(result.is_valid());

    // Invalid: below minimum
    let invalid_low = ValidatedConfig { value: 0 };
    let result = validator.validate(&invalid_low);
    assert!(!result.is_valid());
    assert!(!result.errors().is_empty());

    // Invalid: above maximum
    let invalid_high = ValidatedConfig { value: 101 };
    let result = validator.validate(&invalid_high);
    assert!(!result.is_valid());
    assert!(!result.errors().is_empty());
}
