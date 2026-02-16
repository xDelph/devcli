//! Integration tests for layered configuration with encryption.

use config_manager::loader::{
    AsyncConfigLoader, ConfigLoader, EncryptedLoader, JsonLoader, LayeredLoader, MergeStrategy,
};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct AppConfig {
    name: String,
    api_key: String,
    database: DatabaseConfig,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    password: String,
}

#[test]
fn test_layered_with_encryption() {
    let temp_dir = TempDir::new().unwrap();

    // Create base config (unencrypted)
    let base_path = temp_dir.path().join("base.json");
    let base_loader = JsonLoader::new(&base_path);
    let base_config = AppConfig {
        name: "myapp".to_string(),
        api_key: "public-key".to_string(),
        database: DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            password: "default-pass".to_string(),
        },
    };
    base_loader.save(&base_config).unwrap();

    // Create encrypted override for production secrets
    let secrets_path = temp_dir.path().join("secrets.json");
    let secrets_base = JsonLoader::new(&secrets_path);
    let secrets_loader = EncryptedLoader::new(secrets_base, "production-password");

    let secrets_config = AppConfig {
        name: "myapp".to_string(),
        api_key: "secret-production-key".to_string(),
        database: DatabaseConfig {
            host: "prod.example.com".to_string(),
            port: 5432,
            password: "super-secret-pass".to_string(),
        },
    };
    secrets_loader.save(&secrets_config).unwrap();

    // Verify secrets are encrypted on disk
    let raw_secrets = std::fs::read_to_string(&secrets_path).unwrap();
    assert!(!raw_secrets.contains("super-secret-pass"));
    assert!(!raw_secrets.contains("secret-production-key"));

    // Load layered configuration
    let layered: LayeredLoader<AppConfig> = LayeredLoader::new(MergeStrategy::DeepMerge)
        .add_layer(base_loader)
        .add_layer(secrets_loader);

    let config: AppConfig = layered.load().unwrap();

    // Verify merged configuration
    assert_eq!(config.name, "myapp");
    assert_eq!(config.api_key, "secret-production-key"); // From encrypted
    assert_eq!(config.database.host, "prod.example.com"); // From encrypted
    assert_eq!(config.database.password, "super-secret-pass"); // From encrypted
}

#[test]
fn test_layered_override_strategy() {
    let temp_dir = TempDir::new().unwrap();

    // Layer 1: defaults
    let defaults_path = temp_dir.path().join("defaults.json");
    let defaults_loader = JsonLoader::new(&defaults_path);
    let defaults = AppConfig {
        name: "app".to_string(),
        api_key: "default-key".to_string(),
        database: DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            password: "default".to_string(),
        },
    };
    defaults_loader.save(&defaults).unwrap();

    // Layer 2: production overrides
    let prod_path = temp_dir.path().join("production.json");
    let prod_loader = JsonLoader::new(&prod_path);
    let prod = AppConfig {
        name: "production-app".to_string(),
        api_key: "prod-key".to_string(),
        database: DatabaseConfig {
            host: "prod.db.example.com".to_string(),
            port: 5433,
            password: "prod-password".to_string(),
        },
    };
    prod_loader.save(&prod).unwrap();

    // With Override strategy, later layer completely replaces
    let layered: LayeredLoader<AppConfig> = LayeredLoader::new(MergeStrategy::Override)
        .add_layer(defaults_loader)
        .add_layer(prod_loader);

    let config: AppConfig = layered.load().unwrap();

    assert_eq!(config, prod); // Exact match to production config
}

#[test]
fn test_three_layer_configuration() {
    let temp_dir = TempDir::new().unwrap();

    // Base defaults
    let base_config = AppConfig {
        name: "app".to_string(),
        api_key: "dev-key".to_string(),
        database: DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            password: "dev-pass".to_string(),
        },
    };

    // Environment-specific (staging)
    let env_config = AppConfig {
        name: "staging-app".to_string(),
        api_key: "staging-key".to_string(),
        database: DatabaseConfig {
            host: "staging.db.example.com".to_string(),
            port: 5432,
            password: "staging-pass".to_string(),
        },
    };

    // User overrides (optional)
    let user_config = AppConfig {
        name: "staging-app".to_string(),
        api_key: "custom-key".to_string(),
        database: DatabaseConfig {
            host: "staging.db.example.com".to_string(),
            port: 5433, // Custom port
            password: "staging-pass".to_string(),
        },
    };

    // Save all layers
    let base_path = temp_dir.path().join("base.json");
    JsonLoader::new(&base_path).save(&base_config).unwrap();

    let env_path = temp_dir.path().join("staging.json");
    JsonLoader::new(&env_path).save(&env_config).unwrap();

    let user_path = temp_dir.path().join("user.json");
    JsonLoader::new(&user_path).save(&user_config).unwrap();

    // Load with deep merge
    let layered: LayeredLoader<AppConfig> = LayeredLoader::new(MergeStrategy::DeepMerge)
        .add_layer(JsonLoader::new(&base_path))
        .add_layer(JsonLoader::new(&env_path))
        .add_layer(JsonLoader::new(&user_path));

    let config: AppConfig = layered.load().unwrap();

    // Verify each layer's contribution
    assert_eq!(config.name, "staging-app"); // From env layer
    assert_eq!(config.api_key, "custom-key"); // From user layer
    assert_eq!(config.database.host, "staging.db.example.com"); // From env layer
    assert_eq!(config.database.port, 5433); // From user layer (custom)
}
