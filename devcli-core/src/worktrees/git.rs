//! Which checkouts a git repository has.
//!
//! Everything comes from git itself, so worktrees created anywhere are found:
//! `.worktrees/`, `.claude/worktrees/`, sibling directories, the directories
//! agent harnesses keep under `~/.codex` or `~/.cursor`, bare-repository
//! layouts. Nothing is written by this module.

use crate::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::SystemTime;

/// The checkout (worktree) a path belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkout {
    /// Top-level directory of the checkout.
    pub root: PathBuf,
    /// The repository's shared git directory; identical for all its worktrees.
    pub common_dir: PathBuf,
}

/// One entry of `git worktree list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub root: PathBuf,
    /// Name of its admin directory (`<common_dir>/worktrees/<name>`); `None`
    /// for the main worktree. Git makes it unique within the repository.
    pub admin_name: Option<String>,
    /// Checked-out branch, without `refs/heads/`; `None` when detached.
    pub branch: Option<String>,
    pub bare: bool,
    /// Git reports it as prunable (its directory was deleted without
    /// `git worktree remove`).
    pub prunable: bool,
    pub locked: bool,
}

impl Worktree {
    /// Whether apps can run from it: a checkout whose directory exists.
    pub fn is_usable(&self) -> bool {
        !self.bare && !self.prunable && self.root.is_dir()
    }
}

/// `path` with symlinks resolved when it exists (macOS `/var` vs
/// `/private/var`), so paths from git and from the config compare equal.
pub fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The checkout containing `path`, or `None` outside a git repository (or in
/// a bare repository, which has no checkout).
///
/// Cached for the life of the process: the answer for a given directory does
/// not change while devcli runs.
pub fn checkout_of(path: &Path) -> Option<Checkout> {
    static CACHE: Mutex<Option<HashMap<PathBuf, Option<Checkout>>>> = Mutex::new(None);

    let path = canonical(path);
    if let Ok(mut cache) = CACHE.lock()
        && let Some(hit) = cache.get_or_insert_with(HashMap::new).get(&path)
    {
        return hit.clone();
    }

    let found = discover_checkout(&path);
    if let Ok(mut cache) = CACHE.lock() {
        cache
            .get_or_insert_with(HashMap::new)
            .insert(path, found.clone());
    }
    found
}

fn discover_checkout(path: &Path) -> Option<Checkout> {
    if !path.is_dir() {
        return None;
    }
    let out = git(
        path,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ],
    )?;
    let mut lines = out.lines();
    let root = lines.next()?.trim();
    let common_dir = lines.next()?.trim();
    if root.is_empty() || common_dir.is_empty() {
        return None;
    }
    Some(Checkout {
        root: canonical(Path::new(root)),
        common_dir: canonical(Path::new(common_dir)),
    })
}

/// The `origin` remote of the checkout at `root`, to recognise another clone
/// of the same repository.
pub fn origin_url(root: &Path) -> Option<String> {
    git(root, &["config", "--get", "remote.origin.url"])
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

/// Every worktree of the repository whose shared git directory is
/// `common_dir`, main worktree first.
///
/// Cached while `<common_dir>/worktrees` is unchanged: creating or removing a
/// worktree touches that directory, and the TUI asks several times a second.
pub fn list(common_dir: &Path) -> Result<Vec<Worktree>> {
    type Entry = (Option<SystemTime>, Vec<Worktree>);
    static CACHE: Mutex<Option<HashMap<PathBuf, Entry>>> = Mutex::new(None);

    let stamp = std::fs::metadata(common_dir.join("worktrees"))
        .and_then(|m| m.modified())
        .ok();
    if let Ok(mut cache) = CACHE.lock()
        && let Some((cached_stamp, worktrees)) =
            cache.get_or_insert_with(HashMap::new).get(common_dir)
        && *cached_stamp == stamp
    {
        return Ok(worktrees.clone());
    }

    let out = git(common_dir, &["worktree", "list", "--porcelain"])
        .ok_or_else(|| anyhow::anyhow!("`git worktree list` failed in {}", common_dir.display()))?;
    let worktrees = parse_porcelain(&out, &admin_names(common_dir));
    if let Ok(mut cache) = CACHE.lock() {
        cache
            .get_or_insert_with(HashMap::new)
            .insert(common_dir.to_path_buf(), (stamp, worktrees.clone()));
    }
    Ok(worktrees)
}

/// Admin directory name of every linked worktree, keyed by its root.
///
/// Read from `<common_dir>/worktrees/<name>/gitdir` (the path of the
/// worktree's `.git` file), which is still there when the worktree directory
/// itself has been deleted.
fn admin_names(common_dir: &Path) -> HashMap<PathBuf, String> {
    let mut names = HashMap::new();
    let Ok(entries) = std::fs::read_dir(common_dir.join("worktrees")) else {
        return names;
    };
    for entry in entries.flatten() {
        let Ok(gitdir) = std::fs::read_to_string(entry.path().join("gitdir")) else {
            continue;
        };
        let dot_git = PathBuf::from(gitdir.trim());
        if let Some(root) = dot_git.parent() {
            names.insert(
                canonical(root),
                entry.file_name().to_string_lossy().into_owned(),
            );
        }
    }
    names
}

/// Parse `git worktree list --porcelain`.
fn parse_porcelain(out: &str, admin_names: &HashMap<PathBuf, String>) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    for block in out.split("\n\n") {
        let mut root = None;
        let mut branch = None;
        let (mut bare, mut prunable, mut locked) = (false, false, false);
        for line in block.lines() {
            let (key, value) = line.split_once(' ').unwrap_or((line, ""));
            match key {
                "worktree" => root = Some(canonical(Path::new(value))),
                "branch" => {
                    branch = Some(
                        value
                            .strip_prefix("refs/heads/")
                            .unwrap_or(value)
                            .to_string(),
                    )
                }
                "bare" => bare = true,
                "prunable" => prunable = true,
                "locked" => locked = true,
                _ => {}
            }
        }
        let Some(root) = root else {
            continue;
        };
        // Git always lists the main worktree first.
        let admin_name = if worktrees.is_empty() {
            None
        } else {
            admin_names.get(&root).cloned()
        };
        worktrees.push(Worktree {
            root,
            admin_name,
            branch,
            bare,
            prunable,
            locked,
        });
    }
    worktrees
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Run git in `dir`, panicking on failure.
    pub(crate) fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .output()
            .expect("run git");
        assert!(
            status.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    /// A repository at `<dir>/repo` with one commit.
    pub(crate) fn init_repo(dir: &Path) -> PathBuf {
        let repo = dir.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        run_git(&repo, &["init", "-q", "-b", "main"]);
        std::fs::create_dir_all(repo.join("apps/api")).unwrap();
        std::fs::write(repo.join("apps/api/README"), "api\n").unwrap();
        run_git(&repo, &["add", "."]);
        run_git(&repo, &["commit", "-q", "-m", "init"]);
        repo
    }

    #[test]
    fn porcelain_is_parsed() {
        let out = "worktree /r\nHEAD 1\nbranch refs/heads/main\n\n\
                   worktree /w/a\nHEAD 2\nbranch refs/heads/feature/deep\n\n\
                   worktree /w/b\nHEAD 3\ndetached\nlocked agent\n\n\
                   worktree /w/gone\nHEAD 4\nbranch refs/heads/old\nprunable gitdir file points to non-existent location\n";
        let names: HashMap<PathBuf, String> = [("/w/a", "a"), ("/w/b", "b1"), ("/w/gone", "gone")]
            .into_iter()
            .map(|(p, n)| (PathBuf::from(p), n.to_string()))
            .collect();
        let worktrees = parse_porcelain(out, &names);

        assert_eq!(worktrees.len(), 4);
        assert_eq!(worktrees[0].admin_name, None, "main worktree");
        assert_eq!(worktrees[0].branch.as_deref(), Some("main"));
        assert_eq!(worktrees[1].branch.as_deref(), Some("feature/deep"));
        assert_eq!(worktrees[1].admin_name.as_deref(), Some("a"));
        assert_eq!(worktrees[2].branch, None, "detached");
        assert!(worktrees[2].locked);
        assert_eq!(worktrees[2].admin_name.as_deref(), Some("b1"));
        assert!(worktrees[3].prunable);
    }

    #[test]
    fn worktrees_of_a_real_repository() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = init_repo(dir.path());
        // Same directory name twice: git disambiguates the admin names.
        run_git(
            &repo,
            &["worktree", "add", "-q", "../a/feat", "-b", "feat-a"],
        );
        run_git(
            &repo,
            &["worktree", "add", "-q", "../b/feat", "-b", "feat-b"],
        );
        run_git(&repo, &["worktree", "add", "-q", "--detach", "../gone"]);
        std::fs::remove_dir_all(dir.path().join("gone")).unwrap();

        let checkout = checkout_of(&dir.path().join("a/feat/apps/api")).expect("in a checkout");
        assert_eq!(checkout.root, canonical(&dir.path().join("a/feat")));
        let main = checkout_of(&repo).unwrap();
        assert_eq!(checkout.common_dir, main.common_dir);

        let worktrees = list(&checkout.common_dir).unwrap();
        let roots: Vec<_> = worktrees.iter().map(|w| w.root.clone()).collect();
        assert_eq!(roots[0], canonical(&repo));
        assert_eq!(worktrees.len(), 4);

        let names: Vec<_> = worktrees.iter().map(|w| w.admin_name.clone()).collect();
        assert_eq!(names[1].as_deref(), Some("feat"));
        assert_eq!(names[2].as_deref(), Some("feat1"));
        assert!(worktrees[1].is_usable());
        assert!(!worktrees[3].is_usable(), "deleted with rm -rf");

        assert_eq!(checkout_of(dir.path()), None, "not a repository");
    }
}
