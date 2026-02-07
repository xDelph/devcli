// Command utilities for processing and modifying commands before execution

use std::collections::HashMap;

/// Inject --env-file flag into docker commands
///
/// For Docker/OrbStack, we use --env-file flag to let Docker handle the .env file.
/// The flag is placed before the image name to ensure proper parsing.
///
/// Example:
/// - Input: "docker run --name api --rm -p 3000:3000 myimage", env_file_path: ".env"
/// - Output: "docker run --name api --rm -p 3000:3000 --env-file .env myimage"
pub fn inject_docker_env_file(command: &str, env_file_path: &str) -> String {
    // Split command into parts
    let parts: Vec<&str> = command.split_whitespace().collect();

    if parts.is_empty() {
        return command.to_string();
    }

    // Check if this is a docker command
    let is_docker = parts[0] == "docker" || parts[0].ends_with("/docker");

    if !is_docker || parts.len() < 2 {
        return command.to_string();
    }

    // Find the actual subcommand (skip flags like --context)
    let mut subcommand_index = 1;
    while subcommand_index < parts.len() && parts[subcommand_index].starts_with('-') {
        subcommand_index += 1;
        // Skip the value of the flag (e.g., "orbstack" after "--context")
        if subcommand_index < parts.len() && !parts[subcommand_index].starts_with('-') {
            subcommand_index += 1;
        }
    }

    if subcommand_index >= parts.len() {
        return command.to_string();
    }

    let subcommand = parts[subcommand_index];
    let supports_env_file = matches!(subcommand, "run" | "create");

    if !supports_env_file {
        return command.to_string();
    }

    // Check if --env-file is already present
    if command.contains("--env-file") {
        return command.to_string();
    }

    // Find the image name (last non-flag argument after the subcommand)
    // We need to insert --env-file before the image name
    let mut image_index = parts.len() - 1;

    // The image name is typically the last argument that doesn't start with -
    // Start searching from after the subcommand
    for i in (subcommand_index + 1..parts.len()).rev() {
        if !parts[i].starts_with('-') {
            image_index = i;
            break;
        }
    }

    // Build result: everything before image, then --env-file, then image
    let mut result = Vec::new();
    result.extend_from_slice(&parts[0..image_index]);
    result.push("--env-file");
    result.push(env_file_path);
    result.extend_from_slice(&parts[image_index..]);

    result.join(" ")
}

/// Inject dockerfile path into docker build commands
///
/// For docker build commands, inject -f flag with the dockerfile path.
///
/// Example:
/// - Input: "docker build -t myimage .", dockerfile_path: "docker/Dockerfile"
/// - Output: "docker build -f docker/Dockerfile -t myimage ."
pub fn inject_dockerfile_path(command: &str, dockerfile_path: &str) -> String {
    // Split command into parts
    let parts: Vec<&str> = command.split_whitespace().collect();

    if parts.is_empty() {
        return command.to_string();
    }

    // Check if this is a docker command
    let is_docker = parts[0] == "docker" || parts[0].ends_with("/docker");

    if !is_docker || parts.len() < 2 {
        return command.to_string();
    }

    // Find the actual subcommand (skip flags like --context)
    let mut subcommand_index = 1;
    while subcommand_index < parts.len() && parts[subcommand_index].starts_with('-') {
        subcommand_index += 1;
        // Skip the value of the flag (e.g., "orbstack" after "--context")
        if subcommand_index < parts.len() && !parts[subcommand_index].starts_with('-') {
            subcommand_index += 1;
        }
    }

    if subcommand_index >= parts.len() {
        return command.to_string();
    }

    let subcommand = parts[subcommand_index];
    if subcommand != "build" {
        return command.to_string();
    }

    // Check if -f or --file is already present
    if command.contains(" -f ") || command.contains(" --file ") {
        return command.to_string();
    }

    // Inject -f after the subcommand
    let mut result = Vec::new();
    result.extend_from_slice(&parts[0..=subcommand_index]);
    result.push("-f");
    result.push(dockerfile_path);
    result.extend_from_slice(&parts[subcommand_index + 1..]);

    result.join(" ")
}

/// Inject environment variables as shell prefix for OrbStack commands
///
/// For OrbStack, we prefix the command with KEY="VALUE" pairs (shell-style with quotes).
/// Values are quoted to prevent shell parsing errors with special characters.
///
/// Example:
/// - Input: "docker run myimage", env_vars: {"NODE_ENV": "production", "PORT": "3000"}
/// - Output: "NODE_ENV=\"production\" PORT=\"3000\" docker run myimage"
pub fn inject_orbstack_env_vars(command: &str, env_vars: &HashMap<String, String>) -> String {
    if env_vars.is_empty() {
        return command.to_string();
    }

    // Sort keys for consistent output
    let mut keys: Vec<_> = env_vars.keys().collect();
    keys.sort();

    // Build the env var prefix (KEY="VALUE" format with quotes)
    let env_prefix: Vec<String> = keys
        .iter()
        .filter_map(|k| env_vars.get(*k).map(|v| format!("{}=\"{}\"", k, v)))
        .collect();

    format!("{} {}", env_prefix.join(" "), command)
}

/// Inject the --platform flag into docker commands
///
/// This function modifies docker and docker-compose commands to include
/// the --platform flag with the specified platform value.
///
/// Examples:
/// - "docker run myimage" -> "docker run --platform linux/amd64 myimage"
/// - "docker build -t myimage ." -> "docker build --platform linux/amd64 -t myimage ."
/// - "docker-compose up" -> "docker-compose up" (no change, docker-compose doesn't support --platform in the same way)
pub fn inject_docker_platform(command: &str, platform: &str) -> String {
    // Split command into parts
    let parts: Vec<&str> = command.split_whitespace().collect();

    if parts.is_empty() {
        return command.to_string();
    }

    // Check if this is a docker command
    let is_docker = parts[0] == "docker" || parts[0].ends_with("/docker");

    if !is_docker || parts.len() < 2 {
        return command.to_string();
    }

    // Find the actual subcommand (skip flags like --context)
    let mut subcommand_index = 1;
    while subcommand_index < parts.len() && parts[subcommand_index].starts_with('-') {
        subcommand_index += 1;
        // Skip the value of the flag (e.g., "orbstack" after "--context")
        if subcommand_index < parts.len() && !parts[subcommand_index].starts_with('-') {
            subcommand_index += 1;
        }
    }

    if subcommand_index >= parts.len() {
        return command.to_string();
    }

    let subcommand = parts[subcommand_index];
    let supports_platform = matches!(subcommand, "run" | "build" | "create" | "pull");

    if !supports_platform {
        return command.to_string();
    }

    // Check if --platform is already present
    if command.contains("--platform") {
        return command.to_string();
    }

    // Inject --platform after the subcommand
    let mut result = Vec::new();
    result.extend_from_slice(&parts[0..=subcommand_index]);
    result.push("--platform");
    result.push(platform);
    result.extend_from_slice(&parts[subcommand_index + 1..]);

    result.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_docker_env_file() {
        let command = "docker run myimage";
        let result = inject_docker_env_file(command, ".env");
        assert_eq!(result, "docker run --env-file .env myimage");
    }
}
