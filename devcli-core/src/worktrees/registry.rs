//! Stable slugs and port slots for worktrees.
//!
//! The set of worktrees comes from git on every command; what git cannot give
//! is a name and a port offset that stay put while worktrees come and go.
//! Numbering worktrees by their position in `git worktree list` would move
//! the ports of running apps whenever another worktree is created or removed,
//! so each worktree keeps the slot it got first, recorded in
//! `<devcli dir>/worktrees.json`. That file is runtime state, like the process
//! state: `config.json` is never touched.
//!
//! Sub-agents create worktrees and start apps in parallel, so every update
//! happens under an exclusive lock.

use super::git::Worktree;
use crate::Result;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The slot of one worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    /// The repository's shared git directory.
    pub common_dir: PathBuf,
    /// The worktree's root.
    pub root: PathBuf,
    /// Its name in devcli: `project@slug`.
    pub slug: String,
    /// 1, 2, ...: the configured checkout is 0. Smallest free number at
    /// allocation, kept until the worktree disappears.
    pub index: u32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Table {
    #[serde(default)]
    worktrees: Vec<Slot>,
}

/// A name usable in a project name, a process id (`project.app.env`) and a log
/// file name: letters, digits, `-` and `_`.
pub fn sanitize_slug(raw: &str) -> String {
    let slug: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "worktree".to_string()
    } else {
        slug.to_string()
    }
}

/// The slots of one repository: those of the worktrees apps can run from,
/// and those kept for processes still running from a worktree that is gone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Assignment {
    /// One per worktree of `live`, in the same order.
    pub live: Vec<Slot>,
    /// Worktrees moved (`mv`) or deleted while an app still runs from them:
    /// still nameable (`api@slug`) and their index is not handed out again.
    pub held: Vec<Slot>,
}

/// Release the slots of worktrees that are gone, then give every worktree of
/// `live` its slot, allocating one for new worktrees.
///
/// A slot is kept while its worktree is listed by the caller, or its
/// directory is still a checkout: not merely because a caller did not list
/// it, since another devcli may have listed the worktrees just before a
/// sub-agent created a new one. A directory that is no longer a checkout, or
/// is gone (with its repository, possibly), releases its slot - unless
/// `held` says a tracked process still runs from it: a worktree moved with
/// `mv` keeps its apps running, on the ports of its slot.
fn allocate(
    table: &mut Table,
    common_dir: &Path,
    live: &[&Worktree],
    held: &mut dyn FnMut(&Path) -> bool,
) -> Assignment {
    table.worktrees.retain(|slot| {
        (slot.common_dir == common_dir && live.iter().any(|w| w.root == slot.root))
            || slot.root.join(".git").exists()
            || held(&slot.root)
    });
    let kept: Vec<Slot> = table
        .worktrees
        .iter()
        .filter(|s| s.common_dir == common_dir && !live.iter().any(|w| w.root == s.root))
        .filter(|s| held(&s.root))
        .cloned()
        .collect();

    let mut slots = Vec::new();
    for worktree in live {
        if let Some(slot) = table
            .worktrees
            .iter()
            .find(|s| s.common_dir == common_dir && s.root == worktree.root)
        {
            slots.push(slot.clone());
            continue;
        }

        let taken = |slug: &str| {
            table
                .worktrees
                .iter()
                .any(|s| s.common_dir == common_dir && s.slug == slug)
        };
        let wanted = wanted_slug(worktree);
        let mut slug = wanted.clone();
        let mut n = 2;
        while taken(&slug) {
            slug = format!("{wanted}-{n}");
            n += 1;
        }

        let mut index = 1;
        while table
            .worktrees
            .iter()
            .any(|s| s.common_dir == common_dir && s.index == index)
        {
            index += 1;
        }

        let slot = Slot {
            common_dir: common_dir.to_path_buf(),
            root: worktree.root.clone(),
            slug,
            index,
        };
        table.worktrees.push(slot.clone());
        slots.push(slot);
    }
    Assignment {
        live: slots,
        held: kept,
    }
}

/// Whether a tracked process still runs from the worktree at `root`: the
/// default `held` of [`Registry::assign`]. The process state is only read
/// when a slot would otherwise be released, and never created.
pub fn running_from() -> impl FnMut(&Path) -> bool {
    let mut roots: Option<Vec<PathBuf>> = None;
    move |root: &Path| {
        let roots = roots.get_or_insert_with(|| {
            let dir = crate::config::loader::config_home().join("processes");
            if !dir.is_dir() {
                return Vec::new();
            }
            let Ok(store) = process_manager::StateStore::new(dir) else {
                return Vec::new();
            };
            store
                .list()
                .unwrap_or_default()
                .into_iter()
                .filter(|p| store.is_running(p))
                .filter_map(|p| {
                    p.metadata
                        .get(super::meta::WORKTREE_ROOT)
                        .map(PathBuf::from)
                })
                .collect()
        });
        roots.iter().any(|r| r == root)
    }
}

/// The name a worktree asks for: git's admin directory name (unique within
/// the repository), or the directory name for the main worktree.
fn wanted_slug(worktree: &Worktree) -> String {
    let raw = worktree.admin_name.clone().unwrap_or_else(|| {
        worktree
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    sanitize_slug(&raw)
}

pub struct Registry {
    path: PathBuf,
}

impl Registry {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("worktrees.json"),
        }
    }

    /// The devcli directory's registry.
    pub fn open() -> Self {
        Self::new(&crate::config::loader::config_home())
    }

    /// The slot of every worktree in `live` (worktrees of the repository at
    /// `common_dir` that apps can run from), allocating slots for new ones.
    ///
    /// A slot is released once its worktree directory is gone, so its slug
    /// and index can be reused. Not merely when it is missing from `live`:
    /// another devcli may have listed the worktrees a moment earlier, before
    /// a sub-agent created the one it is now starting apps in.
    pub fn assign(
        &self,
        common_dir: &Path,
        live: &[&Worktree],
        held: &mut dyn FnMut(&Path) -> bool,
    ) -> Result<Assignment> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.path.with_extension("lock"))?;
        lock.lock_exclusive()?;

        let mut table = self.load()?;
        let before = table.worktrees.clone();
        let slots = allocate(&mut table, common_dir, live, held);
        if table.worktrees != before {
            let json = serde_json::to_vec_pretty(&table)?;
            config_manager::utils::fs::write_atomic(&self.path, &json)?;
        }
        FileExt::unlock(&lock)?;
        Ok(slots)
    }

    /// The slots [`assign`](Self::assign) would give, without taking the lock
    /// or writing anything: for commands that only look (`status`,
    /// `devcli worktrees`), which must work with a read-only devcli directory
    /// and must not hand out slots just by looking.
    pub fn peek(
        &self,
        common_dir: &Path,
        live: &[&Worktree],
        held: &mut dyn FnMut(&Path) -> bool,
    ) -> Result<Assignment> {
        let mut table = self.load()?;
        Ok(allocate(&mut table, common_dir, live, held))
    }

    fn load(&self) -> Result<Table> {
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                // A damaged table only costs the current numbering; refusing
                // to start anything over it would be worse.
                tracing::warn!(path = %self.path.display(), error = %e, "Resetting worktree registry");
                Table::default()
            })),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Table::default()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worktree(root: &Path, admin: Option<&str>) -> Worktree {
        std::fs::create_dir_all(root).unwrap();
        // A linked worktree's `.git` is a file pointing at its admin dir.
        std::fs::write(root.join(".git"), "gitdir: elsewhere\n").unwrap();
        Worktree {
            root: root.to_path_buf(),
            admin_name: admin.map(str::to_string),
            branch: None,
            bare: false,
            prunable: false,
            locked: false,
        }
    }

    #[test]
    fn slugs_are_safe_in_ids_and_file_names() {
        assert_eq!(sanitize_slug("feat"), "feat");
        assert_eq!(sanitize_slug("v1.2@x/y"), "v1-2-x-y");
        assert_eq!(sanitize_slug("..."), "worktree");
    }

    #[test]
    fn slots_are_stable_while_worktrees_come_and_go() {
        let dir = tempfile::TempDir::new().unwrap();
        let w = dir.path();
        let registry = Registry::new(&w.join("devcli"));
        let repo = Path::new("/repo/.git");
        let a = worktree(&w.join("a"), Some("a"));
        let b = worktree(&w.join("b"), Some("b"));

        let first = registry
            .assign(repo, &[&a, &b], &mut |_| false)
            .unwrap()
            .live;
        assert_eq!((first[0].slug.as_str(), first[0].index), ("a", 1));
        assert_eq!((first[1].slug.as_str(), first[1].index), ("b", 2));

        // A new worktree that sorts first does not shift the others.
        let z = worktree(&w.join("0"), Some("z"));
        let second = registry
            .assign(repo, &[&z, &a, &b], &mut |_| false)
            .unwrap()
            .live;
        assert_eq!(second[0].index, 3);
        assert_eq!(second[1], first[0]);
        assert_eq!(second[2], first[1]);

        // A caller that has not seen `z` yet does not take its slot away.
        registry.assign(repo, &[&a, &b], &mut |_| false).unwrap();
        assert_eq!(
            registry.assign(repo, &[&z], &mut |_| false).unwrap().live[0],
            second[0]
        );

        // `a` is deleted: its index is released and reused.
        std::fs::remove_dir_all(&a.root).unwrap();
        let c = worktree(&w.join("c"), Some("c"));
        let third = registry
            .assign(repo, &[&b, &z, &c], &mut |_| false)
            .unwrap()
            .live;
        assert_eq!(third[0], first[1]);
        assert_eq!((third[2].slug.as_str(), third[2].index), ("c", 1));
    }

    #[test]
    fn repositories_are_numbered_separately() {
        let dir = tempfile::TempDir::new().unwrap();
        let registry = Registry::new(&dir.path().join("devcli"));
        let a = worktree(&dir.path().join("one/w"), Some("w"));
        let b = worktree(&dir.path().join("two/w"), Some("w"));
        let one = registry
            .assign(Path::new("/one/.git"), &[&a], &mut |_| false)
            .unwrap()
            .live;
        let two = registry
            .assign(Path::new("/two/.git"), &[&b], &mut |_| false)
            .unwrap()
            .live;
        assert_eq!((one[0].index, two[0].index), (1, 1));
        assert_eq!(one[0].slug, two[0].slug);
    }

    #[test]
    fn colliding_slugs_get_a_suffix() {
        let dir = tempfile::TempDir::new().unwrap();
        let w = dir.path();
        let registry = Registry::new(&w.join("devcli"));
        let repo = Path::new("/repo/.git");
        // The main worktree's directory name can equal a linked worktree's
        // admin name, and sanitizing can merge two names.
        let main = worktree(&w.join("x/feat"), None);
        let dotted = worktree(&w.join("v1.2"), Some("v1.2"));
        let dashed = worktree(&w.join("v1-2"), Some("v1-2"));
        let linked = worktree(&w.join("y/feat"), Some("feat"));
        let slots = registry
            .assign(repo, &[&main, &dotted, &dashed, &linked], &mut |_| false)
            .unwrap()
            .live;
        let slugs: Vec<_> = slots.iter().map(|s| s.slug.as_str()).collect();
        assert_eq!(slugs, vec!["feat", "v1-2", "v1-2-2", "feat-2"]);
    }

    #[test]
    fn concurrent_assignments_never_hand_out_the_same_slot() {
        // Sub-agents create worktrees and run `devcli start` at the same time,
        // each seeing a different subset: no slot may be lost or duplicated.
        let dir = tempfile::TempDir::new().unwrap();
        let roots: Vec<PathBuf> = (0..8).map(|i| dir.path().join(format!("w{i}"))).collect();
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let devcli = dir.path().join("devcli");
                let roots = roots.clone();
                std::thread::spawn(move || {
                    let live: Vec<Worktree> = roots[..=i]
                        .iter()
                        .map(|r| worktree(r, r.file_name().and_then(|n| n.to_str())))
                        .collect();
                    let refs: Vec<&Worktree> = live.iter().collect();
                    Registry::new(&devcli)
                        .assign(Path::new("/repo/.git"), &refs, &mut |_| false)
                        .unwrap()
                        .live
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        let table = Registry::new(&dir.path().join("devcli")).load().unwrap();
        assert_eq!(table.worktrees.len(), 8, "a slot was lost");
        let mut indexes: Vec<_> = table.worktrees.iter().map(|s| s.index).collect();
        indexes.sort();
        assert_eq!(indexes, (1..=8).collect::<Vec<_>>(), "duplicate index");
    }

    #[test]
    fn peeking_writes_nothing_and_matches_what_assign_gives() {
        let dir = tempfile::TempDir::new().unwrap();
        let devcli = dir.path().join("devcli");
        let registry = Registry::new(&devcli);
        let a = worktree(&dir.path().join("a"), Some("a"));

        let peeked = registry
            .peek(Path::new("/repo/.git"), &[&a], &mut |_| false)
            .unwrap()
            .live;
        assert!(!devcli.exists(), "peeking created the devcli directory");
        assert_eq!(
            registry
                .assign(Path::new("/repo/.git"), &[&a], &mut |_| false)
                .unwrap()
                .live,
            peeked
        );
    }

    #[test]
    fn a_directory_that_is_no_longer_a_checkout_releases_its_slot() {
        let dir = tempfile::TempDir::new().unwrap();
        let registry = Registry::new(&dir.path().join("devcli"));
        let repo = Path::new("/repo/.git");
        let a = worktree(&dir.path().join("a"), Some("a"));
        assert_eq!(
            registry.assign(repo, &[&a], &mut |_| false).unwrap().live[0].index,
            1
        );

        // `git worktree remove --force` kept the directory, without `.git`.
        std::fs::remove_file(a.root.join(".git")).unwrap();
        let b = worktree(&dir.path().join("b"), Some("b"));
        assert_eq!(
            registry.assign(repo, &[&b], &mut |_| false).unwrap().live[0].index,
            1
        );
    }

    #[test]
    fn a_worktree_an_app_still_runs_from_keeps_its_slot() {
        let dir = tempfile::TempDir::new().unwrap();
        let registry = Registry::new(&dir.path().join("devcli"));
        let repo = Path::new("/repo/.git");
        let a = worktree(&dir.path().join("a"), Some("a"));
        let slot = registry.assign(repo, &[&a], &mut |_| false).unwrap().live[0].clone();

        // Moved with `mv` while its api runs: git lists the old path as
        // prunable, so callers no longer pass it as live.
        std::fs::rename(&a.root, dir.path().join("moved")).unwrap();
        let running = a.root.clone();
        let mut held = |root: &Path| root == running;
        let b = worktree(&dir.path().join("b"), Some("b"));
        let now = registry.assign(repo, &[&b], &mut held).unwrap();
        assert_eq!(now.held, vec![slot.clone()], "still nameable");
        assert_eq!(now.live[0].index, 2, "its port slot is not handed out");
        assert_eq!(registry.peek(repo, &[&b], &mut held).unwrap(), now);

        // Once nothing runs from it, the slot goes.
        let after = registry.assign(repo, &[&b], &mut |_| false).unwrap();
        assert!(after.held.is_empty());
        let c = worktree(&dir.path().join("c"), Some("c"));
        let reused = registry.assign(repo, &[&b, &c], &mut |_| false).unwrap();
        assert_eq!(reused.live[1].index, 1);
    }
}
