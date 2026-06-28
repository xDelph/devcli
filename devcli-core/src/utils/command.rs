// Command utilities for processing and modifying commands before execution

use std::collections::HashMap;

fn parse_argv(command: &str) -> Vec<String> {
    match shell_words::split(command) {
        Ok(parts) if !parts.is_empty() => parts,
        Ok(_) => vec![command.to_string()],
        Err(_) => vec![command.to_string()],
    }
}

fn join_argv(parts: &[String]) -> String {
    shell_words::join(parts.iter().map(String::as_str))
}

fn is_docker_argv(parts: &[String]) -> bool {
    parts
        .first()
        .map(|p| p == "docker" || p.ends_with("/docker"))
        .unwrap_or(false)
}

/// Index of the docker subcommand (`run`, `build`, …), skipping global flags like `--context`.
fn docker_subcommand_index(parts: &[String]) -> Option<usize> {
    if !is_docker_argv(parts) || parts.len() < 2 {
        return None;
    }

    let mut index = 1;
    while index < parts.len() && parts[index].starts_with('-') {
        index += 1;
        if index < parts.len() && !parts[index].starts_with('-') {
            index += 1;
        }
    }

    if index < parts.len() {
        Some(index)
    } else {
        None
    }
}

fn argv_has_flag(parts: &[String], flag: &str) -> bool {
    parts.iter().any(|p| p == flag)
}

/// Inject --env-file flag into docker commands
///
/// For Docker/OrbStack, we use --env-file flag to let Docker handle the .env file.
/// The flag is placed before the image name to ensure proper parsing.
pub fn inject_docker_env_file(command: &str, env_file_path: &str) -> String {
    let parts = parse_argv(command);

    if !is_docker_argv(&parts) {
        return command.to_string();
    }

    let Some(subcommand_index) = docker_subcommand_index(&parts) else {
        return command.to_string();
    };

    let subcommand = parts[subcommand_index].as_str();
    if !matches!(subcommand, "run" | "create") {
        return command.to_string();
    }

    if argv_has_flag(&parts, "--env-file") {
        return command.to_string();
    }

    let mut image_index = parts.len().saturating_sub(1);
    for i in (subcommand_index + 1..parts.len()).rev() {
        if !parts[i].starts_with('-') {
            image_index = i;
            break;
        }
    }

    let mut result = parts[..image_index].to_vec();
    result.push("--env-file".to_string());
    result.push(env_file_path.to_string());
    result.extend_from_slice(&parts[image_index..]);

    join_argv(&result)
}

/// Inject dockerfile path into docker build commands
pub fn inject_dockerfile_path(command: &str, dockerfile_path: &str) -> String {
    let parts = parse_argv(command);

    if !is_docker_argv(&parts) {
        return command.to_string();
    }

    let Some(subcommand_index) = docker_subcommand_index(&parts) else {
        return command.to_string();
    };

    if parts[subcommand_index] != "build" {
        return command.to_string();
    }

    if argv_has_flag(&parts, "-f") || argv_has_flag(&parts, "--file") {
        return command.to_string();
    }

    let mut result = parts[..=subcommand_index].to_vec();
    result.push("-f".to_string());
    result.push(dockerfile_path.to_string());
    result.extend_from_slice(&parts[subcommand_index + 1..]);

    join_argv(&result)
}

/// Inject environment variables as shell prefix for OrbStack commands
pub fn inject_orbstack_env_vars(command: &str, env_vars: &HashMap<String, String>) -> String {
    if env_vars.is_empty() {
        return command.to_string();
    }

    let mut keys: Vec<_> = env_vars.keys().collect();
    keys.sort();

    let env_prefix: Vec<String> = keys
        .iter()
        .filter_map(|k| env_vars.get(*k).map(|v| format!("{}=\"{}\"", k, v)))
        .collect();

    format!("{} {}", env_prefix.join(" "), command)
}

/// Inject the --platform flag into docker commands
pub fn inject_docker_platform(command: &str, platform: &str) -> String {
    let parts = parse_argv(command);

    if !is_docker_argv(&parts) {
        return command.to_string();
    }

    let Some(subcommand_index) = docker_subcommand_index(&parts) else {
        return command.to_string();
    };

    let subcommand = parts[subcommand_index].as_str();
    if !matches!(subcommand, "run" | "build" | "create" | "pull") {
        return command.to_string();
    }

    if argv_has_flag(&parts, "--platform") {
        return command.to_string();
    }

    let mut result = parts[..=subcommand_index].to_vec();
    result.push("--platform".to_string());
    result.push(platform.to_string());
    result.extend_from_slice(&parts[subcommand_index + 1..]);

    join_argv(&result)
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

    #[test]
    fn test_inject_docker_env_file_preserves_quoted_paths() {
        let command = r#"docker run --rm -v "/data/with spaces":/data myimage"#;
        let result = inject_docker_env_file(command, "/env files/.env");
        let reparsed = parse_argv(&result);
        assert!(reparsed.contains(&"--env-file".to_string()));
        assert!(reparsed.contains(&"/env files/.env".to_string()));
        assert!(reparsed.contains(&"myimage".to_string()));
    }
}
