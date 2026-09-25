// Configuration file loading and saving
// Handles reading/writing JSON files from ~/.devcli/
//
// JSON I/O goes through config_manager_support (config-manager crate).
// This module owns path discovery only.

use super::models::{Config, Preferences};
use crate::config_manager_support::{self, DevCliConfigManager};
use crate::Result;
use config_manager::utils::expand_tilde;
use std::env;
use std::fs;
use std::path::PathBuf;

/// The `devcli_CONFIG_DIR` override (if set), with `~` expanded to `$HOME`.
///
/// Legacy behavior used the raw value (a `~` was treated as a literal
/// directory name); this keeps the variable portable.
fn env_config_dir() -> Option<PathBuf> {
    env::var("devcli_CONFIG_DIR")
        .ok()
        .map(|dir| expand_tilde(&dir))
}

// Get the full path to the config file
// Returns: ./.devcli/config.json (current dir) or ~/.devcli/config.json (home dir)
// Priority: 1) devcli_CONFIG_DIR env var, 2) current directory, 3) home directory
pub fn get_config_path() -> Result<PathBuf> {
    if let Some(config_dir) = env_config_dir() {
        let path = config_dir.join("config.json");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        return Ok(path);
    }

    let current_dir = env::current_dir()?;
    let local_config = current_dir.join(".devcli").join("config.json");
    if local_config.exists() {
        return Ok(local_config);
    }

    let home = env::var("HOME")?;
    Ok(PathBuf::from(home).join(".devcli").join("config.json"))
}

// Get the full path to the preferences file
// Priority: 1) devcli_CONFIG_DIR env var, 2) current directory, 3) home directory
pub fn get_preferences_path() -> Result<PathBuf> {
    if let Some(config_dir) = env_config_dir() {
        let path = config_dir.join("preferences.json");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        return Ok(path);
    }

    let current_dir = env::current_dir()?;
    let local_prefs = current_dir.join(".devcli").join("preferences.json");
    if local_prefs.exists() {
        return Ok(local_prefs);
    }

    let home = env::var("HOME")?;
    Ok(PathBuf::from(home).join(".devcli").join("preferences.json"))
}

pub fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;
    DevCliConfigManager::for_path(&config_path)?.load()
}

pub fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;
    DevCliConfigManager::for_path(&config_path)?.save(config)
}

pub fn load_preferences() -> Result<Preferences> {
    let pref_path = get_preferences_path()?;
    config_manager_support::load_preferences(&pref_path)
}

pub fn save_preferences(prefs: &Preferences) -> Result<()> {
    let pref_path = get_preferences_path()?;
    config_manager_support::save_preferences(&pref_path, prefs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // Single sequential test on purpose: `devcli_CONFIG_DIR` is process-global,
    // so splitting these scenarios across parallel #[test]s would race each
    // other (env vars are shared between test threads).
    #[test]
    fn devcli_config_dir_override_is_expanded_and_restored() {
        let original = env::var("devcli_CONFIG_DIR").ok();

        // ~ expands to $HOME
        env::set_var("devcli_CONFIG_DIR", "~/devcli-cfg");
        let home = env::var("HOME").expect("HOME must be set");
        assert_eq!(
            env_config_dir(),
            Some(PathBuf::from(format!("{home}/devcli-cfg")))
        );

        // plain absolute path passes through untouched
        env::set_var("devcli_CONFIG_DIR", "/tmp/devcli-cfg");
        assert_eq!(env_config_dir(), Some(PathBuf::from("/tmp/devcli-cfg")));

        // unset → None
        env::remove_var("devcli_CONFIG_DIR");
        assert_eq!(env_config_dir(), None);

        match original {
            Some(prev) => env::set_var("devcli_CONFIG_DIR", prev),
            None => env::remove_var("devcli_CONFIG_DIR"),
        }
    }
}
