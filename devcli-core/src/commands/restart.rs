// Restart command - Restart running applications
// Allows restarting applications in exactly the same state they were running
//
// Example: devcli restart api
// Example: devcli restart api --project qm
// Flow: Find running process → Stop it → Start same process with same config

use crate::config::load_preferences;
use crate::process_manager_support::{emit_line, find_process, state_store, OutputChannel};
use crate::Result;

// Arguments for the restart command
pub struct RestartCommandArgs {
    pub app_name: String,                 // Name of the app to restart
    pub project: Option<String>,          // Optional: specify project if name is ambiguous
    pub env: Option<String>, // Optional: "local" or "docker" (overrides existing config)
    pub skip_deps: bool,     // If true, don't check/start dependencies
    pub silent: bool,        // If true, don't print to terminal (for TUI mode)
    pub output_tx: Option<OutputChannel>, // Optional output stream (for TUI popup)
}

// Main implementation of the restart command
#[tracing::instrument(skip(args), fields(app_name = %args.app_name, project = ?args.project, env = ?args.env, skip_deps = args.skip_deps))]
pub async fn restart_command(args: RestartCommandArgs) -> Result<()> {
    let silent = args.silent;
    let store = state_store()?;
    store.cleanup_dead()?;

    // Resolve app to get project and actual app name
    let config = crate::config::load_config()?;
    let resolved_app =
        crate::config::resolve_app(&config, &args.app_name, args.project.as_deref())?;
    let project_name = resolved_app.project.clone();
    let actual_app_name = resolved_app.app_name.clone();

    // Check if process exists and is running
    let existing_process = if let Some(process) =
        find_process(&store, &project_name, &actual_app_name, args.env.as_deref())?
    {
        if store.is_running(&process) {
            Some(process)
        } else {
            store.delete(&process.id)?;
            emit_line(
                silent,
                args.output_tx.as_ref(),
                format!(
                    "Process '{}' was not running (cleaning up stale state file)",
                    args.app_name
                ),
            );
            None
        }
    } else {
        None
    };

    let Some(process) = existing_process else {
        anyhow::bail!(
            "Process '{}' is not currently running. Use 'start' command instead.",
            args.app_name
        );
    };

    // Determine environment and project for restart
    let preferences = load_preferences()?;
    let environment = args.env.clone().unwrap_or_else(|| {
        process
            .metadata
            .get("environment")
            .cloned()
            .unwrap_or_else(|| preferences.default_env.clone())
    });
    let project = Some(project_name.clone());

    emit_line(
        silent,
        args.output_tx.as_ref(),
        format!("Restarting process '{}'...", args.app_name),
    );

    // Stop current process first
    let stop_args = crate::commands::stop::StopCommandArgs {
        app_name: Some(args.app_name.clone()),
        project: project.clone(),
        all: false,
        force: false,
        dry_run: false,
        silent,
        output_tx: args.output_tx.clone(),
    };
    crate::commands::stop::stop_command(stop_args).await?;

    // Decide restart path (run vs start)
    let command_variant = process.metadata.get("command_variant").cloned();
    let use_run_command = if let Some(ref variant) = command_variant {
        if let Some(default_variant) = resolved_app.app.defaults.get(&environment) {
            variant != default_variant
        } else {
            true
        }
    } else {
        false
    };

    if use_run_command {
        let run_args = crate::commands::run::RunCommandArgs {
            app_name: args.app_name.clone(),
            command_variant: command_variant.unwrap_or_else(|| "start".to_string()),
            project,
            env: Some(environment),
            skip_deps: args.skip_deps,
            silent,
            output_tx: args.output_tx.clone(),
        };

        crate::commands::run::run_command(run_args).await?;
    } else {
        let start_args = crate::commands::start::StartCommandArgs {
            app_names: vec![args.app_name.clone()],
            project,
            env: Some(environment),
            skip_deps: args.skip_deps,
            silent,
            stage: process.metadata.get("stage").cloned(),
            detached: false,
            output_tx: args.output_tx.clone(),
        };

        crate::commands::start::start_command(start_args).await?;
    }

    emit_line(
        silent,
        args.output_tx.as_ref(),
        format!("✓ Process '{}' restarted successfully", args.app_name),
    );

    Ok(())
}
