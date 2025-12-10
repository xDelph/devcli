//! Configuration initialization functionality
//! 
//! This module handles creating new configuration files with template content.
//! It provides the `devcli config init` command functionality.

use crate::Result;
use std::env;
use std::fs;



/// Initialize a new config file with empty projects
/// 
/// Example: `devcli config init`
///
/// Creates ~/.devcli/config.json with an empty projects structure.
/// Displays an example project configuration in the console for reference.
/// The user can then edit the file to add their own projects and apps.
/// 
/// # Errors
/// 
/// Returns an error if:
/// - Config file already exists (won't overwrite existing configs)
/// - Cannot create the ~/.devcli directory
/// - Cannot write the template file
/// - HOME environment variable is not set
pub async fn config_init() -> Result<()> {
    // Get the home directory from environment
    let home = env::var("HOME")?;
    
    // Build paths: ~/.devcli/ and ~/.devcli/config.json
    let config_dir = std::path::PathBuf::from(home).join(".devcli");
    let config_path = config_dir.join("config.json");
    
    // Check if config already exists
    // Don't overwrite existing configs - user might lose their data!
    if config_path.exists() {
        anyhow::bail!(
            "Config file already exists at {}. Delete it first if you want to recreate it.",
            config_path.display()
        );
    }
    
    // Create the ~/.devcli directory if it doesn't exist
    // create_dir_all is like "mkdir -p" - creates parent directories too
    fs::create_dir_all(&config_dir)?;
    
    // Create empty config with no projects
    let empty_config = r#"{
  "projects": {}
}
"#;
    
    // Example project template to show in console
    let example_template = r#"{
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
    
    // Write the empty config to the config file
    fs::write(&config_path, empty_config)?;
    
    // Show success message with example
    println!("✓ Created empty config file at {}", config_path.display());
    println!("\nHere's an example project configuration you can use as a reference:");
    println!("{}", example_template);
    println!("Edit the config file to add your projects and apps.");
    println!("Then run 'devcli config validate' to check your configuration.");
    
    Ok(())
}
#[cfg(test)]
#[path = "init_test.rs"]
mod tests;