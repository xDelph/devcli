use crate::process_manager_support::{process_id, state_store};
use crate::Result;
use anyhow::Context;
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use process_manager::state::{ManagedProcess, ProcessRuntime};
use process_manager::{engine, HealthCheck, RestartPolicy, Task};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct SpawnerPayload {
    pub app_name: String,
    pub alternative_name: Option<String>,
    pub command: String,
    pub working_dir: PathBuf,
    pub env_vars: HashMap<String, String>,
    pub project: Option<String>,
    pub environment: Option<String>,
    pub command_variant: Option<String>,
    pub stage: Option<String>,
    pub log_file_path: PathBuf,
}

#[tracing::instrument(skip(payload_base64), name = "internal_spawner")]
pub async fn internal_spawner_command(payload_base64: String) -> Result<()> {
    let payload_bytes = general_purpose::STANDARD
        .decode(&payload_base64)
        .context("Failed to decode payload")?;
    let payload: SpawnerPayload =
        serde_json::from_slice(&payload_bytes).context("Failed to parse payload JSON")?;

    let project = payload
        .project
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let environment = payload
        .environment
        .clone()
        .unwrap_or_else(|| "local".to_string());
    let id = process_id(&project, &payload.app_name, &environment);

    let store = state_store()?;
    store.cleanup_dead()?;

    if let Some(existing) = store.load(&id)? {
        if store.is_running(&existing) {
            anyhow::bail!(
                "Process '{}:{}' is already running with PID {}",
                project,
                payload.app_name,
                existing.pid
            );
        }
        store.delete(&id)?;
    }

    let task = Task {
        id: id.clone(),
        command: payload.command.clone(),
        args: vec![],
        working_dir: payload.working_dir.clone(),
        env: payload.env_vars.clone(),
        is_detached: true,
        log_file: Some(payload.log_file_path.clone()),
        health_check: HealthCheck::Process {},
        restart_policy: RestartPolicy {
            enabled: false,
            ..RestartPolicy::default()
        },
    };

    let running = engine::spawn(&task).await?;
    if !running.is_alive() {
        anyhow::bail!("Failed to start process '{}'", payload.app_name);
    }

    let process_manager::engine::RunningProcess {
        pid,
        pgid,
        child: _,
        output_rx,
    } = running;
    drop(output_rx);

    let mut metadata = HashMap::new();
    metadata.insert("project".to_string(), project.clone());
    metadata.insert("app_config_name".to_string(), payload.app_name.clone());
    metadata.insert("environment".to_string(), environment.clone());
    if let Some(command_variant) = &payload.command_variant {
        metadata.insert("command_variant".to_string(), command_variant.clone());
    }
    if let Some(stage) = &payload.stage {
        metadata.insert("stage".to_string(), stage.clone());
    }
    if let Some(alt) = &payload.alternative_name {
        metadata.insert("alternative_name".to_string(), alt.clone());
    }

    let managed = ManagedProcess {
        id,
        pid,
        pgid,
        task,
        start_time: Utc::now(),
        metadata,
        runtime: ProcessRuntime::default(),
    };

    store.save(&managed)?;
    store.ensure_daemon_running()?;
    Ok(())
}
