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

/// Environment variable overriding the devcli directory.
pub const CONFIG_DIR_ENV: &str = "DEVCLI_CONFIG_DIR";

/// Former, lowercase-prefixed name of [`CONFIG_DIR_ENV`], still honoured.
///
/// Every other devcli variable is uppercase (`DEVCLI_METRICS_ADDR`,
/// `LOG_LEVEL`, ...); users and agents exporting `DEVCLI_CONFIG_DIR` were
/// silently ignored and wrote to their real ~/.devcli.
pub const LEGACY_CONFIG_DIR_ENV: &str = "devcli_CONFIG_DIR";

/// The config-directory override (if set), with `~` expanded to `$HOME`.
///
/// An empty value counts as unset: it used to resolve to the current
/// directory, scattering config, state and logs wherever devcli ran.
fn env_config_dir() -> Option<PathBuf> {
    [CONFIG_DIR_ENV, LEGACY_CONFIG_DIR_ENV]
        .iter()
        .filter_map(|name| env::var(name).ok())
        .find(|dir| !dir.trim().is_empty())
        .map(|dir| expand_tilde(dir.trim()))
}

/// Root directory holding every devcli-owned file (config, preferences,
/// process state, logs).
///
/// Priority: 1) `DEVCLI_CONFIG_DIR` (or legacy `devcli_CONFIG_DIR`), 2) `.devcli` in the current directory when
/// one already exists, 3) `~/.devcli`.
///
/// Everything must derive from here so that a single override isolates the whole
/// tool; the process state and log directories used to be hardcoded to
/// `~/.devcli`, which made the override useless for tests.
pub fn config_home() -> PathBuf {
    if let Some(config_dir) = env_config_dir() {
        return config_dir;
    }

    if let Ok(current_dir) = env::current_dir() {
        let local = current_dir.join(".devcli");
        if local.exists() {
            return local;
        }
    }

    let home = env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".devcli")
}

// Get the full path to the config file
// Returns: ./.devcli/config.json (current dir) or ~/.devcli/config.json (home dir)
// Priority: 1) devcli_CONFIG_DIR env var, 2) current directory, 3) home directory
pub fn get_config_path() -> Result<PathBuf> {
    let path = config_home().join("config.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

// Get the full path to the preferences file
// Priority: 1) devcli_CONFIG_DIR env var, 2) current directory, 3) home directory
pub fn get_preferences_path() -> Result<PathBuf> {
    let path = config_home().join("preferences.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

/// The config to extend when adding apps: empty if there is no config file
/// yet, the parsed config otherwise.
///
/// Never empty because of an *error*: `auto-add` used to fall back to an empty
/// config on any load failure - a trailing comma, a typo - and then saved it,
/// replacing every app the user had with the one being added.
pub fn load_config_or_new() -> Result<Config> {
    if !get_config_path()?.exists() {
        return Ok(Config {
            projects: Default::default(),
        });
    }
    load_config().map_err(|e| {
        anyhow::anyhow!(
            "{e}\nFix the config file (see `devcli config validate`) before adding apps."
        )
    })
}

/// The config, parsed again only when config.json changed on disk.
///
/// For render paths that want it on every frame: the TUI's config panel used
/// to read and parse the whole file (and create the config directory) on each
/// redraw. A `stat` per call decides whether the cached copy is still valid.
pub fn load_config_cached() -> Result<Config> {
    use std::sync::Mutex;
    type Key = (PathBuf, std::time::SystemTime, u64);
    static CACHE: Mutex<Option<(Key, Config)>> = Mutex::new(None);

    let path = config_home().join("config.json");
    let key = match fs::metadata(&path).and_then(|m| Ok((m.modified()?, m.len()))) {
        Ok((modified, len)) => (path, modified, len),
        Err(_) => return load_config(),
    };

    if let Ok(cache) = CACHE.lock() {
        if let Some((cached_key, config)) = cache.as_ref() {
            if *cached_key == key {
                return Ok(config.clone());
            }
        }
    }

    let config = load_config()?;
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some((key, config.clone()));
    }
    Ok(config)
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
    //
    // The shared `env_guard` also keeps this from colliding with the TUI config
    // tests: while this test holds the override *unset*, a concurrent test that
    // calls `save_config` would resolve the real ~/.devcli and overwrite the
    // user's own configuration.
    #[test]
    fn devcli_config_dir_override_is_expanded_and_restored() {
        let _guard = crate::test_utils::env_guard();
        let original = env::var(CONFIG_DIR_ENV).ok();
        let original_legacy = env::var(LEGACY_CONFIG_DIR_ENV).ok();
        env::remove_var(LEGACY_CONFIG_DIR_ENV);

        // ~ expands to $HOME
        env::set_var(CONFIG_DIR_ENV, "~/devcli-cfg");
        let home = env::var("HOME").expect("HOME must be set");
        assert_eq!(
            env_config_dir(),
            Some(PathBuf::from(format!("{home}/devcli-cfg")))
        );

        // plain absolute path passes through untouched
        env::set_var(CONFIG_DIR_ENV, "/tmp/devcli-cfg");
        assert_eq!(env_config_dir(), Some(PathBuf::from("/tmp/devcli-cfg")));

        // the legacy name still works, the uppercase one wins over it
        env::remove_var(CONFIG_DIR_ENV);
        env::set_var(LEGACY_CONFIG_DIR_ENV, "/tmp/legacy-cfg");
        assert_eq!(env_config_dir(), Some(PathBuf::from("/tmp/legacy-cfg")));
        env::set_var(CONFIG_DIR_ENV, "/tmp/devcli-cfg");
        assert_eq!(env_config_dir(), Some(PathBuf::from("/tmp/devcli-cfg")));

        // empty → ignored, not "the current directory"
        env::set_var(CONFIG_DIR_ENV, "  ");
        assert_eq!(env_config_dir(), Some(PathBuf::from("/tmp/legacy-cfg")));
        env::remove_var(LEGACY_CONFIG_DIR_ENV);
        assert_eq!(env_config_dir(), None);

        // unset → None
        env::remove_var(CONFIG_DIR_ENV);
        assert_eq!(env_config_dir(), None);

        for (name, value) in [
            (CONFIG_DIR_ENV, original),
            (LEGACY_CONFIG_DIR_ENV, original_legacy),
        ] {
            match value {
                Some(prev) => env::set_var(name, prev),
                None => env::remove_var(name),
            }
        }
    }
}
