//! `devcli worktrees`: the git worktrees of the configured apps'
//! repositories, their slots (port offsets) and what runs in each.

use crate::config::load_config;
use crate::process_manager_support::state_store;
use crate::utils::path::expand_path;
use crate::worktrees::git::{self, Checkout};
use crate::worktrees::registry::{self, Registry};
use crate::worktrees::PORT_STRIDE;
use crate::Result;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct WorktreesArgs {
    pub project: Option<String>,
}

/// One configured app living in a repository.
struct RepoApp {
    project: String,
    app: String,
    /// Path of the app relative to the configured checkout.
    relative: PathBuf,
}

#[tracing::instrument(skip(args))]
pub async fn worktrees_command(args: WorktreesArgs) -> Result<()> {
    let config = load_config()?;
    if let Some(project) = &args.project
        && !config.projects.contains_key(project)
    {
        anyhow::bail!("Project '{project}' not found in config.");
    }

    // Configured apps, grouped by repository.
    let mut repos: BTreeMap<PathBuf, (Checkout, Vec<RepoApp>)> = BTreeMap::new();
    for (project, p) in &config.projects {
        if args.project.as_ref().is_some_and(|f| f != project) {
            continue;
        }
        for (app, a) in &p.apps {
            let path = git::canonical(&expand_path(&a.path));
            let Some(home) = git::checkout_of(&path) else {
                continue;
            };
            let relative = path.strip_prefix(&home.root).unwrap_or(&path).to_path_buf();
            repos
                .entry(home.common_dir.clone())
                .or_insert_with(|| (home, Vec::new()))
                .1
                .push(RepoApp {
                    project: project.clone(),
                    app: app.clone(),
                    relative,
                });
        }
    }

    let store = state_store()?;
    let processes = store.list()?;
    let running = |project: &str, app: &str| {
        processes.iter().any(|p| {
            p.metadata.get("project").map(String::as_str) == Some(project)
                && p.metadata.get("app_config_name").map(String::as_str) == Some(app)
                && store.is_running(p)
        })
    };
    let here = std::env::current_dir()
        .ok()
        .and_then(|cwd| git::checkout_of(&cwd))
        .map(|c| c.root);

    let mut out = Vec::new();
    for (common_dir, (home, apps)) in &repos {
        let worktrees = git::list(common_dir)?;
        let usable: Vec<_> = worktrees
            .iter()
            .filter(|w| w.is_usable() && w.root != home.root)
            .collect();
        // Listing does not hand out slots: those shown are what `start` gives.
        let assignment =
            Registry::open().peek(common_dir, &usable, &mut registry::running_from())?;
        let slots = &assignment.live;

        let instances = |root: &PathBuf, slug: Option<&str>| -> Vec<Value> {
            apps.iter()
                // A worktree that is gone only lists what still runs there.
                .filter(|a| {
                    root.join(&a.relative).is_dir()
                        || slug.is_some_and(|s| running(&format!("{}@{s}", a.project), &a.app))
                })
                .map(|a| {
                    let (project, name) = match slug {
                        Some(s) => (format!("{}@{s}", a.project), format!("{}@{s}", a.app)),
                        None => (a.project.clone(), a.app.clone()),
                    };
                    json!({
                        "project": project,
                        "app": a.app,
                        "name": name,
                        "running": running(&project, &a.app),
                    })
                })
                .collect()
        };

        let home_worktree = worktrees.iter().find(|w| w.root == home.root);
        let mut entries = vec![json!({
            "slug": Value::Null,
            "index": 0,
            "port_offset": 0,
            "path": home.root,
            "branch": home_worktree.and_then(|w| w.branch.clone()),
            "configured": true,
            "current": here.as_ref() == Some(&home.root),
            "stale": false,
            "locked": home_worktree.is_some_and(|w| w.locked),
            "apps": instances(&home.root, None),
        })];
        for (worktree, slot) in usable.iter().zip(slots) {
            entries.push(json!({
                "slug": slot.slug,
                "index": slot.index,
                "port_offset": slot.index * PORT_STRIDE,
                "path": worktree.root,
                "branch": worktree.branch,
                "configured": false,
                "current": here.as_ref() == Some(&worktree.root),
                "stale": false,
                "locked": worktree.locked,
                "apps": instances(&worktree.root, Some(&slot.slug)),
            }));
        }
        // Deleted (or moved) without `git worktree remove`: git still lists
        // them until `git worktree prune`; devcli never prunes on the user's
        // behalf. One an app still runs from keeps its slot and its name.
        let stale = worktrees
            .iter()
            .filter(|w| !w.bare && w.root != home.root && !w.is_usable())
            .map(|w| {
                (
                    w.root.clone(),
                    w.admin_name.clone(),
                    w.branch.clone(),
                    w.locked,
                )
            });
        let unlisted = assignment
            .held
            .iter()
            .filter(|s| !worktrees.iter().any(|w| w.root == s.root))
            .map(|s| (s.root.clone(), None, None, false));
        for (root, admin_name, branch, locked) in stale.chain(unlisted) {
            let held = assignment.held.iter().find(|s| s.root == root);
            let slug = match (held, &admin_name) {
                (Some(slot), _) => Some(slot.slug.clone()),
                (None, Some(name)) => Some(registry::sanitize_slug(name)),
                (None, None) => None,
            };
            entries.push(json!({
                "slug": slug,
                "index": held.map(|s| s.index),
                "port_offset": held.map(|s| s.index * PORT_STRIDE),
                "path": root,
                "branch": branch,
                "configured": false,
                "current": false,
                "stale": true,
                "locked": locked,
                "apps": match held {
                    Some(slot) => instances(&root, Some(&slot.slug)),
                    None => Vec::new(),
                },
            }));
        }

        out.push(json!({
            "root": home.root,
            "common_dir": common_dir,
            "projects": apps.iter().map(|a| a.project.clone()).collect::<std::collections::BTreeSet<_>>(),
            "worktrees": entries,
        }));
    }

    if crate::output::json_enabled() {
        return crate::output::print_json(&json!({ "repositories": out }));
    }

    if out.is_empty() {
        println!("No configured app lives in a git repository.");
        return Ok(());
    }
    for repo in &out {
        let projects: Vec<&str> = repo["projects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        println!(
            "\n{} ({})",
            repo["root"].as_str().unwrap_or_default(),
            projects.join(", ")
        );
        println!(
            "  {:<20} {:>6}  {:<24} {:<10} PATH",
            "WORKTREE", "PORTS", "BRANCH", "RUNNING"
        );
        let mut stale = false;
        for w in repo["worktrees"].as_array().into_iter().flatten() {
            let name = match w["slug"].as_str() {
                Some(slug) => slug.to_string(),
                None => "(configured)".to_string(),
            };
            let marker = if w["current"] == Value::Bool(true) {
                " *"
            } else {
                ""
            };
            let ports = match w["port_offset"].as_u64() {
                Some(offset) => format!("+{offset}"),
                None => "-".to_string(),
            };
            let running: Vec<&str> = w["apps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|a| a["running"] == Value::Bool(true))
                .filter_map(|a| a["app"].as_str())
                .collect();
            let running = if w["stale"] == Value::Bool(true) {
                stale = true;
                "stale".to_string()
            } else if running.is_empty() {
                "-".to_string()
            } else {
                running.join(",")
            };
            println!(
                "  {:<20} {:>6}  {:<24} {:<10} {}",
                format!("{name}{marker}"),
                ports,
                w["branch"].as_str().unwrap_or("(detached)"),
                running,
                w["path"].as_str().unwrap_or_default()
            );
        }
        if stale {
            println!("  Stale worktrees were deleted without `git worktree remove`; `git worktree prune` forgets them.");
        }
    }
    println!("\n* current directory. Run an app of a worktree with `devcli start APP@WORKTREE`, or from inside it.");
    Ok(())
}
