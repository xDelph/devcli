// Status command - Display managed processes grouped by project
//
// Example: devcli status
// Example: devcli status api-private --deps

use crate::config::{dependencies::resolve_dependency_chain, load_config, resolve_app};
use crate::process_manager_support::{find_process, state_store};
use crate::Result;
use chrono::Utc;
use process_manager::state::ManagedProcess;
use std::collections::HashMap;

// Arguments for the status command
pub struct StatusCommandArgs {
    pub app_name: Option<String>, // Optional: filter by specific app
    pub project: Option<String>,  // Optional: filter by project
    pub show_deps: bool,          // If true, show dependency status
}

// Main implementation of the status command
#[tracing::instrument(skip(args), fields(app_name = ?args.app_name, project = ?args.project, show_deps = args.show_deps))]
pub async fn status_command(args: StatusCommandArgs) -> Result<()> {
    let store = state_store()?;
    store.cleanup_dead()?;
    let config = load_config()?;

    // Keep only processes that map to apps in current config.
    let mut processes: Vec<ManagedProcess> = store
        .list()?
        .into_iter()
        .filter(|p| {
            let Some(project_name) = p.metadata.get("project") else {
                return false;
            };
            let Some(app_name) = p.metadata.get("app_config_name") else {
                return false;
            };
            config
                .projects
                .get(project_name)
                .map(|proj| proj.apps.contains_key(app_name))
                .unwrap_or(false)
        })
        .collect();

    if let Some(ref project_filter) = args.project {
        processes.retain(|p| p.metadata.get("project").map(String::as_str) == Some(project_filter));
    }

    if let Some(ref app_filter) = args.app_name {
        processes.retain(|p| {
            p.metadata.get("app_config_name").map(String::as_str) == Some(app_filter)
                || p.id.contains(&format!(".{}.", app_filter))
        });
    }

    if args.show_deps {
        if let Some(app_name) = args.app_name {
            return show_with_dependencies(app_name, &store).await;
        }
    }

    if processes.is_empty() {
        println!("No processes are currently tracked");
        return Ok(());
    }

    let mut grouped: HashMap<String, Vec<ManagedProcess>> = HashMap::new();
    let mut ungrouped = Vec::new();

    for process in processes {
        if let Some(project) = process.metadata.get("project") {
            grouped
                .entry(project.clone())
                .or_default()
                .push(process.clone());
        } else {
            ungrouped.push(process);
        }
    }

    let mut project_names: Vec<_> = grouped.keys().cloned().collect();
    project_names.sort();

    for project_name in project_names {
        println!("\nPROJECT: {}", project_name);

        if let Some(procs) = grouped.get(&project_name) {
            for process in procs {
                let is_running = store.is_running(process);
                let status = if is_running { "running" } else { "stopped" };
                let uptime = format_uptime(is_running, process.start_time);
                let env_display = process
                    .metadata
                    .get("environment")
                    .map(|e| format!(" ({})", e))
                    .unwrap_or_default();
                let display_name = process
                    .metadata
                    .get("app_config_name")
                    .map(String::as_str)
                    .unwrap_or(&process.id);

                println!(
                    "  [{}{}]  PID: {}  {}  {}  {}",
                    display_name, env_display, process.pid, status, uptime, process.task.command
                );
            }
        }
    }

    if !ungrouped.is_empty() {
        println!("\nUNGROUPED:");
        for process in ungrouped {
            let is_running = store.is_running(&process);
            let status = if is_running { "running" } else { "stopped" };
            let uptime = format_uptime(is_running, process.start_time);
            println!(
                "  [{}]  PID: {}  {}  {}  {}",
                process.id, process.pid, status, uptime, process.task.command
            );
        }
    }

    println!();
    Ok(())
}

#[tracing::instrument(skip(store), fields(app_name = %app_name))]
async fn show_with_dependencies(
    app_name: String,
    store: &process_manager::StateStore,
) -> Result<()> {
    let config = load_config()?;
    let resolved = resolve_app(&config, &app_name, None)?;
    let dependencies = resolve_dependency_chain(&config, &resolved)?;

    println!(
        "\nApp: {} (Project: {})",
        resolved.app_name, resolved.project
    );

    if dependencies.is_empty() {
        println!("No dependencies");
    } else {
        println!("\nDependencies:");
        for dep in dependencies {
            let dep_key = format!("{}/{}", dep.project, dep.app_name);
            let is_running = if let Some(process) = find_process(store, &dep.project, &dep.app_name, None)? {
                if store.is_running(&process) {
                    format!("✓ running (PID: {})", process.pid)
                } else {
                    "✗ stopped".to_string()
                }
            } else {
                "✗ not started".to_string()
            };
            println!("  - {} {}", dep_key, is_running);
        }
    }

    if let Some(process) = find_process(store, &resolved.project, &resolved.app_name, None)? {
        let is_running = store.is_running(&process);
        let status = if is_running { "running" } else { "stopped" };
        println!("\nStatus: {}", status);

        if is_running {
            println!("PID: {}", process.pid);
            println!("Uptime: {}", format_uptime(true, process.start_time));
        }
    } else {
        println!("\nStatus: not started");
    }

    println!();
    Ok(())
}

fn format_uptime(is_running: bool, start_time: chrono::DateTime<Utc>) -> String {
    if !is_running {
        return "-".to_string();
    }

    let duration = Utc::now().signed_duration_since(start_time);
    let days = duration.num_days();
    let hours = duration.num_hours() % 24;
    let minutes = duration.num_minutes() % 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}
