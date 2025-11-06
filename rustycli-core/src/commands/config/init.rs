//! Configuration initialization functionality
//! 
//! This module handles creating new configuration files with template content.
//! It provides the `rustycli config init` command functionality.

use crate::Result;
use std::env;
use std::fs;



/// Initialize a new config file with a template
/// 
/// Example: `rustycli config init`
///
/// Creates ~/.rustycli/config.json with an example app configuration.
/// The user can then edit this file to add their own projects and apps.
/// 
/// # Errors
/// 
/// Returns an error if:
/// - Config file already exists (won't overwrite existing configs)
/// - Cannot create the ~/.rustycli directory
/// - Cannot write the template file
/// - HOME environment variable is not set
pub async fn config_init() -> Result<()> {
    // Get the home directory from environment
    let home = env::var("HOME")?;
    
    // Build paths: ~/.rustycli/ and ~/.rustycli/config.json
    let config_dir = std::path::PathBuf::from(home).join(".rustycli");
    let config_path = config_dir.join("config.json");
    
    // Check if config already exists
    // Don't overwrite existing configs - user might lose their data!
    if config_path.exists() {
        anyhow::bail!(
            "Config file already exists at {}. Delete it first if you want to recreate it.",
            config_path.display()
        );
    }
    
    // Create the ~/.rustycli directory if it doesn't exist
    // create_dir_all is like "mkdir -p" - creates parent directories too
    fs::create_dir_all(&config_dir)?;
    
    // Template config with one example app
    // r#"..."# is a raw string literal - backslashes and quotes don't need escaping
    // This makes it easier to embed JSON
    let template = r#"{
  "projects": {
    "example": {
      "apps": {
        "my-app": {
          "type": "nodejs",
          "path": "~/Projects/my-app",
          "commands": {
            "local": {
              "start": "npm start",
              "test": "npm test",
              "build": "npm run build"
            },
            "docker": {
              "build": "docker build -t my-app .",
              "run": "docker run --name my-app --rm my-app"
            }
          },
          "dependencies": [],
          "defaults": {
            "local": "start",
            "docker": "run"
          }
        }
      }
    }
  }
}
"#;
    
    // Write the template to the config file
    fs::write(&config_path, template)?;
    
    // Show success message with next steps
    println!("✓ Created config file at {}", config_path.display());
    println!("\nEdit this file to add your projects and apps.");
    println!("Then run 'rustycli config validate' to check your configuration.");
    
    Ok(())
}
#[cfg(test)]
#[path = "init_test.rs"]
mod tests;