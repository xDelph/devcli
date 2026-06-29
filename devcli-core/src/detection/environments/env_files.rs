// Environment file detection and config-map building (uses env-flow detector).
// Runtime loading lives in `env_flow_support`.

use crate::Result;
use env_flow::detector::{self, DetectedFile};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A detected environment file with metadata for config-map building.
#[derive(Debug, Clone)]
pub struct DetectedEnvFile {
    pub path: String,
    pub stage: Option<String>,
    pub context: String,
}

/// Discover env files under the app root (and dockerfile directory when nested).
pub fn detect_env_files(
    app_path: &Path,
    dockerfile_path: Option<&str>,
) -> Result<Vec<DetectedEnvFile>> {
    let mut seen = HashSet::new();
    let mut detected = Vec::new();

    for file in detector::discover(app_path)? {
        push_detected(&mut detected, &mut seen, file, app_path);
    }

    if let Some(dockerfile_rel) = dockerfile_path {
        let dockerfile_dir = app_path.join(dockerfile_rel).parent().map(|p| p.to_path_buf());
        if let Some(dir) = dockerfile_dir {
            if dir != app_path {
                for file in detector::discover(&dir)? {
                    push_detected(&mut detected, &mut seen, file, app_path);
                }
            }
        }
    }

    Ok(detected)
}

fn push_detected(
    out: &mut Vec<DetectedEnvFile>,
    seen: &mut HashSet<String>,
    file: DetectedFile,
    app_root: &Path,
) {
    let path = file
        .path
        .strip_prefix(app_root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| file.relative_path.clone());

    if !seen.insert(path.clone()) {
        return;
    }

    out.push(to_detected_env_file(path, file));
}

fn to_detected_env_file(path: String, file: DetectedFile) -> DetectedEnvFile {
    let context = if file.is_local {
        "local".to_string()
    } else if let Some(ctx) = file.context {
        ctx
    } else {
        "unspecified".to_string()
    };

    DetectedEnvFile {
        path,
        stage: file.stage,
        context,
    }
}

/// Prompt user to select which environments should use a specific env file.
///
/// # Arguments
/// * `env_file_path` - Path to the env file (e.g., ".env", ".env.dev")
/// * `available_envs` - List of available environments for this app
///
/// # Returns
/// Vector of selected environment names
pub fn prompt_env_file_environments(
    env_file_path: &str,
    available_envs: &[&str],
) -> crate::Result<Vec<String>> {
    use inquire::MultiSelect;

    let prompt_text = format!("Which environments should use '{}'?", env_file_path);

    let selected = MultiSelect::new(&prompt_text, available_envs.to_vec())
        .with_help_message("Use space to select, enter to confirm. Select all that apply.")
        .prompt()?;

    Ok(selected.into_iter().map(|s| s.to_string()).collect())
}

/// Build env_files map with interactive prompts for unspecified contexts
/// This is the interactive version used during auto-add
///
/// # Arguments
/// * `detected_files` - Vector of detected env files
/// * `available_envs` - List of available environments for this app (e.g., ["local", "docker"])
///
/// # Returns
/// HashMap structure with user-selected environments for unspecified files
pub fn build_env_files_map_interactive(
    detected_files: &[DetectedEnvFile],
    available_envs: &[&str],
) -> crate::Result<HashMap<String, HashMap<String, String>>> {
    let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();

    println!("\n🔍 Processing {} env files...", detected_files.len());
    println!("Available environments: {:?}", available_envs);

    for file in detected_files {
        let stage_key = file.stage.clone().unwrap_or_else(|| "base".to_string());

        if !map.contains_key(&stage_key) {
            map.insert(stage_key.clone(), HashMap::new());
        }

        let stage_map = map
            .get_mut(&stage_key)
            .expect("stage_key should exist after insertion");

        println!(
            "\n📄 File: {} | Stage: {} | Context: {}",
            file.path, stage_key, file.context
        );

        // Handle unspecified context - prompt user
        if file.context == "unspecified" {
            println!("  → Prompting for environments...");
            let selected_envs = prompt_env_file_environments(&file.path, available_envs)?;

            for env in selected_envs {
                stage_map.insert(env.clone(), file.path.clone());

                // Docker and OrbStack share files
                if env == "docker" {
                    stage_map.insert("orbstack".to_string(), file.path.clone());
                } else if env == "orbstack" {
                    stage_map.insert("docker".to_string(), file.path.clone());
                }
            }
        } else {
            // Context is specified, use it directly
            println!("  → Auto-assigned to: {}", file.context);
            stage_map.insert(file.context.clone(), file.path.clone());

            // Docker and OrbStack share files
            if file.context == "docker" {
                println!("  → Also assigned to: orbstack");
                stage_map.insert("orbstack".to_string(), file.path.clone());
            } else if file.context == "orbstack" {
                println!("  → Also assigned to: docker");
                stage_map.insert("docker".to_string(), file.path.clone());
            }
        }
    }

    Ok(map)
}

/// Build env_files map structure for config
/// Groups detected files by stage and context
///
/// Special handling:
/// - Files in "docker" context are also set for "orbstack" (they use the same commands)
/// - Files in "orbstack" context are also set for "docker" (they use the same commands)
/// - Files with "unspecified" context are skipped (user should be prompted to assign them)
///
/// # Arguments
/// * `detected_files` - Vector of detected env files
///
/// # Returns
/// HashMap structure: { "dev": { "local": ".env.dev", "docker": "docker/.env.dev", "orbstack": "docker/.env.dev" }, ... }
pub fn build_env_files_map(
    detected_files: &[DetectedEnvFile],
) -> HashMap<String, HashMap<String, String>> {
    let mut map: HashMap<String, HashMap<String, String>> = HashMap::new();

    for file in detected_files {
        // Skip files with unspecified context - they need user input
        if file.context == "unspecified" {
            continue;
        }

        let stage_key = file.stage.clone().unwrap_or_else(|| "base".to_string());

        if !map.contains_key(&stage_key) {
            map.insert(stage_key.clone(), HashMap::new());
        }

        let stage_map = map
            .get_mut(&stage_key)
            .expect("stage_key should exist after insertion");

        // Insert the file for its detected context
        stage_map.insert(file.context.clone(), file.path.clone());

        // Docker and OrbStack use the same Docker commands, so share env files
        if file.context == "docker" {
            stage_map.insert("orbstack".to_string(), file.path.clone());
        } else if file.context == "orbstack" {
            stage_map.insert("docker".to_string(), file.path.clone());
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_env_files() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        // Create various env files
        fs::write(app_path.join(".env"), "BASE=1").unwrap();
        fs::write(app_path.join(".env.dev"), "DEV=1").unwrap();
        fs::write(app_path.join(".env.qa"), "QA=1").unwrap();
        fs::write(app_path.join(".env.local"), "LOCAL=1").unwrap();
        fs::write(app_path.join(".env.dev.local"), "DEV_LOCAL=1").unwrap();

        // Create docker directory with env files
        let docker_dir = app_path.join("docker");
        fs::create_dir(&docker_dir).unwrap();
        fs::write(docker_dir.join(".env"), "DOCKER_BASE=1").unwrap();
        fs::write(docker_dir.join(".env.prod"), "DOCKER_PROD=1").unwrap();

        let detected = detect_env_files(app_path, Some("docker/Dockerfile")).unwrap();

        // Should find all 7 files
        assert_eq!(detected.len(), 7);

        // Verify some specific files
        let base_file = detected.iter().find(|f| f.path == ".env").unwrap();
        assert_eq!(base_file.stage, None);
        assert_eq!(base_file.context, "unspecified"); // Changed from "all"

        let dev_file = detected.iter().find(|f| f.path == ".env.dev").unwrap();
        assert_eq!(dev_file.stage, Some("dev".to_string()));
        assert_eq!(dev_file.context, "unspecified"); // Changed from "all"

        let docker_prod = detected
            .iter()
            .find(|f| f.path == "docker/.env.prod")
            .unwrap();
        assert_eq!(docker_prod.stage, Some("prod".to_string()));
        assert_eq!(docker_prod.context, "docker");
    }

    #[test]
    fn test_build_env_files_map() {
        let files = vec![
            DetectedEnvFile {
                path: ".env".to_string(),
                stage: None,
                context: "unspecified".to_string(), // Will be skipped
            },
            DetectedEnvFile {
                path: ".env.dev".to_string(),
                stage: Some("dev".to_string()),
                context: "unspecified".to_string(), // Will be skipped
            },
            DetectedEnvFile {
                path: ".env.dev.local".to_string(),
                stage: Some("dev".to_string()),
                context: "local".to_string(),
            },
            DetectedEnvFile {
                path: "docker/.env.qa".to_string(),
                stage: Some("qa".to_string()),
                context: "docker".to_string(),
            },
        ];

        let map = build_env_files_map(&files);

        // Should have 2 stages: dev, qa (base and dev "unspecified" files are skipped)
        assert_eq!(map.len(), 2);

        // Check dev - only has local (unspecified was skipped)
        let dev_map = map.get("dev").unwrap();
        assert_eq!(dev_map.get("local"), Some(&".env.dev.local".to_string()));
        assert_eq!(dev_map.len(), 1); // Only local

        // Check qa - docker context should also be set for orbstack
        let qa_map = map.get("qa").unwrap();
        assert_eq!(qa_map.get("docker"), Some(&"docker/.env.qa".to_string()));
        assert_eq!(qa_map.get("orbstack"), Some(&"docker/.env.qa".to_string()));
    }

    #[test]
    fn test_docker_orbstack_sharing() {
        // Test that docker and orbstack contexts share env files
        let files = vec![
            DetectedEnvFile {
                path: "docker/.env".to_string(),
                stage: None,
                context: "docker".to_string(),
            },
            DetectedEnvFile {
                path: "orbstack/.env.dev".to_string(),
                stage: Some("dev".to_string()),
                context: "orbstack".to_string(),
            },
        ];

        let map = build_env_files_map(&files);

        // Base stage: docker/.env should be available for both docker and orbstack
        let base_map = map.get("base").unwrap();
        assert_eq!(base_map.get("docker"), Some(&"docker/.env".to_string()));
        assert_eq!(base_map.get("orbstack"), Some(&"docker/.env".to_string()));

        // Dev stage: orbstack/.env.dev should be available for both docker and orbstack
        let dev_map = map.get("dev").unwrap();
        assert_eq!(
            dev_map.get("orbstack"),
            Some(&"orbstack/.env.dev".to_string())
        );
        assert_eq!(
            dev_map.get("docker"),
            Some(&"orbstack/.env.dev".to_string())
        );
    }
}
