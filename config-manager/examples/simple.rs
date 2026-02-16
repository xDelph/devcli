//! Simple configuration example.
//!
//! Demonstrates basic loading and saving of configuration.

use config_manager::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    name: String,
    version: String,
    port: u16,
}

fn main() -> Result<()> {
    // Create a configuration manager
    let manager = ConfigManager::<AppConfig>::builder()
        .loader(JsonLoader::new("examples/simple_config.json"))
        .build()?;

    // Create a sample config
    let config = AppConfig {
        name: "myapp".to_string(),
        version: "1.0.0".to_string(),
        port: 3000,
    };

    // Save the config
    manager.save(&config)?;
    println!("✓ Saved configuration");

    // Load it back
    let loaded = manager.load()?;
    println!("✓ Loaded configuration:");
    println!("  Name: {}", loaded.name);
    println!("  Version: {}", loaded.version);
    println!("  Port: {}", loaded.port);

    Ok(())
}
