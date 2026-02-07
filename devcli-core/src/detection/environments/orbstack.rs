// OrbStack environment detection
// Similar to Docker but uses OrbStack context and parses .env files
// OrbStack doesn't support --env-file, so we parse and pass env vars as -e flags

use crate::detection::dockerfile;
use crate::detection::utils::find_dockerfile;
use crate::Result;
use std::collections::HashMap;

use std::path::Path;

/// Phase 2c: Detect OrbStack commands if Dockerfile exists
/// Similar to Docker detection but uses OrbStack context
/// Environment variables from .env files are applied at runtime, not baked into commands
///
/// # Arguments
/// * `path` - App directory
/// * `app_type` - The detected app type (affects port mappings)
///
/// # Returns
/// HashMap of OrbStack commands, or None if no Dockerfile found
pub fn detect_orbstack_commands(
    path: &Path,
    app_type: &str,
) -> Result<Option<HashMap<String, String>>> {
    // Search for Dockerfile (up to 2 levels deep)
    let dockerfile = find_dockerfile(path)?;

    // No Dockerfile = no OrbStack commands
    if dockerfile.is_none() {
        return Ok(None);
    }

    let mut commands = HashMap::new();

    // Use directory name as the image name
    let app_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("app");

    let dockerfile_path = dockerfile.as_ref()
        .expect("dockerfile should be Some after is_none check");

    // Check if this is a multi-stage build
    let stages = dockerfile::parse_dockerfile(dockerfile_path).unwrap_or_default();

    if !stages.is_empty() {
        // Multi-stage Dockerfile detected!
        // Generate commands for each stage
        for stage in &stages {
            let stage_name = &stage.name;

            // Build command for this specific stage
            commands.insert(
                stage_name.clone(),
                format!(
                    "docker --context orbstack build --target {} -t {}:{} .",
                    stage_name, app_name, stage_name
                ),
            );

            // For test stages, also add a run command
            // Note: env vars will be applied at runtime, not here
            if stage_name.to_lowercase().contains("test") {
                commands.insert(
                    format!("{}-run", stage_name),
                    format!(
                        "docker --context orbstack run --rm {}:{}",
                        app_name, stage_name
                    ),
                );
            }
        }
    }

    // Always add a general build command (builds the final stage)
    commands.insert(
        "build".to_string(),
        format!("docker --context orbstack build -t {} .", app_name),
    );

    // Add run command with appropriate port mappings based on app type
    // Note: env vars from .env files will be applied at runtime via process env_vars
    let run_cmd = match app_type {
        "nodejs" | "nx" => {
            // Node.js apps typically run on port 3000
            format!(
                "docker --context orbstack run --name {} --rm -p 3000:3000 {}",
                app_name, app_name
            )
        }
        "python" => {
            // Python web apps typically run on port 8000
            format!(
                "docker --context orbstack run --name {} --rm -p 8000:8000 {}",
                app_name, app_name
            )
        }
        "redis" => {
            // Redis default port is 6379
            format!(
                "docker --context orbstack run --name {} --rm -p 6379:6379 {}",
                app_name, app_name
            )
        }
        "traefik" => {
            // Traefik uses ports 80 (HTTP) and 443 (HTTPS)
            format!(
                "docker --context orbstack run --name {} --rm -p 80:80 -p 443:443 {}",
                app_name, app_name
            )
        }
        _ => {
            // Unknown app type - no port mapping
            format!(
                "docker --context orbstack run --name {} --rm {}",
                app_name, app_name
            )
        }
    };

    commands.insert("run".to_string(), run_cmd);

    // Add stop command
    commands.insert(
        "stop".to_string(),
        format!("docker --context orbstack stop {}", app_name),
    );

    Ok(Some(commands))
}

/// Suggest a default command for OrbStack environment
///
/// # Arguments
/// * `commands` - Available OrbStack commands
///
/// # Returns
/// Suggested command name (usually "run"), or None if no commands
pub fn suggest_orbstack_default(commands: &Option<HashMap<String, String>>) -> Option<String> {
    if let Some(cmds) = commands {
        // For OrbStack, "run" is almost always the default
        if cmds.contains_key("run") {
            return Some("run".to_string());
        }
        // Fallback to first available command
        cmds.keys().next().cloned()
    } else {
        None
    }
}
