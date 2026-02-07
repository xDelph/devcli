// Environment file detection and management
// Discovers all .env files and maps them to stages and contexts

use crate::Result;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Represents a detected environment file with its metadata
#[derive(Debug, Clone)]
pub struct DetectedEnvFile {
    /// Relative path from app root (e.g., ".env.dev", "docker/.env.qa")
    pub path: String,
    /// Detected stage (dev, qa, preprod, prod, or custom)
    pub stage: Option<String>,
    /// Detected context (local, docker, all)
    pub context: String,
}

/// Detect all environment files in an app directory
/// Searches for patterns: .env, .env.*, .*.env
/// Maps files to stages and contexts based on naming conventions
///
/// # Naming Conventions:
/// - `.env` → stage: None, context: "all"
/// - `.env.dev` → stage: "dev", context: "all"
/// - `.env.local` → stage: None, context: "local"
/// - `.env.dev.local` → stage: "dev", context: "local"
/// - `docker/.env.qa` → stage: "qa", context: "docker"
/// - `.local.env` → stage: None, context: "local"
///
/// # Arguments
/// * `app_path` - Root directory of the app
/// * `dockerfile_path` - Optional relative path to Dockerfile (e.g., "docker/Dockerfile")
///
/// # Returns
/// Vector of detected env files with their metadata
pub fn detect_env_files(
    app_path: &Path,
    dockerfile_path: Option<&str>,
) -> Result<Vec<DetectedEnvFile>> {
    let mut detected_files = Vec::new();

    // Search in app root
    detected_files.extend(search_env_files_in_dir(app_path, app_path)?);

    // Search in Dockerfile directory if it exists
    if let Some(dockerfile_rel) = dockerfile_path {
        let dockerfile_full = app_path.join(dockerfile_rel);
        if let Some(dockerfile_dir) = dockerfile_full.parent() {
            if dockerfile_dir != app_path {
                detected_files.extend(search_env_files_in_dir(dockerfile_dir, app_path)?);
            }
        }
    }

    Ok(detected_files)
}

/// Search for env files in a specific directory
///
/// # Arguments
/// * `search_dir` - Directory to search in
/// * `app_root` - App root for calculating relative paths
///
/// # Returns
/// Vector of detected env files
fn search_env_files_in_dir(search_dir: &Path, app_root: &Path) -> Result<Vec<DetectedEnvFile>> {
    let mut files = Vec::new();

    if !search_dir.exists() || !search_dir.is_dir() {
        return Ok(files);
    }

    for entry in fs::read_dir(search_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only process files (not directories)
        if !path.is_file() {
            continue;
        }

        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        // Check if this is an env file
        if is_env_file(filename) {
            let relative_path = path
                .strip_prefix(app_root)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| filename.to_string());

            let (stage, context) = parse_env_filename(filename, &path, app_root);

            files.push(DetectedEnvFile {
                path: relative_path,
                stage,
                context,
            });
        }
    }

    Ok(files)
}

/// Check if a filename matches env file patterns
/// Patterns: .env, .env.*, .*.env
fn is_env_file(filename: &str) -> bool {
    filename == ".env"
        || filename.starts_with(".env.")
        || (filename.starts_with('.') && filename.ends_with(".env"))
}

/// Parse stage and context from env filename
///
/// # Examples:
/// - `.env` → (None, "unspecified")
/// - `.env.dev` → (Some("dev"), "unspecified")
/// - `.env.local` → (None, "local")
/// - `.env.dev.local` → (Some("dev"), "local")
/// - `docker/.env.qa` → (Some("qa"), "docker")
/// - `.local.env` → (None, "local")
/// - `.dev.env` → (Some("dev"), "unspecified")
///
/// # Arguments
/// * `filename` - Name of the env file
/// * `full_path` - Full path to the file
/// * `app_root` - App root directory
///
/// # Returns
/// Tuple of (stage, context) where context is "unspecified" if not explicitly set
fn parse_env_filename(
    filename: &str,
    full_path: &Path,
    app_root: &Path,
) -> (Option<String>, String) {
    // Determine context from directory
    let dir_context = if let Some(parent) = full_path.parent() {
        if parent == app_root {
            None
        } else {
            parent
                .file_name()
                .and_then(|n| n.to_str())
                .map(|dir_name| dir_name.to_lowercase())
        }
    } else {
        None
    };

    // Known contexts
    let known_contexts = ["local", "docker", "orbstack", "k8s"];

    // Known stages (for future use in smarter detection)
    let _known_stages = [
        "dev",
        "qa",
        "preprod",
        "prod",
        "staging",
        "production",
        "test",
    ];

    // Parse filename
    if filename == ".env" {
        // Base .env file - context will be determined by user prompt
        return (
            None,
            dir_context.unwrap_or_else(|| "unspecified".to_string()),
        );
    }

    // Handle .env.* pattern
    if let Some(suffix) = filename.strip_prefix(".env.") {
        let parts: Vec<&str> = suffix.split('.').collect();

        match parts.len() {
            1 => {
                // .env.X - could be stage or context
                let part = parts[0];
                if known_contexts.contains(&part) {
                    // It's a context
                    (None, part.to_string())
                } else {
                    // Assume it's a stage - context will be determined by user prompt
                    (
                        Some(part.to_string()),
                        dir_context.unwrap_or_else(|| "unspecified".to_string()),
                    )
                }
            }
            2 => {
                // .env.X.Y - first is stage, second is context
                let stage = parts[0];
                let context = parts[1];
                (Some(stage.to_string()), context.to_string())
            }
            _ => {
                // More complex pattern - use first as stage, last as context
                let stage = parts[0];
                let context = parts[parts.len() - 1];
                (Some(stage.to_string()), context.to_string())
            }
        }
    }
    // Handle .*.env pattern
    else if filename.ends_with(".env") {
        if let Some(prefix) = filename
            .strip_prefix('.')
            .and_then(|s| s.strip_suffix(".env"))
        {
            // .X.env - X could be stage or context
            if known_contexts.contains(&prefix) {
                (None, prefix.to_string())
            } else {
                (
                    Some(prefix.to_string()),
                    dir_context.unwrap_or_else(|| "unspecified".to_string()),
                )
            }
        } else {
            (None, "unspecified".to_string())
        }
    } else {
        // Shouldn't reach here, but handle gracefully
        (None, "unspecified".to_string())
    }
}

/// Prompt user to select which environments should use a specific env file
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

        let stage_map = map.get_mut(&stage_key)
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

        let stage_map = map.get_mut(&stage_key)
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

/// Parse a .env file and return key-value pairs
/// Handles basic .env format: KEY=VALUE
/// Skips empty lines and comments (lines starting with #)
/// Empty values are replaced with "XXX" placeholder
pub fn parse_env_file(env_path: &Path) -> Result<HashMap<String, String>> {
    let mut env_vars = HashMap::new();

    if !env_path.exists() {
        return Ok(env_vars);
    }

    let content = fs::read_to_string(env_path)?;

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse KEY=VALUE format
        if let Some(pos) = line.find('=') {
            let key = line[..pos].trim().to_string();
            let value = line[pos + 1..].trim().to_string();

            // Remove quotes if present
            let value = value.trim_matches('"').trim_matches('\'').to_string();

            // Replace empty values with XXX placeholder
            let value = if value.is_empty() {
                "XXX".to_string()
            } else {
                value
            };

            env_vars.insert(key, value);
        }
    }

    Ok(env_vars)
}

/// Find the .env file with priority order, supporting stage-specific files
///
/// Priority order when stage is specified:
/// 1. Stage-specific file at Dockerfile level (e.g., docker/.env.dev)
/// 2. Stage-specific file at root level (e.g., .env.dev)
/// 3. Base .env file at Dockerfile level
/// 4. Base .env file at root level
///
/// Priority order when stage is NOT specified:
/// 1. .env file at the Dockerfile level (using dockerfile_path from config if provided)
/// 2. .env file at the Dockerfile level (by searching for Dockerfile if not in config)
/// 3. .env file at the root level
///
/// # Arguments
/// * `app_path` - The root directory of the app
/// * `dockerfile_path` - Optional relative path to Dockerfile from config (e.g., "docker/Dockerfile")
/// * `stage` - Optional deployment stage (dev, qa, preprod, prod)
///
/// # Returns
/// Path to .env file if found, relative to app_path for Docker compatibility
pub fn find_env_file(
    app_path: &Path,
    dockerfile_path: Option<&str>,
    stage: Option<&str>,
) -> Result<Option<String>> {
    use crate::detection::utils::find_dockerfile;

    // Helper function to check if a file exists and return its relative path
    let check_file = |path: &std::path::PathBuf| -> Option<String> {
        if path.exists() {
            path.strip_prefix(app_path)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        } else {
            None
        }
    };

    // Determine the Dockerfile directory
    let dockerfile_dir = if let Some(dockerfile_rel_path) = dockerfile_path {
        let dockerfile_full_path = app_path.join(dockerfile_rel_path);
        dockerfile_full_path.parent().map(|p| p.to_path_buf())
    } else {
        find_dockerfile(app_path)?.and_then(|df| df.parent().map(|p| p.to_path_buf()))
    };

    // If stage is specified, try stage-specific files first
    if let Some(stage_name) = stage {
        let stage_filename = format!(".env.{}", stage_name);

        // 1. Try stage-specific file at Dockerfile level
        if let Some(ref dir) = dockerfile_dir {
            if let Some(path) = check_file(&dir.join(&stage_filename)) {
                return Ok(Some(path));
            }
        }

        // 2. Try stage-specific file at root level
        if let Some(path) = check_file(&app_path.join(&stage_filename)) {
            return Ok(Some(path));
        }
    }

    // 3. Fall back to base .env at Dockerfile level
    if let Some(ref dir) = dockerfile_dir {
        if let Some(path) = check_file(&dir.join(".env")) {
            return Ok(Some(path));
        }
    }

    // 4. Fall back to base .env at root level
    if let Some(path) = check_file(&app_path.join(".env")) {
        return Ok(Some(path));
    }

    Ok(None)
}

/// Load environment variables from .env files for runtime use
/// Searches for .env files in the app directory and returns them as a HashMap
/// This is used at runtime when executing OrbStack commands
///
/// Priority order when stage is specified:
/// 1. Stage-specific file at Dockerfile level (e.g., docker/.env.dev)
/// 2. Stage-specific file at root level (e.g., .env.dev)
/// 3. Base .env file at Dockerfile level
/// 4. Base .env file at root level
///
/// Priority order when stage is NOT specified:
/// 1. .env file at the Dockerfile level (using dockerfile_path from config if provided)
/// 2. .env file at the Dockerfile level (by searching for Dockerfile if not in config)
/// 3. .env file at the root level
///
/// # Arguments
/// * `app_path` - The root directory of the app
/// * `dockerfile_path` - Optional relative path to Dockerfile from config (e.g., "docker/Dockerfile")
/// * `stage` - Optional deployment stage (dev, qa, preprod, prod)
///
/// # Returns
/// HashMap of environment variables loaded from .env files
pub fn load_env_vars_for_runtime(
    app_path: &Path,
    dockerfile_path: Option<&str>,
    stage: Option<&str>,
) -> Result<HashMap<String, String>> {
    // Use find_env_file to determine which file to load based on priority
    if let Some(env_file_path) = find_env_file(app_path, dockerfile_path, stage)? {
        let full_path = app_path.join(&env_file_path);
        return parse_env_file(&full_path);
    }

    // No env file found, return empty HashMap
    Ok(HashMap::new())
}

/// Resolve env file path using new env_files structure with fallback to legacy
/// This function bridges the new env_files map with the old find_env_file logic
///
/// Priority order (context-first approach):
/// 1. stage + environment (e.g., qa + docker)
/// 2. base + environment (e.g., base + docker)
///
/// Note: If env_files_map is provided, we STRICTLY use it and do NOT fallback to legacy logic.
/// This ensures that if a user configures env files, we don't pick random ones.
///
/// # Arguments
/// * `app_path` - The root directory of the app
/// * `env_files_map` - Optional new env_files structure from config
/// * `stage` - Optional deployment stage
/// * `environment` - Runtime environment (local, docker, orbstack, k8s)
/// * `dockerfile_path` - Optional relative path to Dockerfile (for fallback)
///
/// # Returns
/// Relative path to env file if found
pub fn resolve_env_file_path(
    app_path: &Path,
    env_files_map: Option<&HashMap<String, HashMap<String, String>>>,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Result<Option<String>> {
    // Try new env_files structure first
    if let Some(env_files) = env_files_map {
        // Priority 1: stage + environment (e.g., qa + docker)
        if let Some(stage_name) = stage {
            if let Some(stage_map) = env_files.get(stage_name) {
                if let Some(path) = stage_map.get(environment) {
                    return Ok(Some(path.clone()));
                }
            }
        }

        // Priority 2: base + environment (e.g., base + docker)
        if let Some(base_map) = env_files.get("base") {
            if let Some(path) = base_map.get(environment) {
                return Ok(Some(path.clone()));
            }
        }

        // If map is provided but no match found, return None (strict mode)
        return Ok(None);
    }

    // Fall back to legacy find_env_file logic ONLY if no map provided
    find_env_file(app_path, dockerfile_path, stage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_env_file() {
        assert!(is_env_file(".env"));
        assert!(is_env_file(".env.dev"));
        assert!(is_env_file(".env.local"));
        assert!(is_env_file(".local.env"));
        assert!(is_env_file(".dev.env"));
        assert!(!is_env_file("env"));
        assert!(!is_env_file("config.json"));
        assert!(!is_env_file(".envrc"));
    }

    #[test]
    fn test_parse_env_filename_base() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".env");

        let (stage, context) = parse_env_filename(".env", &path, temp_dir.path());
        assert_eq!(stage, None);
        assert_eq!(context, "unspecified"); // Changed from "all"
    }

    #[test]
    fn test_parse_env_filename_stage() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".env.dev");

        let (stage, context) = parse_env_filename(".env.dev", &path, temp_dir.path());
        assert_eq!(stage, Some("dev".to_string()));
        assert_eq!(context, "unspecified"); // Changed from "all"
    }

    #[test]
    fn test_parse_env_filename_context() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".env.local");

        let (stage, context) = parse_env_filename(".env.local", &path, temp_dir.path());
        assert_eq!(stage, None);
        assert_eq!(context, "local");
    }

    #[test]
    fn test_parse_env_filename_stage_and_context() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".env.dev.local");

        let (stage, context) = parse_env_filename(".env.dev.local", &path, temp_dir.path());
        assert_eq!(stage, Some("dev".to_string()));
        assert_eq!(context, "local");
    }

    #[test]
    fn test_parse_env_filename_in_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let docker_dir = temp_dir.path().join("docker");
        fs::create_dir(&docker_dir).unwrap();
        let path = docker_dir.join(".env.qa");

        let (stage, context) = parse_env_filename(".env.qa", &path, temp_dir.path());
        assert_eq!(stage, Some("qa".to_string()));
        assert_eq!(context, "docker");
    }

    #[test]
    fn test_parse_env_filename_reverse_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join(".local.env");

        let (stage, context) = parse_env_filename(".local.env", &path, temp_dir.path());
        assert_eq!(stage, None);
        assert_eq!(context, "local");
    }

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
