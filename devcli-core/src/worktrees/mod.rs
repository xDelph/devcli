//! Git worktree support: run the apps of any worktree of a configured
//! repository without adding anything to `config.json`.
//!
//! See `dev-docs/worktrees-design.md`.

pub mod git;
mod instance;
pub mod registry;
mod selection;

pub use instance::{pinned, resolve, resolve_all, split, Instance, Mode, Selector, PORT_STRIDE};
pub use selection::{base_project, Selection};

/// Process metadata keys describing the worktree an instance runs from.
pub mod meta {
    /// The project in `config.json` (`proj` for `proj@slug`).
    pub const BASE_PROJECT: &str = "base_project";
    pub const WORKTREE: &str = "worktree";
    pub const WORKTREE_ROOT: &str = "worktree_root";
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
