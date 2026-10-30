// Stop command - Stop running applications
// Allows stopping one or more running applications
//
// Example: devcli stop api
// Example: devcli stop api --project qm
// Example: devcli stop --all
// Flow: Find running process → Send termination signals → Clean up state files

use crate::process_manager_support::{emit_line, state_store, OutputChannel};
use crate::Result;
use process_manager::engine;

// Arguments for the stop command
pub struct StopCommandArgs {
    pub app_name: Option<String>,         // Optional: specific app to stop
    pub project: Option<String>,          // Optional: stop all apps in a project
    pub all: bool,                        // If true, stop all running processes
    pub force: bool,                      // If true, use SIGKILL instead of SIGTERM
    pub dry_run: bool,                    // If true, only report what would be stopped
    pub silent: bool,                     // If true, don't print to terminal (for TUI mode)
    pub output_tx: Option<OutputChannel>, // Optional output stream (for TUI popup)
}

// Main implementation of the stop command
#[tracing::instrument(skip(args), fields(app_name = ?args.app_name, project = ?args.project, all = args.all, force = args.force))]
pub async fn stop_command(args: StopCommandArgs) -> Result<()> {
    refuse_bulk_stop_for_agents(&args, crate::utils::agent::detect())?;
    let store = state_store()?;
    let silent = args.silent;
    store.cleanup_dead()?;

    tracing::info!(
        app_name = ?args.app_name,
        project = ?args.project,
        all = args.all,
        force = args.force,
        "Stop command initiated"
    );

    let processes = store.list()?;
    let running_processes: Vec<_> = processes
        .into_iter()
        .filter(|p| store.is_running(p))
        .collect();
    let processes_to_stop = filter_processes(&args, running_processes)?;

    if processes_to_stop.is_empty() {
        if crate::output::json_enabled() && !silent {
            crate::output::print_json(&serde_json::json!({
                "dry_run": args.dry_run,
                "stopped": 0,
                "failed": 0,
                "errors": [],
                "message": "No processes to stop",
            }))?;
        } else if args.all {
            emit_line(
                silent,
                args.output_tx.as_ref(),
                "No processes are currently running",
            );
        } else if let Some(app_name) = &args.app_name {
            emit_line(
                silent,
                args.output_tx.as_ref(),
                format!("Process '{}' is not currently running", app_name),
            );
        } else if let Some(project) = &args.project {
            emit_line(
                silent,
                args.output_tx.as_ref(),
                format!(
                    "No processes are currently running for project '{}'",
                    project
                ),
            );
        } else {
            emit_line(
                silent,
                args.output_tx.as_ref(),
                "No processes specified to stop",
            );
        }
        return Ok(());
    }

    if crate::output::human(silent) {
        display_stop_summary(&processes_to_stop)?;
    }

    // Dry run: report the target set and exit without sending signals.
    if args.dry_run {
        if crate::output::json_enabled() {
            let entries: Vec<_> = processes_to_stop
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "app": p.metadata.get("app_config_name").map(String::as_str).unwrap_or(&p.id),
                        "project": p.metadata.get("project"),
                        "pid": p.pid,
                        "command": p.task.command,
                    })
                })
                .collect();
            // Same keys as a real stop (nothing was stopped), plus the target set.
            crate::output::print_json(&serde_json::json!({
                "dry_run": true,
                "stopped": 0,
                "failed": 0,
                "errors": [],
                "count": entries.len(),
                "processes": entries,
            }))?;
        } else if !silent {
            println!("\nDry run: no processes were stopped.");
        }
        return Ok(());
    }

    // Stop each process
    let mut stopped_count = 0;
    let mut errors = Vec::new();

    for process in &processes_to_stop {
        match stop_single_process(process, args.force, &store, silent, args.output_tx.as_ref())
            .await
        {
            Ok(_) => {
                if crate::output::human(silent) {
                    let app_name = process
                        .metadata
                        .get("app_config_name")
                        .map(String::as_str)
                        .unwrap_or(&process.id);
                    println!("✓ Stopped: {} (PID: {})", app_name, process.pid);
                }
                stopped_count += 1;
            }
            Err(e) => {
                let app_name = process
                    .metadata
                    .get("app_config_name")
                    .map(String::as_str)
                    .unwrap_or(&process.id);
                let error_msg = format!("Failed to stop {}: {}", app_name, e);
                errors.push(error_msg.clone());
                if !crate::output::json_enabled() {
                    emit_line(silent, args.output_tx.as_ref(), format!("✗ {}", error_msg));
                }
            }
        }
    }

    if crate::output::human(silent) {
        println!("\nStop Summary:");
        println!("  ✓ Successfully stopped: {}", stopped_count);
        if !errors.is_empty() {
            println!("  ✗ Failed to stop: {}", errors.len());
        }
        if stopped_count > 0 {
            println!("✓ All specified processes have been stopped");
        }
    }

    if crate::output::json_enabled() && !silent {
        crate::output::print_json(&serde_json::json!({
            "dry_run": false,
            "stopped": stopped_count,
            "failed": errors.len(),
            "errors": errors,
        }))?;
    }

    if !errors.is_empty() {
        anyhow::bail!("Some processes failed to stop:\n  {}", errors.join("\n  "));
    }

    Ok(())
}

// Filter processes based on command arguments
fn filter_processes(
    args: &StopCommandArgs,
    processes: Vec<process_manager::state::ManagedProcess>,
) -> Result<Vec<process_manager::state::ManagedProcess>> {
    if args.all {
        return Ok(processes);
    }
    if args.app_name.is_none() && args.project.is_none() {
        return Ok(Vec::new());
    }
    // `stop api` stops the instance of the worktree the command runs in (or
    // of the configured checkout), never another worktree's; `stop api@*`
    // every instance.
    let config = crate::config::load_config()?;
    let selection = crate::worktrees::Selection::of(
        &config,
        args.app_name.as_deref(),
        args.project.as_deref(),
    )?;
    Ok(processes
        .into_iter()
        .filter(|p| selection.matches(&p.metadata))
        .collect())
}

/// An agent stops the apps it works on, by name. `stop --all` and a stop of
/// a whole project would also take down what the user and other agents run,
/// so they are refused when devcli detects it runs under an AI agent.
/// Listing them (`--dry-run`) is fine, and devcli's own callers (the TUI,
/// `restart`) are not concerned.
fn refuse_bulk_stop_for_agents(args: &StopCommandArgs, agent: Option<&str>) -> Result<()> {
    let bulk = args.all || (args.app_name.is_none() && args.project.is_some());
    let (true, false, false, Some(agent)) = (bulk, args.dry_run, args.silent, agent) else {
        return Ok(());
    };
    let what = if args.all {
        "`stop --all`".to_string()
    } else {
        format!(
            "stopping every app of project '{}'",
            args.project.as_deref().unwrap_or_default()
        )
    };
    anyhow::bail!(
        "{what} is not allowed when devcli runs under an AI agent ({agent}): it would also \
         stop apps the user and other agents are running. Stop the apps you started by \
         name: devcli --json stop APP --project PROJECT"
    )
}

// Display summary of processes that will be stopped
fn display_stop_summary(processes: &[process_manager::state::ManagedProcess]) -> Result<()> {
    if processes.is_empty() {
        return Ok(());
    }

    println!("Processes to stop:");

    // Group by project for better display
    let mut by_project: std::collections::HashMap<String, Vec<_>> =
        std::collections::HashMap::new();

    for process in processes {
        let project = process
            .metadata
            .get("project")
            .map(String::as_str)
            .unwrap_or("ungrouped");
        by_project
            .entry(project.to_string())
            .or_insert_with(Vec::new)
            .push(process);
    }

    for (project, procs) in by_project {
        println!("\n  Project: {}", project);
        for process in procs {
            let display_name = process
                .metadata
                .get("app_config_name")
                .map(String::as_str)
                .unwrap_or(&process.id);
            println!(
                "    [{}] PID: {}  {}",
                display_name, process.pid, process.task.command
            );
        }
    }

    println!();
    Ok(())
}

// Stop a single process with proper signal handling
#[tracing::instrument(skip(process, store, output_tx), fields(id = %process.id, pid = process.pid, force = force))]
pub(crate) async fn stop_single_process(
    process: &process_manager::state::ManagedProcess,
    force: bool,
    store: &process_manager::StateStore,
    silent: bool,
    output_tx: Option<&OutputChannel>,
) -> Result<()> {
    // Flag the entry first so pm-daemon does not restart the app while we take
    // it down, and act on the entry as it is now: the daemon may have restarted
    // the app (new PID) since the caller looked it up.
    let Some(current) = store.mark_stopping(&process.id)? else {
        return Ok(());
    };
    let process = &current;

    let display_name = process
        .metadata
        .get("app_config_name")
        .map(String::as_str)
        .unwrap_or(&process.id);

    if !force {
        emit_line(
            silent,
            output_tx,
            format!(
                "Sending SIGTERM to process '{}' (PID: {})...",
                display_name, process.pid
            ),
        );

        let _ = engine::terminate(process.pid, process.pgid, false).await?;

        // Wait for graceful shutdown
        for _ in 0..10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if let Some(reloaded) = store.load(&process.id)? {
                if !store.is_running(&reloaded) {
                    let _ = store.delete(&process.id);
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }
    }

    emit_line(
        silent,
        output_tx,
        format!(
            "Sending SIGKILL to process '{}' (PID: {})...",
            display_name, process.pid
        ),
    );

    let _ = engine::terminate(process.pid, process.pgid, true).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    if let Some(reloaded) = store.load(&process.id)?
        && store.is_running(&reloaded)
    {
        anyhow::bail!(
            "Process '{}' (PID: {}) is still running after SIGKILL",
            display_name,
            process.pid
        );
    }

    store.delete(&process.id)?;
    Ok(())
}

#[cfg(test)]
mod agent_guard_tests {
    use super::*;

    fn args(app: Option<&str>, project: Option<&str>, all: bool, dry_run: bool) -> StopCommandArgs {
        StopCommandArgs {
            app_name: app.map(str::to_string),
            project: project.map(str::to_string),
            all,
            force: false,
            dry_run,
            silent: false,
            output_tx: None,
        }
    }

    #[test]
    fn agents_cannot_stop_everything() {
        let agent = Some("Claude Code");
        let err = refuse_bulk_stop_for_agents(&args(None, None, true, false), agent).unwrap_err();
        assert!(err.to_string().contains("stop --all"), "{err}");
        assert!(
            refuse_bulk_stop_for_agents(&args(None, Some("shop"), false, false), agent).is_err()
        );

        // Named apps, and listing, are fine.
        assert!(
            refuse_bulk_stop_for_agents(&args(Some("api"), Some("shop"), false, false), agent)
                .is_ok()
        );
        assert!(refuse_bulk_stop_for_agents(&args(None, None, true, true), agent).is_ok());
    }

    #[test]
    fn people_and_devcli_itself_still_can() {
        assert!(refuse_bulk_stop_for_agents(&args(None, None, true, false), None).is_ok());
        let mut tui = args(None, Some("shop"), false, false);
        tui.silent = true;
        assert!(refuse_bulk_stop_for_agents(&tui, Some("Claude Code")).is_ok());
    }
}
