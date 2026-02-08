// Restart command - Restart running applications
// Allows restarting applications in exactly the same state they were running
//
// Example: devcli restart api
// Example: devcli restart api --project qm
// Flow: Find running process → Stop it → Start same process with same config

use crate::config::load_preferences;
use crate::process::ProcessTracker;
use crate::Result;

// Arguments for the restart command
pub struct RestartCommandArgs {
    pub app_name: String,        // Name of the app to restart
    pub project: Option<String>, // Optional: specify project if name is ambiguous
    pub env: Option<String>,     // Optional: "local" or "docker" (overrides existing config)
    pub skip_deps: bool,         // If true, don't check/start dependencies
    pub silent: bool,            // If true, don't print to terminal (for TUI mode)
}

// Main implementation of the restart command
// Restart means: stop existing process → start same process with same config
#[tracing::instrument(skip(args), fields(app_name = %args.app_name, project = ?args.project, env = ?args.env, skip_deps = args.skip_deps))]
pub async fn restart_command(args: RestartCommandArgs) -> Result<()> {
    let silent = args.silent;

    tracing::info!(
        app_name = %args.app_name,
        project = ?args.project,
        env = ?args.env,
        "Restart command initiated"
    );

    // Step 0: Load config and resolve app to get project name
    // We need the project name to find the PID file
    let config = crate::config::load_config()?;
    let resolved_app =
        crate::config::resolve_app(&config, &args.app_name, args.project.as_deref())?;
    let project_name = resolved_app.project.clone();

    // Step 1: Get the process tracker and clean up dead processes
    let tracker = ProcessTracker::new()?;
    tracker.cleanup_dead()?;

    // Step 2: Check if the process is currently running
    let existing_process = if let Some(process) =
        tracker.get_process(&project_name, &args.app_name, args.env.as_deref())?
    {
        // Check if process is still actually running
        if tracker.is_running(process.pid) {
            Some(process)
        } else {
            // Process died but we have PID file - clean it up
            tracker.remove_process(
                &project_name,
                &args.app_name,
                process.environment.as_deref(),
            )?;
            if !silent {
                println!(
                    "Process '{}' was not running (cleaning up stale PID file)",
                    args.app_name
                );
            }
            None
        }
    } else {
        None
    };

    // Step 3: If process is not running, we can't restart it - suggest starting instead
    let Some(process) = existing_process else {
        tracing::warn!(
            app_name = %args.app_name,
            project = %project_name,
            "Cannot restart - process not running"
        );
        anyhow::bail!(
            "Process '{}' is not currently running. Use 'start' command instead.",
            args.app_name
        );
    };

    tracing::debug!(
        app_name = %args.app_name,
        pid = process.pid,
        environment = ?process.environment,
        "Found running process to restart"
    );

    // Step 4: Determine environment and project for restart
    // Use --env flag if provided, otherwise use existing process's environment
    let preferences = load_preferences()?;
    let environment = args.env.clone().unwrap_or_else(|| {
        process
            .environment
            .clone()
            .unwrap_or_else(|| preferences.default_env.clone())
    });

    // Use --project flag if provided, otherwise use existing process's project
    // Note: we already resolved the project above, but we keep this logic for consistency with args
    let project = Some(project_name.clone());

    tracing::info!(
        app_name = %args.app_name,
        environment = %environment,
        "Restarting process"
    );

    if !silent {
        println!("Restarting process '{}'...", args.app_name);
    }

    // Step 5: Stop the existing process using stop_command
    let stop_args = crate::commands::stop::StopCommandArgs {
        app_name: Some(args.app_name.clone()),
        project: project.clone(),
        all: false,
        force: false,
        silent,
    };

    crate::commands::stop::stop_command(stop_args).await?;

    // Step 6: Determine if we should use start or run command
    // If command_variant exists and is not the default, use run command
    // Otherwise, use start command
    // We already have resolved_app from Step 0

    let use_run_command = if let Some(ref variant) = process.command_variant {
        // Check if this variant is the default for this environment
        if let Some(default_variant) = resolved_app.app.defaults.get(&environment) {
            variant != default_variant
        } else {
            // No default set, assume it was run with a specific variant
            true
        }
    } else {
        // No variant stored, use start
        false
    };

    if use_run_command {
        // Use run command with the specific variant
        let command_variant = process
            .command_variant
            .clone()
            .unwrap_or_else(|| "start".to_string());

        let run_args = crate::commands::run::RunCommandArgs {
            app_name: args.app_name.clone(),
            command_variant,
            project,
            env: Some(environment),
            skip_deps: args.skip_deps,
        };

        crate::commands::run::run_command(run_args).await?;
    } else {
        // Use start command (default behavior)
        let start_args = crate::commands::start::StartCommandArgs {
            app_names: vec![args.app_name.clone()],
            project,
            env: Some(environment),
            skip_deps: args.skip_deps,
            silent,
            stage: process.stage, // Use the same stage as before
        };

        crate::commands::start::start_command(start_args).await?;
    }

    tracing::info!(
        app_name = %args.app_name,
        "Process restarted successfully"
    );

    if !silent {
        println!("✓ Process '{}' restarted successfully", args.app_name);
    }

    Ok(())
}
