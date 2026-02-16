//! Integration tests for async workflows.

use config_manager::loader::{AsyncConfigLoader, AsyncJsonLoader};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tokio;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Config {
    value: String,
}

#[tokio::test]
async fn test_concurrent_async_loads() {
    let temp_dir = TempDir::new().unwrap();

    // Create multiple config files
    let configs: Vec<_> = (0..5)
        .map(|i| {
            let path = temp_dir.path().join(format!("config_{}.json", i));
            let config = Config {
                value: format!("value_{}", i),
            };
            (path, config)
        })
        .collect();

    // Save all configs
    for (path, config) in &configs {
        let loader = AsyncJsonLoader::new(path);
        loader.save(config).await.unwrap();
    }

    // Load all configs concurrently
    let handles: Vec<_> = configs
        .iter()
        .map(|(path, expected_config)| {
            let path = path.clone();
            let expected = expected_config.clone();
            tokio::spawn(async move {
                let loader = AsyncJsonLoader::new(&path);
                let loaded: Config = loader.load().await.unwrap();
                assert_eq!(loaded, expected);
                loaded
            })
        })
        .collect();

    // Wait for all to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_async_read_write_cycle() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("cycle.json");

    let loader = AsyncJsonLoader::new(&path);

    // Write initial config
    let config1 = Config {
        value: "initial".to_string(),
    };
    loader.save(&config1).await.unwrap();

    // Read it back
    let loaded1: Config = loader.load().await.unwrap();
    assert_eq!(loaded1, config1);

    // Update and save
    let config2 = Config {
        value: "updated".to_string(),
    };
    loader.save(&config2).await.unwrap();

    // Read updated version
    let loaded2: Config = loader.load().await.unwrap();
    assert_eq!(loaded2, config2);
    assert_ne!(loaded2.value, config1.value);
}

#[tokio::test]
async fn test_async_multiple_formats() {
    let temp_dir = TempDir::new().unwrap();

    let json_path = temp_dir.path().join("config.json");
    let json_loader = AsyncJsonLoader::new(&json_path);

    #[cfg(feature = "toml")]
    let toml_path = temp_dir.path().join("config.toml");
    #[cfg(feature = "toml")]
    let toml_loader = config_manager::loader::AsyncTomlLoader::new(&toml_path);

    #[cfg(feature = "yaml")]
    let yaml_path = temp_dir.path().join("config.yaml");
    #[cfg(feature = "yaml")]
    let yaml_loader = config_manager::loader::AsyncYamlLoader::new(&yaml_path);

    let config = Config {
        value: "test-value".to_string(),
    };

    // Save to all formats concurrently
    let json_save = json_loader.save(&config);

    #[cfg(feature = "toml")]
    let toml_save = toml_loader.save(&config);

    #[cfg(feature = "yaml")]
    let yaml_save = yaml_loader.save(&config);

    json_save.await.unwrap();

    #[cfg(feature = "toml")]
    toml_save.await.unwrap();

    #[cfg(feature = "yaml")]
    yaml_save.await.unwrap();

    // Verify all files exist and can be loaded
    let loaded_json: Config = json_loader.load().await.unwrap();
    assert_eq!(loaded_json, config);

    #[cfg(feature = "toml")]
    {
        let loaded_toml: Config = toml_loader.load().await.unwrap();
        assert_eq!(loaded_toml, config);
    }

    #[cfg(feature = "yaml")]
    {
        let loaded_yaml: Config = yaml_loader.load().await.unwrap();
        assert_eq!(loaded_yaml, config);
    }
}
