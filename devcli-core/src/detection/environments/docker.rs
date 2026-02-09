// Docker environment detection
// Detects Docker commands if Dockerfile exists and generates appropriate commands

use crate::detection::dockerfile;
use crate::detection::utils::find_dockerfile;
use crate::Result;
use std::collections::HashMap;
use std::path::Path;

/// Phase 2b: Detect Docker commands if Dockerfile exists
/// Searches for Dockerfile up to 2 levels deep
/// Generates build/run/stop commands based on app type
/// For multi-stage Dockerfiles, generates commands for each stage
///
/// # Arguments
/// * `path` - App directory
/// * `app_type` - The detected app type (affects port mappings)
///
/// # Returns
/// HashMap of Docker commands, or None if no Dockerfile found
pub fn detect_docker_commands(
    path: &Path,
    app_type: &str,
) -> Result<Option<HashMap<String, String>>> {
    // Search for Dockerfile (up to 2 levels deep)
    let dockerfile = find_dockerfile(path)?;

    // No Dockerfile = no Docker commands
    if dockerfile.is_none() {
        return Ok(None);
    }

    let mut commands = HashMap::new();

    // Use directory name as the image name
    let app_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("app");

    let dockerfile_path = dockerfile
        .as_ref()
        .expect("dockerfile should be Some after is_none check");

    // Check if this is a multi-stage build
    let stages = dockerfile::parse_dockerfile(dockerfile_path).unwrap_or_default();

    if !stages.is_empty() {
        // Multi-stage Dockerfile detected!
        // Generate commands for each stage
        for stage in &stages {
            let stage_name = &stage.name;

            // Build command for this specific stage
            // Example: docker build --target test -t myapp:test .
            commands.insert(
                stage_name.clone(),
                format!(
                    "docker build --target {} -t {}:{} .",
                    stage_name, app_name, stage_name
                ),
            );

            // For test stages, also add a run command
            // This allows running tests in the container
            if stage_name.to_lowercase().contains("test") {
                commands.insert(
                    format!("{}-run", stage_name),
                    format!("docker run --rm {}:{}", app_name, stage_name),
                );
            }
        }
    }

    // Always add a general build command (builds the final stage)
    commands.insert(
        "build".to_string(),
        format!("docker build -t {} .", app_name),
    );

    // Add run command with appropriate port mappings based on app type
    match app_type {
        "nodejs" | "nx" => {
            // Node.js apps typically run on port 3000
            commands.insert(
                "run".to_string(),
                format!(
                    "docker run --name {} --rm -p 3000:3000 {}",
                    app_name, app_name
                ),
            );
        }
        "python" => {
            // Python web apps typically run on port 8000
            commands.insert(
                "run".to_string(),
                format!(
                    "docker run --name {} --rm -p 8000:8000 {}",
                    app_name, app_name
                ),
            );
        }
        "redis" => {
            // Redis default port is 6379
            commands.insert(
                "run".to_string(),
                format!(
                    "docker run --name {} --rm -p 6379:6379 {}",
                    app_name, app_name
                ),
            );
        }
        "traefik" => {
            // Traefik uses ports 80 (HTTP) and 443 (HTTPS)
            commands.insert(
                "run".to_string(),
                format!(
                    "docker run --name {} --rm -p 80:80 -p 443:443 {}",
                    app_name, app_name
                ),
            );
        }
        _ => {
            // Unknown app type - no port mapping
            commands.insert(
                "run".to_string(),
                format!("docker run --name {} --rm {}", app_name, app_name),
            );
        }
    }

    // Add stop command
    commands.insert("stop".to_string(), format!("docker stop {}", app_name));

    Ok(Some(commands))
}

/// Suggest a default command for Docker environment
///
/// # Arguments
/// * `commands` - Available Docker commands
///
/// # Returns
/// Suggested command name (usually "run"), or None if no commands
pub fn suggest_docker_default(commands: &Option<HashMap<String, String>>) -> Option<String> {
    if let Some(cmds) = commands {
        // For Docker, "run" is almost always the default
        if cmds.contains_key("run") {
            return Some("run".to_string());
        }
        // Fallback to first available command
        cmds.keys().next().cloned()
    } else {
        None
    }
}
