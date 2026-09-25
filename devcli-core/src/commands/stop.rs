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
    pub silent: bool,                     // If true, don't print to terminal (for TUI mode)
    pub output_tx: Option<OutputChannel>, // Optional output stream (for TUI popup)
}

// Main implementation of the stop command
#[tracing::instrument(skip(args), fields(app_name = ?args.app_name, project = ?args.project, all = args.all, force = args.force))]
pub async fn stop_command(args: StopCommandArgs) -> Result<()> {
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

    // Resolve alternative_name to actual app name if provided
    let actual_app_name = if let Some(ref app_name) = args.app_name {
        let config = crate::config::load_config()?;
        match crate::config::resolve_app(&config, app_name, args.project.as_deref()) {
            Ok(resolved) => Some(resolved.app_name),
            Err(_) => Some(app_name.clone()),
        }
    } else {
        None
    };

    let processes = store.list()?;
    let running_processes: Vec<_> = processes
        .into_iter()
        .filter(|p| store.is_running(p))
        .collect();
    let processes_to_stop = filter_processes(&args, &actual_app_name, running_processes, &store)?;

    if processes_to_stop.is_empty() {
        if args.all {
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

    if !silent {
        display_stop_summary(&processes_to_stop)?;
    }

    // Stop each process
    let mut stopped_count = 0;
    let mut errors = Vec::new();

    for process in &processes_to_stop {
        match stop_single_process(process, args.force, &store, silent, args.output_tx.as_ref())
            .await
        {
            Ok(_) => {
                if !silent {
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
                emit_line(silent, args.output_tx.as_ref(), format!("✗ {}", error_msg));
            }
        }
    }

    if !silent {
        println!("\nStop Summary:");
        println!("  ✓ Successfully stopped: {}", stopped_count);
        if !errors.is_empty() {
            println!("  ✗ Failed to stop: {}", errors.len());
        }
        if stopped_count > 0 {
            println!("✓ All specified processes have been stopped");
        }
    }

    if !errors.is_empty() {
        anyhow::bail!("Some processes failed to stop:\n  {}", errors.join("\n  "));
    }

    Ok(())
}

// Filter processes based on command arguments
fn filter_processes(
    args: &StopCommandArgs,
    actual_app_name: &Option<String>,
    processes: Vec<process_manager::state::ManagedProcess>,
    _store: &process_manager::StateStore,
) -> Result<Vec<process_manager::state::ManagedProcess>> {
    let mut filtered = Vec::new();

    if args.all {
        filtered.extend(processes);
    } else if let Some(ref app_name) = actual_app_name {
        for process in processes {
            let matches_app = process.metadata.get("app_config_name").map(String::as_str)
                == Some(app_name.as_str())
                || process.id.contains(&format!(".{}.", app_name));

            if !matches_app {
                continue;
            }

            if let Some(ref project_filter) = args.project {
                if process.metadata.get("project").map(String::as_str) == Some(project_filter) {
                    filtered.push(process);
                    break;
                }
            } else {
                filtered.push(process);
                break;
            }
        }
    } else if let Some(ref project) = args.project {
        for process in processes {
            if process.metadata.get("project").map(String::as_str) == Some(project) {
                filtered.push(process);
            }
        }
    }

    Ok(filtered)
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
async fn stop_single_process(
    process: &process_manager::state::ManagedProcess,
    force: bool,
    store: &process_manager::StateStore,
    silent: bool,
    output_tx: Option<&OutputChannel>,
) -> Result<()> {
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

    if let Some(reloaded) = store.load(&process.id)? {
        if store.is_running(&reloaded) {
            anyhow::bail!(
                "Process '{}' (PID: {}) is still running after SIGKILL",
                display_name,
                process.pid
            );
        }
    }

    store.delete(&process.id)?;
    Ok(())
}
