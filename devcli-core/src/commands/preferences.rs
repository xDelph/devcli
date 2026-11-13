// Preferences management commands
// Handles user preferences stored in ~/.devcli/preferences.json
//
// Preferences are personal settings that don't belong in the main config
// Example: Your preferred environment (local vs docker)

use crate::config::{load_preferences, save_preferences, Preferences};
use crate::Result;

/// The preferences under the keys `pref set` takes, for `--json`.
fn preferences_json(prefs: &Preferences) -> serde_json::Value {
    serde_json::json!({
        "default-env": prefs.default_env,
        "detached-mode": prefs.detached_mode,
        "auto-start-deps": prefs.auto_start_deps,
        "docker-platform": prefs.docker_platform,
        "default-stage": prefs.default_stage,
    })
}

/// Confirm a `pref set`: one JSON document with `--json`, a line otherwise.
fn report_set(key: &str, value: serde_json::Value, message: String) -> Result<()> {
    if crate::output::json_enabled() {
        return crate::output::print_json(&serde_json::json!({ "key": key, "value": value }));
    }
    println!("{message}");
    Ok(())
}

// Set a preference value
// Example: devcli pref set default-env docker
pub async fn pref_set(key: String, value: String) -> Result<()> {
    // Load current preferences from file
    // If file doesn't exist, this returns default preferences
    let mut prefs = load_preferences()?;

    // Match on the preference key to validate and set the value
    // This prevents setting invalid preference keys
    match key.as_str() {
        "default-env" => {
            // Stored under its canonical name: `compose` kept as typed was
            // used as is by every later `start`.
            use crate::config::models::Environment;
            let env = Environment::parse(&value)
                .map_err(|e| anyhow::anyhow!("Invalid value for default-env: {e}"))?;
            let value = env.as_str().to_string();

            prefs.default_env = value.clone();
            save_preferences(&prefs)?;
            report_set(
                &key,
                value.clone().into(),
                format!("✓ Set default-env to '{value}'"),
            )?;
        }

        "detached-mode" => {
            let bool_value = match value.as_str() {
                "true" | "yes" | "1" => true,
                "false" | "no" | "0" => false,
                _ => {
                    anyhow::bail!(
                        "Invalid value '{}' for detached-mode. Must be 'true' or 'false'.",
                        value
                    );
                }
            };

            prefs.detached_mode = bool_value;
            save_preferences(&prefs)?;
            report_set(
                &key,
                bool_value.into(),
                format!("✓ Set detached-mode to '{bool_value}'"),
            )?;
        }

        "auto-start-deps" => {
            let bool_value = match value.as_str() {
                "true" | "yes" | "1" => true,
                "false" | "no" | "0" => false,
                _ => {
                    anyhow::bail!(
                        "Invalid value '{}' for auto-start-deps. Must be 'true' or 'false'.",
                        value
                    );
                }
            };

            prefs.auto_start_deps = bool_value;
            save_preferences(&prefs)?;
            report_set(
                &key,
                bool_value.into(),
                format!("✓ Set auto-start-deps to '{bool_value}'"),
            )?;
        }

        "docker-platform" => {
            // Validate that the value is a valid platform
            // Common values: linux/amd64, linux/arm64, linux/arm/v7, etc.
            if !value.starts_with("linux/")
                && !value.starts_with("darwin/")
                && !value.starts_with("windows/")
            {
                anyhow::bail!(
                    "Invalid value '{}' for docker-platform. Must be in format 'os/arch' (e.g., 'linux/amd64', 'linux/arm64').",
                    value
                );
            }

            prefs.docker_platform = value.clone();
            save_preferences(&prefs)?;
            report_set(
                &key,
                value.clone().into(),
                format!("✓ Set docker-platform to '{value}'"),
            )?;
        }

        "default-stage" => {
            // Validate that the value is a valid stage
            use crate::config::models::Stage;
            if value == "none" {
                prefs.default_stage = None;
                save_preferences(&prefs)?;
                report_set(
                    &key,
                    serde_json::Value::Null,
                    "✓ Removed default-stage".to_string(),
                )?;
            } else if Stage::from_string(&value).is_some() {
                prefs.default_stage = Some(value.clone());
                save_preferences(&prefs)?;
                report_set(
                    &key,
                    value.clone().into(),
                    format!("✓ Set default-stage to '{value}'"),
                )?;
            } else {
                anyhow::bail!(
                    "Invalid value '{}' for default-stage. Must be one of: {}, none",
                    value,
                    Stage::all_names()
                );
            }
        }

        // If someone tries to set a key that doesn't exist
        _ => {
            anyhow::bail!("Unknown preference key '{}'. Valid keys: default-env, detached-mode, auto-start-deps, docker-platform, default-stage", key);
        }
    }

    Ok(())
}

// Show all current preferences
// Example: devcli pref show
pub async fn pref_show() -> Result<()> {
    // Load preferences from file
    let prefs = load_preferences()?;
    if crate::output::json_enabled() {
        return crate::output::print_json(&preferences_json(&prefs));
    }

    // Display all preference values
    println!("Current preferences:");
    println!("  default-env: {}", prefs.default_env);
    println!("  detached-mode: {}", prefs.detached_mode);
    println!("  auto-start-deps: {}", prefs.auto_start_deps);
    println!("  docker-platform: {}", prefs.docker_platform);
    println!(
        "  default-stage: {}",
        prefs.default_stage.as_deref().unwrap_or("(not set)")
    );

    Ok(())
}

// Reset all preferences to default values
// Example: devcli pref reset
pub async fn pref_reset() -> Result<()> {
    // Create a new Preferences with default values
    // Preferences::default() calls the impl Default we defined in models.rs
    let prefs = Preferences::default();

    // Save the defaults to file (overwrites existing preferences)
    save_preferences(&prefs)?;
    if crate::output::json_enabled() {
        return crate::output::print_json(&preferences_json(&prefs));
    }

    // Confirm to the user and show what the defaults are
    println!("✓ Preferences reset to defaults");
    println!("  default-env: {}", prefs.default_env);
    println!("  detached-mode: {}", prefs.detached_mode);
    println!("  auto-start-deps: {}", prefs.auto_start_deps);
    println!("  docker-platform: {}", prefs.docker_platform);
    println!(
        "  default-stage: {}",
        prefs.default_stage.as_deref().unwrap_or("(none)")
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TempConfigDir;

    #[tokio::test]
    async fn default_env_is_stored_under_its_canonical_name() {
        let _dir = TempConfigDir::new();
        pref_set("default-env".into(), "compose".into())
            .await
            .unwrap();
        assert_eq!(load_preferences().unwrap().default_env, "docker-compose");

        let err = pref_set("default-env".into(), "dokcer".into())
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Did you mean 'docker'?"), "{err}");
    }

    #[test]
    fn json_uses_the_keys_pref_set_takes() {
        let value = preferences_json(&Preferences::default());
        assert_eq!(value["default-env"], "local");
        assert_eq!(value["detached-mode"], false);
        assert_eq!(value["auto-start-deps"], true);
        assert_eq!(value["docker-platform"], "linux/amd64");
        assert!(value["default-stage"].is_null());
    }
}
