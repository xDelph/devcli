//! Git worktree support: run the apps of any worktree of a configured
//! repository without adding anything to `config.json`.
//!
//! See `dev-docs/worktrees-design.md`.

pub mod git;
mod instance;
pub mod registry;
pub mod runtime;
mod selection;

pub use instance::{pinned, resolve, resolve_all, split, Instance, Mode, Selector, PORT_STRIDE};
pub use selection::{base_project, Selection};

/// Process metadata keys describing the worktree an instance runs from.
pub mod meta {
    /// The project in `config.json` (`proj` for `proj@slug`).
    pub const BASE_PROJECT: &str = "base_project";
    pub const WORKTREE: &str = "worktree";
    /// pm-daemon stops the process when this directory is deleted.
    pub const WORKTREE_ROOT: &str = process_manager::state::WORKTREE_ROOT_KEY;
    pub const WORKTREE_BRANCH: &str = "worktree_branch";
}

/// Record in process `metadata` which worktree `resolved` runs from.
pub fn record(
    metadata: &mut std::collections::HashMap<String, String>,
    resolved: &crate::config::resolver::ResolvedApp,
) {
    let Some(instance) = &resolved.worktree else {
        metadata.insert(meta::BASE_PROJECT.to_string(), resolved.project.clone());
        return;
    };
    metadata.insert(
        meta::BASE_PROJECT.to_string(),
        instance.base_project.clone(),
    );
    metadata.insert(meta::WORKTREE.to_string(), instance.slug.clone());
    metadata.insert(
        meta::WORKTREE_ROOT.to_string(),
        instance.root.to_string_lossy().into_owned(),
    );
    if let Some(branch) = &instance.branch {
        metadata.insert(meta::WORKTREE_BRANCH.to_string(), branch.clone());
    }
}

/// How an instance is named to the user: `api`, or `api@slug` in a worktree.
pub fn instance_name(app: &str, worktree: Option<&Instance>) -> String {
    match worktree {
        Some(instance) => format!("{app}@{}", instance.slug),
        None => app.to_string(),
    }
}

/// Where to register an app found at `path`.
#[derive(Debug, PartialEq, Eq)]
pub enum Registration {
    /// Not in a linked worktree: register `path` itself.
    AsIs,
    /// In a linked worktree: register the same directory of the main
    /// checkout; the worktree's copy runs as `APP@<worktree>`.
    MainCheckout {
        path: std::path::PathBuf,
        worktree: String,
    },
    /// Only exists in this linked worktree (an app added on its branch).
    WorktreeOnly { worktree: String },
}

/// Sub-agents run `auto-add` inside their worktree too. Registering that
/// path would point the config at a directory deleted with the worktree,
/// and duplicate an app devcli already runs in every worktree.
pub fn registration_for(path: &std::path::Path) -> Registration {
    let path = git::canonical(path);
    let Some(here) = git::checkout_of(&path) else {
        return Registration::AsIs;
    };
    let Ok(worktrees) = git::list(&here.common_dir) else {
        return Registration::AsIs;
    };
    // The main worktree is listed first; a bare repository has none.
    let Some(main) = worktrees.first().filter(|w| !w.bare && w.root.is_dir()) else {
        return Registration::AsIs;
    };
    if main.root == here.root {
        return Registration::AsIs;
    }
    // The slug devcli names this worktree by (sanitized, maybe suffixed),
    // not git's raw admin name.
    let usable: Vec<&git::Worktree> = worktrees
        .iter()
        .filter(|w| w.is_usable() && w.root != main.root)
        .collect();
    let worktree = registry::Registry::open()
        .peek(&here.common_dir, &usable)
        .ok()
        .and_then(|slots| slots.into_iter().find(|s| s.root == here.root))
        .map(|s| s.slug)
        .unwrap_or_else(|| here.root.display().to_string());
    let relative = path
        .strip_prefix(&here.root)
        .unwrap_or(std::path::Path::new(""));
    let in_main = main.root.join(relative);
    if in_main.is_dir() {
        Registration::MainCheckout {
            path: in_main,
            worktree,
        }
    } else {
        Registration::WorktreeOnly { worktree }
    }
}
