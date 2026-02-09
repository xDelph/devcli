// Local environment detection
// Extracts commands for running apps locally (npm scripts, nx targets, etc.)

use crate::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Phase 2a: Detect commands for local environment
/// Extracts commands based on app type (npm scripts, nx targets, etc.)
///
/// # Arguments
/// * `path` - App directory
/// * `app_type` - The detected app type
///
/// # Returns
/// HashMap of command name -> command string, or None if no commands found
pub fn detect_local_commands(
    path: &Path,
    app_type: &str,
) -> Result<Option<HashMap<String, String>>> {
    let mut commands = HashMap::new();

    match app_type {
        "nodejs" => {
            if let Ok(content) = fs::read_to_string(path.join("package.json")) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
                        for (key, value) in scripts {
                            if let Some(_cmd) = value.as_str() {
                                commands.insert(key.clone(), format!("npm run {}", key));
                            }
                        }
                    }
                }
            }
        }
        "nx" => {
            if let Ok(content) = fs::read_to_string(path.join("package.json")) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    if let Some(scripts) = json.get("scripts").and_then(|v| v.as_object()) {
                        for (key, _) in scripts {
                            commands.insert(key.clone(), format!("npm run {}", key));
                        }
                    }
                }
            }
        }
        "python" => {
            commands.insert("start".to_string(), "python main.py".to_string());
            if path.join("tests").exists() || path.join("test").exists() {
                commands.insert("test".to_string(), "pytest".to_string());
            }
            if path.join("requirements.txt").exists() {
                commands.insert(
                    "install".to_string(),
                    "pip install -r requirements.txt".to_string(),
                );
            }
        }
        "redis" => {
            let mut config_found = false;

            // Check for config files in current directory
            if path.join("redis.conf").exists() {
                commands.insert("start".to_string(), "redis-server redis.conf".to_string());
                config_found = true;
            }

            // Check for config files in redis/ subdirectory
            if !config_found {
                let redis_dir = path.join("redis");
                if redis_dir.exists() {
                    if redis_dir.join("redis.conf").exists() {
                        commands.insert(
                            "start".to_string(),
                            "redis-server redis/redis.conf".to_string(),
                        );
                        config_found = true;
                    } else if redis_dir.join("redis.config").exists() {
                        commands.insert(
                            "start".to_string(),
                            "redis-server redis/redis.config".to_string(),
                        );
                        config_found = true;
                    }
                }
            }

            // Check for any redis config files in current directory
            if !config_found {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let file_name = entry.file_name();
                        let file_name_str = file_name.to_string_lossy();
                        if file_name_str.starts_with("redis")
                            && (file_name_str.ends_with(".conf")
                                || file_name_str.ends_with(".config"))
                        {
                            commands.insert(
                                "start".to_string(),
                                format!("redis-server {}", file_name_str),
                            );
                            config_found = true;
                            break;
                        }
                    }
                }
            }

            // Fallback to generic redis-server if no config found
            if !config_found {
                commands.insert("start".to_string(), "redis-server".to_string());
            }
        }
        "traefik" => {
            let mut config_found = false;

            // Check for config files in current directory
            for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
                if path.join(file).exists() {
                    commands.insert(
                        "start".to_string(),
                        format!("traefik --configFile={}", file),
                    );
                    config_found = true;
                    break;
                }
            }

            // Check for config files in traefik/ subdirectory
            if !config_found {
                let traefik_dir = path.join("traefik");
                if traefik_dir.exists() {
                    for file in &["traefik.yml", "traefik.yaml", "traefik.toml"] {
                        if traefik_dir.join(file).exists() {
                            commands.insert(
                                "start".to_string(),
                                format!("traefik --configFile=traefik/{}", file),
                            );
                            config_found = true;
                            break;
                        }
                    }
                }
            }

            // Check for any traefik config files in current directory
            if !config_found {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let file_name = entry.file_name();
                        let file_name_str = file_name.to_string_lossy();
                        if file_name_str.starts_with("traefik")
                            && (file_name_str.ends_with(".yml")
                                || file_name_str.ends_with(".yaml")
                                || file_name_str.ends_with(".toml"))
                        {
                            commands.insert(
                                "start".to_string(),
                                format!("traefik --configFile={}", file_name_str),
                            );
                            config_found = true;
                            break;
                        }
                    }
                }
            }

            // Fallback if no config file found
            if !config_found {
                commands.insert("start".to_string(), "traefik".to_string());
            }
        }
        _ => {}
    }

    if commands.is_empty() {
        Ok(None)
    } else {
        Ok(Some(commands))
    }
}

/// Suggest a sensible default command for local environment
/// Prefers common dev server commands based on app type
///
/// # Arguments
/// * `app_type` - The app type
/// * `commands` - Available local commands
///
/// # Returns
/// Suggested command name, or None if no commands
pub fn suggest_local_default(
    app_type: &str,
    commands: &Option<HashMap<String, String>>,
) -> Option<String> {
    if let Some(cmds) = commands {
        // Choose default based on app type and common command names
        match app_type {
            "nodejs" | "nx" => {
                // For Node/Nx apps, prefer dev server commands
                if cmds.contains_key("serve") {
                    return Some("serve".to_string());
                }
                if cmds.contains_key("start") {
                    return Some("start".to_string());
                }
                if cmds.contains_key("dev") {
                    return Some("dev".to_string());
                }
            }
            "python" => {
                // For Python apps, "start" is most common
                if cmds.contains_key("start") {
                    return Some("start".to_string());
                }
            }
            "redis" | "traefik" => {
                // For services, "start" is the standard command
                if cmds.contains_key("start") {
                    return Some("start".to_string());
                }
            }
            _ => {}
        }

        // Fallback: just use the first available command
        cmds.keys().next().cloned()
    } else {
        None
    }
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod local_tests;
