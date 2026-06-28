//! Integration tests for async workflows.

use config_manager::core::Result;
use config_manager::loader::{AsyncConfigLoader, AsyncJsonLoader};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Config {
    value: String,
}

async fn save_config(loader: &AsyncJsonLoader, config: &Config) -> Result<()> {
    AsyncConfigLoader::<Config>::save(loader, config).await
}

async fn load_config(loader: &AsyncJsonLoader) -> Result<Config> {
    AsyncConfigLoader::<Config>::load(loader).await
}

#[tokio::test]
async fn test_concurrent_async_loads() {
    let temp_dir = TempDir::new().unwrap();

    let configs: Vec<_> = (0..5)
        .map(|i| {
            let path = temp_dir.path().join(format!("config_{}.json", i));
            let config = Config {
                value: format!("value_{}", i),
            };
            (path, config)
        })
        .collect();

    for (path, config) in &configs {
        let loader = AsyncJsonLoader::new(path);
        save_config(&loader, config).await.unwrap();
    }

    let handles: Vec<_> = configs
        .iter()
        .map(|(path, expected_config)| {
            let path = path.clone();
            let expected = expected_config.clone();
            tokio::spawn(async move {
                let loader = AsyncJsonLoader::new(&path);
                let loaded = load_config(&loader).await.unwrap();
                assert_eq!(loaded, expected);
            })
        })
        .collect();

    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_async_read_write_cycle() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().join("cycle.json");
    let loader = AsyncJsonLoader::new(&path);

    let config1 = Config {
        value: "initial".to_string(),
    };
    save_config(&loader, &config1).await.unwrap();

    let loaded1 = load_config(&loader).await.unwrap();
    assert_eq!(loaded1, config1);

    let config2 = Config {
        value: "updated".to_string(),
    };
    save_config(&loader, &config2).await.unwrap();

    let loaded2 = load_config(&loader).await.unwrap();
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

    save_config(&json_loader, &config).await.unwrap();

    #[cfg(feature = "toml")]
    {
        AsyncConfigLoader::<Config>::save(&toml_loader, &config)
            .await
            .unwrap();
    }

    #[cfg(feature = "yaml")]
    {
        AsyncConfigLoader::<Config>::save(&yaml_loader, &config)
            .await
            .unwrap();
    }

    let loaded_json = load_config(&json_loader).await.unwrap();
    assert_eq!(loaded_json, config);

    #[cfg(feature = "toml")]
    {
        let loaded_toml = AsyncConfigLoader::<Config>::load(&toml_loader)
            .await
            .unwrap();
        assert_eq!(loaded_toml, config);
    }

    #[cfg(feature = "yaml")]
    {
        let loaded_yaml = AsyncConfigLoader::<Config>::load(&yaml_loader)
            .await
            .unwrap();
        assert_eq!(loaded_yaml, config);
    }
}
