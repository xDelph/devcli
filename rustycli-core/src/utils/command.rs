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
    
    // Check if the command is one that supports --env-file (run, create)
    let subcommand = parts[1];
    let supports_env_file = matches!(subcommand, "run" | "create");
    
    if !supports_env_file {
        return command.to_string();
    }
    
    // Check if --env-file is already present
    if command.contains("--env-file") {
        return command.to_string();
    }
    
    // Find the image name (last non-flag argument)
    // We need to insert --env-file before the image name
    let mut image_index = parts.len() - 1;
    
    // The image name is typically the last argument that doesn't start with -
    // and isn't a value for a flag
    for i in (2..parts.len()).rev() {
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
    
    // Check if the command is build
    let subcommand = parts[1];
    if subcommand != "build" {
        return command.to_string();
    }
    
    // Check if -f or --file is already present
    if command.contains(" -f ") || command.contains(" --file ") {
        return command.to_string();
    }
    
    // Inject -f after the subcommand
    let mut result = vec![parts[0], parts[1], "-f", dockerfile_path];
    result.extend_from_slice(&parts[2..]);
    
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
        .map(|k| format!("{}=\"{}\"", k, env_vars.get(*k).unwrap()))
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
    
    // Check if the command is one that supports --platform
    // Common commands: run, build, create, pull
    let subcommand = parts[1];
    let supports_platform = matches!(subcommand, "run" | "build" | "create" | "pull");
    
    if !supports_platform {
        return command.to_string();
    }
    
    // Check if --platform is already present
    if command.contains("--platform") {
        return command.to_string();
    }
    
    // Inject --platform after the subcommand
    let mut result = vec![parts[0], parts[1], "--platform", platform];
    result.extend_from_slice(&parts[2..]);
    
    result.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_inject_docker_platform_run() {
        let cmd = "docker run myimage";
        let result = inject_docker_platform(cmd, "linux/amd64");
        assert_eq!(result, "docker run --platform linux/amd64 myimage");
    }
    
    #[test]
    fn test_inject_docker_platform_build() {
        let cmd = "docker build -t myimage .";
        let result = inject_docker_platform(cmd, "linux/arm64");
        assert_eq!(result, "docker build --platform linux/arm64 -t myimage .");
    }
    
    #[test]
    fn test_inject_docker_platform_already_present() {
        let cmd = "docker run --platform linux/amd64 myimage";
        let result = inject_docker_platform(cmd, "linux/arm64");
        assert_eq!(result, cmd); // Should not modify
    }
    
    #[test]
    fn test_inject_docker_platform_non_docker() {
        let cmd = "npm start";
        let result = inject_docker_platform(cmd, "linux/amd64");
        assert_eq!(result, cmd); // Should not modify
    }
    
    #[test]
    fn test_inject_docker_platform_unsupported_subcommand() {
        let cmd = "docker ps";
        let result = inject_docker_platform(cmd, "linux/amd64");
        assert_eq!(result, cmd); // Should not modify
    }
    
    #[test]
    fn test_inject_docker_env_file() {
        let cmd = "docker run myimage";
        let result = inject_docker_env_file(cmd, ".env");
        assert_eq!(result, "docker run --env-file .env myimage");
    }
    
    #[test]
    fn test_inject_docker_env_file_with_flags() {
        let cmd = "docker run --name api --rm -p 3000:3000 myimage";
        let result = inject_docker_env_file(cmd, ".env");
        assert_eq!(result, "docker run --name api --rm -p 3000:3000 --env-file .env myimage");
    }
    
    #[test]
    fn test_inject_dockerfile_path_build() {
        let cmd = "docker build -t myimage .";
        let result = inject_dockerfile_path(cmd, "docker/Dockerfile");
        assert_eq!(result, "docker build -f docker/Dockerfile -t myimage .");
    }
    
    #[test]
    fn test_inject_dockerfile_path_already_present() {
        let cmd = "docker build -f custom/Dockerfile -t myimage .";
        let result = inject_dockerfile_path(cmd, "docker/Dockerfile");
        assert_eq!(result, cmd); // Should not modify
    }
    
    #[test]
    fn test_inject_dockerfile_path_non_build() {
        let cmd = "docker run myimage";
        let result = inject_dockerfile_path(cmd, "docker/Dockerfile");
        assert_eq!(result, cmd); // Should not modify
    }
    
    #[test]
    fn test_inject_docker_env_file_already_present() {
        let cmd = "docker run --env-file .env myimage";
        let result = inject_docker_env_file(cmd, ".env");
        assert_eq!(result, cmd); // Should not modify
    }
    
    #[test]
    fn test_inject_docker_env_file_non_docker() {
        let cmd = "npm start";
        let result = inject_docker_env_file(cmd, ".env");
        assert_eq!(result, cmd); // Should not modify
    }
    
    #[test]
    fn test_inject_orbstack_env_vars() {
        let cmd = "docker run myimage";
        let mut env_vars = HashMap::new();
        env_vars.insert("NODE_ENV".to_string(), "production".to_string());
        env_vars.insert("PORT".to_string(), "3000".to_string());
        
        let result = inject_orbstack_env_vars(cmd, &env_vars);
        // Should have env vars as prefix with quotes (sorted alphabetically)
        assert_eq!(result, "NODE_ENV=\"production\" PORT=\"3000\" docker run myimage");
    }
    
    #[test]
    fn test_inject_orbstack_env_vars_empty() {
        let cmd = "docker run myimage";
        let env_vars = HashMap::new();
        
        let result = inject_orbstack_env_vars(cmd, &env_vars);
        assert_eq!(result, cmd); // Should not modify
    }
    
    #[test]
    fn test_inject_orbstack_env_vars_single() {
        let cmd = "docker run myimage";
        let mut env_vars = HashMap::new();
        env_vars.insert("DEBUG".to_string(), "true".to_string());
        
        let result = inject_orbstack_env_vars(cmd, &env_vars);
        assert_eq!(result, "DEBUG=\"true\" docker run myimage");
    }
}
