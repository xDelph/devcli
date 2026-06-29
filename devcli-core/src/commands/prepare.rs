use crate::config::resolver::ResolvedApp;
use crate::config::Preferences;
use crate::Result;
use std::collections::HashMap;
use std::path::Path;

pub struct PreparedCommand {
    pub final_command: String,
    pub env_vars: HashMap<String, String>,
}

fn uses_container_env_file(environment: &str) -> bool {
    matches!(environment, "docker" | "orbstack" | "docker-compose")
}

fn apply_loaded_env_vars(
    env_vars: &mut HashMap<String, String>,
    loaded: &crate::env_flow_support::EnvLoad,
    show_output: bool,
) {
    for (key, value) in &loaded.vars {
        env_vars.insert(key.clone(), value.clone());
    }

    if show_output {
        let display = if loaded.layers.is_empty() {
            loaded.summary.clone()
        } else {
            crate::env_flow_support::format_env_display(&loaded.summary, &loaded.layers)
        };
        println!("  Using env file: {}", display);
    }
}

/// Prepare command and environment variables based on the environment
///
/// This shared function handles:
/// 1. Injecting Docker/OrbStack specific flags (platform, context)
/// 2. Resolving and applying environment files (.env)
///    - For Docker/OrbStack/Docker Compose: merged cascade via `--env-file`
///    - For Local/K8s/CI: cascade-load env layers into process environment
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
        "docker" | "orbstack" | "docker-compose" => {
            crate::utils::command::inject_docker_platform(command, &preferences.docker_platform)
        }
        _ => command.to_string(),
    };

    match environment {
        "docker" => {
            env_vars.insert("DOCKER_CONTEXT".to_string(), "default".to_string());
            apply_container_env_file(
                &mut final_command,
                working_dir,
                resolved_app,
                stage,
                "docker",
                show_output,
            )?;

            if let Some(ref dockerfile_path) = resolved_app.app.dockerfile_path {
                final_command =
                    crate::utils::command::inject_dockerfile_path(&final_command, dockerfile_path);
            }
        }
        "orbstack" => {
            env_vars.insert("DOCKER_CONTEXT".to_string(), "orbstack".to_string());
            apply_container_env_file(
                &mut final_command,
                working_dir,
                resolved_app,
                stage,
                "orbstack",
                show_output,
            )?;

            if let Some(ref dockerfile_path) = resolved_app.app.dockerfile_path {
                final_command =
                    crate::utils::command::inject_dockerfile_path(&final_command, dockerfile_path);
            }
        }
        "docker-compose" => {
            apply_container_env_file(
                &mut final_command,
                working_dir,
                resolved_app,
                stage,
                "docker-compose",
                show_output,
            )?;
        }
        "local" | "k8s" | "ci" => {
            if let Some(loaded) = crate::env_flow_support::load_process_runtime(
                working_dir,
                resolved_app.app.env_files.as_ref(),
                stage,
                environment,
                resolved_app.app.dockerfile_path.as_deref(),
            )? {
                tracing::debug!(
                    env_summary = %loaded.summary,
                    environment = %environment,
                    var_count = loaded.vars.len(),
                    "Loaded env vars"
                );
                apply_loaded_env_vars(&mut env_vars, &loaded, show_output);
            }
        }
        _ => {
            if let Some(loaded) = crate::env_flow_support::load_process_runtime(
                working_dir,
                resolved_app.app.env_files.as_ref(),
                stage,
                environment,
                resolved_app.app.dockerfile_path.as_deref(),
            )? {
                apply_loaded_env_vars(&mut env_vars, &loaded, show_output);
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

fn apply_container_env_file(
    final_command: &mut String,
    working_dir: &Path,
    resolved_app: &ResolvedApp,
    stage: Option<&str>,
    environment: &str,
    show_output: bool,
) -> Result<()> {
    if !uses_container_env_file(environment) {
        return Ok(());
    }

    if let Some(prepared) = crate::env_flow_support::prepare_container_env_file(
        working_dir,
        resolved_app.app.env_files.as_ref(),
        stage,
        environment,
        resolved_app.app.dockerfile_path.as_deref(),
    )? {
        let env_file_path = prepared
            .path
            .strip_prefix(working_dir)
            .ok()
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_else(|| prepared.path.to_string_lossy().to_string());

        tracing::debug!(
            env_file = %env_file_path,
            environment = %environment,
            "Using env file"
        );

        *final_command =
            crate::utils::command::inject_docker_env_file(final_command, &env_file_path);

        if show_output {
            println!("  Using env file: {}", prepared.summary);
        }
    }

    Ok(())
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
    fn local_prepare_config_pin_overrides_cascade() {
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

    #[test]
    fn local_prepare_config_map_without_local_entry_still_cascades() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();
        fs::write(dir.path().join(".env.local"), "DEBUG=true\n").unwrap();

        let mut app = AppBuilder::new("nodejs", dir.path().to_str().unwrap())
            .with_local_command("start", "npm start")
            .build();
        app.env_files = Some(HashMap::from([(
            "dev".to_string(),
            HashMap::from([("docker".to_string(), "docker/.env".to_string())]),
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

        assert_eq!(prepared.env_vars.get("DEBUG"), Some(&"true".to_string()));
    }

    #[test]
    fn docker_prepare_injects_merged_env_file() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("docker")).unwrap();
        fs::write(dir.path().join(".env"), "PORT=3000\n").unwrap();
        fs::write(dir.path().join("docker/.env"), "PORT=8080\n").unwrap();

        let app = AppBuilder::new("nodejs", dir.path().to_str().unwrap())
            .with_docker_command("run", "docker run --rm nginx")
            .with_dockerfile_path("docker/Dockerfile")
            .build();

        let resolved = ResolvedApp {
            project: "test".to_string(),
            app_name: "app".to_string(),
            app,
        };

        let prepared = prepare_command(
            "docker run --rm nginx",
            "docker",
            &resolved,
            None,
            dir.path(),
            &PreferencesBuilder::new().build(),
            false,
        )
        .unwrap();

        assert!(prepared.final_command.contains("--env-file"));
    }

    #[test]
    fn docker_prepare_injects_env_file_from_docker_dir() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("docker")).unwrap();
        fs::write(dir.path().join("docker/.env"), "PORT=3000\n").unwrap();

        let app = AppBuilder::new("nodejs", dir.path().to_str().unwrap())
            .with_docker_command("run", "docker run --rm nginx")
            .with_dockerfile_path("docker/Dockerfile")
            .build();

        let resolved = ResolvedApp {
            project: "test".to_string(),
            app_name: "app".to_string(),
            app,
        };

        let prepared = prepare_command(
            "docker run --rm nginx",
            "docker",
            &resolved,
            None,
            dir.path(),
            &PreferencesBuilder::new().build(),
            false,
        )
        .unwrap();

        assert!(prepared.final_command.contains("--env-file"));
        assert!(prepared.final_command.contains("docker/.env"));
    }

    #[test]
    fn orbstack_prepare_injects_env_file() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("orbstack")).unwrap();
        fs::write(dir.path().join("orbstack/.env"), "PORT=8080\n").unwrap();

        let app = AppBuilder::new("nodejs", dir.path().to_str().unwrap())
            .with_orbstack_command("run", "docker run --rm nginx")
            .build();

        let resolved = ResolvedApp {
            project: "test".to_string(),
            app_name: "app".to_string(),
            app,
        };

        let prepared = prepare_command(
            "docker run --rm nginx",
            "orbstack",
            &resolved,
            None,
            dir.path(),
            &PreferencesBuilder::new().build(),
            false,
        )
        .unwrap();

        assert!(prepared.final_command.contains("--env-file"));
        assert!(prepared.final_command.contains("orbstack/.env"));
        assert_eq!(prepared.env_vars.get("DOCKER_CONTEXT"), Some(&"orbstack".to_string()));
    }

    #[test]
    fn k8s_prepare_cascades_env_layers() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "APP=1\n").unwrap();
        fs::create_dir_all(dir.path().join("k8s")).unwrap();
        fs::write(dir.path().join("k8s/.env"), "NS=prod\n").unwrap();

        let mut app = AppBuilder::new("nodejs", dir.path().to_str().unwrap())
            .with_local_command("start", "npm start")
            .build();
        app.commands.k8s = Some(HashMap::from([(
            "apply".to_string(),
            "kubectl apply -f k8s/".to_string(),
        )]));

        let resolved = ResolvedApp {
            project: "test".to_string(),
            app_name: "app".to_string(),
            app,
        };

        let prepared = prepare_command(
            "kubectl apply -f k8s/",
            "k8s",
            &resolved,
            None,
            dir.path(),
            &PreferencesBuilder::new().build(),
            false,
        )
        .unwrap();

        assert_eq!(prepared.env_vars.get("APP"), Some(&"1".to_string()));
        assert_eq!(prepared.env_vars.get("NS"), Some(&"prod".to_string()));
    }
}
