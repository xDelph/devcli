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
///    - For Local: cascade-loads env layers and injects into process environment
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
            // Local environment: cascade-load env layers (or strict config map file)
            if let Some(loaded) = crate::env_flow_support::load_local_runtime(
                working_dir,
                resolved_app.app.env_files.as_ref(),
                stage,
            )? {
                tracing::debug!(
                    env_summary = %loaded.summary,
                    environment = "local",
                    var_count = loaded.vars.len(),
                    "Loaded env vars"
                );

                for (key, value) in &loaded.vars {
                    env_vars.insert(key.clone(), value.clone());
                }

                if show_output {
                    println!("  Using env file: {}", loaded.summary);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::resolver::ResolvedApp;
    use crate::test_utils::{AppBuilder, PreferencesBuilder};
    use std::fs;
    use tempfile::TempDir;

    fn resolved_app(path: &str) -> ResolvedApp {
        ResolvedApp {
            project: "test".to_string(),
            app_name: "app".to_string(),
            app: AppBuilder::new("nodejs", path)
                .with_local_command("start", "npm start")
                .build(),
        }
    }

    #[test]
    fn local_prepare_cascades_env_layers() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\nDEBUG=false\n").unwrap();
        fs::write(dir.path().join(".env.local"), "DEBUG=true\n").unwrap();

        let prepared = prepare_command(
            "npm start",
            "local",
            &resolved_app(dir.path().to_str().unwrap()),
            None,
            dir.path(),
            &PreferencesBuilder::new().build(),
            false,
        )
        .unwrap();

        assert_eq!(prepared.env_vars.get("PORT"), Some(&"3000".to_string()));
        assert_eq!(prepared.env_vars.get("DEBUG"), Some(&"true".to_string()));
    }

    #[test]
    fn local_prepare_strict_config_map_skips_cascade() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();
        fs::write(dir.path().join(".env.local"), "DEBUG=true\n").unwrap();
        fs::write(dir.path().join("pinned.env"), "PINNED=1\n").unwrap();

        let mut app = AppBuilder::new("nodejs", dir.path().to_str().unwrap())
            .with_local_command("start", "npm start")
            .build();
        app.env_files = Some(HashMap::from([(
            "base".to_string(),
            HashMap::from([("local".to_string(), "pinned.env".to_string())]),
        )]));

        let resolved = ResolvedApp {
            project: "test".to_string(),
            app_name: "app".to_string(),
            app,
        };

        let prepared = prepare_command(
            "npm start",
            "local",
            &resolved,
            None,
            dir.path(),
            &PreferencesBuilder::new().build(),
            false,
        )
        .unwrap();

        assert_eq!(prepared.env_vars.get("PINNED"), Some(&"1".to_string()));
        assert_eq!(prepared.env_vars.get("DEBUG"), None);
    }
}
