// Preferences management commands
// Handles user preferences stored in ~/.rustycli/preferences.json
//
// Preferences are personal settings that don't belong in the main config
// Example: Your preferred environment (local vs docker)

use crate::config::{load_preferences, save_preferences, Preferences};
use crate::Result;

// Set a preference value
// Example: rustycli pref set default-env docker
pub async fn pref_set(key: String, value: String) -> Result<()> {
    // Load current preferences from file
    // If file doesn't exist, this returns default preferences
    let mut prefs = load_preferences()?;
    
    // Match on the preference key to validate and set the value
    // This prevents setting invalid preference keys
    match key.as_str() {
        "default-env" => {
            // Validate that the value is either "local" or "docker"
            // Reject any other values
            if value != "local" && value != "docker" {
                anyhow::bail!(
                    "Invalid value '{}' for default-env. Must be 'local' or 'docker'.",
                    value
                );
            }
            
            // Value is valid - update the preference
            prefs.default_env = value.clone();
            
            // Save the updated preferences to file
            save_preferences(&prefs)?;
            
            // Confirm to the user
            println!("✓ Set default-env to '{}'", value);
        }
        
        // If someone tries to set a key that doesn't exist
        _ => {
            anyhow::bail!("Unknown preference key '{}'. Valid keys: default-env", key);
        }
    }
    
    Ok(())
}

// Show all current preferences
// Example: rustycli pref show
pub async fn pref_show() -> Result<()> {
    // Load preferences from file
    let prefs = load_preferences()?;
    
    // Display all preference values
    println!("Current preferences:");
    println!("  default-env: {}", prefs.default_env);
    
    // Future preferences would be added here:
    // println!("  some-other-pref: {}", prefs.some_other_pref);
    
    Ok(())
}

// Reset all preferences to default values
// Example: rustycli pref reset
pub async fn pref_reset() -> Result<()> {
    // Create a new Preferences with default values
    // Preferences::default() calls the impl Default we defined in models.rs
    let prefs = Preferences::default();
    
    // Save the defaults to file (overwrites existing preferences)
    save_preferences(&prefs)?;
    
    // Confirm to the user and show what the defaults are
    println!("✓ Preferences reset to defaults");
    println!("  default-env: {}", prefs.default_env);
    
    Ok(())
}
