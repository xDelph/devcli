use crate::config::resolver::ResolvedApp;
use crate::config::Preferences;
use crate::Result;
use std::collections::HashMap;
use std::path::Path;

pub struct PreparedCommand {
    pub final_command: String,
    pub env_vars: HashMap<String, String>,
}

/// Prepare command and environment variables based on the environment
///
/// This shared function handles:
/// 1. Injecting Docker/OrbStack specific flags (platform, context)
/// 2. Resolving and applying environment files (.env)
///    - For Docker/OrbStack: uses --env-file flag
///    - For Local: loads vars and injects into process environment
/// 3. Injecting Dockerfile path for build commands
#[tracing::instrument(skip(resolved_app, working_dir, preferences), fields(environment = %environment, stage = ?stage))]
pub fn prepare_command(
    command: &str,
    environment: &str,
    resolved_app: &ResolvedApp,
    stage: Option<&str>,
    working_dir: &Path,
    preferences: &Preferences,
    show_output: bool,
) -> Result<PreparedCommand> {
    tracing::debug!(
        command = %command,
        environment = %environment,
        stage = ?stage,
        "Preparing command"
    );

    // Start with current process environment
    let mut env_vars: HashMap<String, String> = std::env::vars().collect();

    // Inject --platform flag for docker/orbstack commands
    let mut final_command = match environment {
        "docker" | "orbstack" => {
            crate::utils::command::inject_docker_platform(command, &preferences.docker_platform)
        }
        _ => command.to_string(),
    };

    match environment {
        "docker" => {
            env_vars.insert("DOCKER_CONTEXT".to_string(), "default".to_string());

            // For Docker, use --env-file flag if .env exists
            if let Ok(Some(env_file_path)) = crate::detection::resolve_env_file_path(
                working_dir,
                resolved_app.app.env_files.as_ref(),
                stage,
                "docker",
                resolved_app.app.dockerfile_path.as_deref(),
            ) {
                tracing::debug!(
                    env_file = %env_file_path,
                    environment = "docker",
                    "Using env file"
                );

                final_command =
                    crate::utils::command::inject_docker_env_file(&final_command, &env_file_path);

                if show_output {
                    println!("  Using env file: {}", env_file_path);
                }
            }

            // Inject dockerfile path for build commands
            if let Some(ref dockerfile_path) = resolved_app.app.dockerfile_path {
                final_command =
                    crate::utils::command::inject_dockerfile_path(&final_command, dockerfile_path);
            }
        }
        "orbstack" => {
            env_vars.insert("DOCKER_CONTEXT".to_string(), "orbstack".to_string());

            // For OrbStack, use --env-file flag (same as Docker)
            if let Ok(Some(env_file_path)) = crate::detection::resolve_env_file_path(
                working_dir,
                resolved_app.app.env_files.as_ref(),
                stage,
                "orbstack",
                resolved_app.app.dockerfile_path.as_deref(),
            ) {
                tracing::debug!(
                    env_file = %env_file_path,
                    environment = "orbstack",
                    "Using env file"
                );

                final_command =
                    crate::utils::command::inject_docker_env_file(&final_command, &env_file_path);

                if show_output {
                    println!("  Using env file: {}", env_file_path);
                }
            }

            // Inject dockerfile path for build commands
            if let Some(ref dockerfile_path) = resolved_app.app.dockerfile_path {
                final_command =
                    crate::utils::command::inject_dockerfile_path(&final_command, dockerfile_path);
            }
        }
        _ => {
            // Local environment: load env vars from .env file and add to process env
            if let Ok(Some(env_file_path)) = crate::detection::resolve_env_file_path(
                working_dir,
                resolved_app.app.env_files.as_ref(),
                stage,
                "local",
                None, // No Dockerfile for local
            ) {
                let full_env_path = working_dir.join(&env_file_path);
                if let Ok(file_vars) = crate::detection::parse_env_file(&full_env_path) {
                    tracing::debug!(
                        env_file = %env_file_path,
                        environment = "local",
                        var_count = file_vars.len(),
                        "Loaded env vars from file"
                    );

                    // Add variables to the process environment
                    for (key, value) in &file_vars {
                        // Overwrite existing env vars (inherited from shell)
                        env_vars.insert(key.clone(), value.clone());
                    }

                    if show_output {
                        println!("  Using env file: {}", env_file_path);
                        // println!("  Loaded env vars:");
                        // let mut sorted_keys: Vec<_> = file_vars.keys().collect();
                        // sorted_keys.sort();
                        // for key in sorted_keys {
                        //     if let Some(val) = file_vars.get(key) {
                        //         println!("    {}={}", key, val);
                        //     }
                        // }
                    }
                }
            }
        }
    }

    tracing::debug!(
        final_command = %final_command,
        env_var_count = env_vars.len(),
        "Command preparation complete"
    );

    Ok(PreparedCommand {
        final_command,
        env_vars,
    })
}
