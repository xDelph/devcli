// Configuration file loading and saving
// Handles reading/writing JSON files from ~/.rustycli/

use super::models::{Config, Preferences};
use crate::Result;
use std::env;
use std::fs;
use std::path::PathBuf;

// Get the full path to the config file
// Returns: ~/.rustycli/config.json
pub fn get_config_path() -> Result<PathBuf> {
    // env::var("HOME") gets the HOME environment variable
    // ? operator returns early if there's an error
    let home = env::var("HOME")?;
    
    // Build the path: ~/. rustycli/config.json
    // .join() appends path segments in a platform-independent way
    Ok(PathBuf::from(home).join(".rustycli").join("config.json"))
}

// Get the full path to the preferences file
// Returns: ~/.rustycli/preferences.json
pub fn get_preferences_path() -> Result<PathBuf> {
    let home = env::var("HOME")?;
    Ok(PathBuf::from(home)
        .join(".rustycli")
        .join("preferences.json"))
}

// Load and parse the main config file
// Reads ~/.rustycli/config.json and converts it to a Config struct
pub fn load_config() -> Result<Config> {
    // Get the config file path
    let config_path = get_config_path()?;
    
    // Check if the file exists
    // If not, show a helpful error message telling the user what to do
    if !config_path.exists() {
        anyhow::bail!(
            "Config file not found at {}. Run 'rustycli config init' to create one.",
            config_path.display()
        );
    }
    
    // Read the entire file as a UTF-8 string
    let contents = fs::read_to_string(&config_path)?;
    
    // Parse the JSON string into a Config struct
    // serde_json::from_str automatically maps JSON fields to struct fields
    let config: Config = serde_json::from_str(&contents)?;
    
    Ok(config)
}

// Load user preferences from ~/.rustycli/preferences.json
// If the file doesn't exist, returns default preferences (not an error)
pub fn load_preferences() -> Result<Preferences> {
    let pref_path = get_preferences_path()?;
    
    // If preferences file doesn't exist, that's okay - use defaults
    // This happens when the user hasn't set any preferences yet
    if !pref_path.exists() {
        return Ok(Preferences::default());
    }
    
    // Read and parse the JSON file
    let contents = fs::read_to_string(&pref_path)?;
    let prefs: Preferences = serde_json::from_str(&contents)?;
    
    Ok(prefs)
}

// Save preferences to ~/.rustycli/preferences.json
// Creates the directory if it doesn't exist
pub fn save_preferences(prefs: &Preferences) -> Result<()> {
    let pref_path = get_preferences_path()?;
    
    // Create the ~/.rustycli directory if it doesn't exist
    // .parent() gets the directory containing the file
    if let Some(parent) = pref_path.parent() {
        // create_dir_all is like "mkdir -p" - creates parent directories too
        fs::create_dir_all(parent)?;
    }
    
    // Convert the Preferences struct to pretty-printed JSON
    // pretty = formatted with indentation for readability
    let contents = serde_json::to_string_pretty(prefs)?;
    
    // Write the JSON string to the file
    // This will overwrite the file if it already exists
    fs::write(&pref_path, contents)?;
    
    Ok(())
}

pub fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;
    
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let contents = serde_json::to_string_pretty(config)?;
    
    fs::write(&config_path, contents)?;
    
    Ok(())
}
