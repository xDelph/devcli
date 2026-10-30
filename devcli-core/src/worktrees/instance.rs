//! Instances of a configured app: the configured checkout, or any worktree
//! of its repository.
//!
//! A worktree instance is the configured app with its path moved into the
//! worktree, under the project name `project@slug`. Process ids, state files
//! and log files all derive from the project name, so instances never collide
//! with each other or with the configured checkout.
//!
//! Selection, by name suffix (on the app or the project):
//! - `api`        the worktree the current directory is in, else the configured checkout
//! - `api@feat`   the worktree whose slug is `feat`
//! - `api@`       the configured checkout, wherever the command runs from
//! - `api@*`      every checkout that has the app (start, stop, status)

use super::git::{self, Checkout, Worktree};
use super::registry::{Registry, Slot};
use crate::config::models::Config;
use crate::config::resolver::{resolve_app, ResolvedApp};
use crate::utils::path::expand_path;
use crate::Result;
use std::path::{Path, PathBuf};

/// Port offset between two consecutive worktree slots: slot 1 runs an app
/// declared on port 3000 on 3100, slot 2 on 3200. Wide enough for apps of the
/// same project declared on neighbouring ports.
pub const PORT_STRIDE: u32 = 100;

/// A worktree instance of a configured app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// Name of the worktree in devcli (`project@slug`).
    pub slug: String,
    /// Its slot: 1, 2, ... (the configured checkout is 0).
    pub index: u32,
    /// The project in `config.json`.
    pub base_project: String,
    /// Root of the worktree.
    pub root: PathBuf,
    /// Root of the configured checkout.
    pub base_root: PathBuf,
    /// The app's directory in the configured checkout.
    pub base_path: PathBuf,
    /// The repository's shared git directory.
    pub common_dir: PathBuf,
    pub branch: Option<String>,
}

impl Instance {
    pub fn port_offset(&self) -> u32 {
        self.index * PORT_STRIDE
    }
}

/// Which instance(s) a name designates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// The current directory decides.
    Auto,
    /// The configured checkout.
    Base,
    Slug(String),
    All,
}

/// How a name without suffix is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// From the command line: the current worktree wins.
    FromCwd,
    /// From devcli itself (dependencies, restarts, the TUI): a name without
    /// suffix is the configured checkout, whatever the current directory.
    Exact,
}

/// `name@suffix` → `(name, selector)`; a name without `@` is `Auto`.
pub fn split(name: &str) -> (&str, Selector) {
    match name.rsplit_once('@') {
        None => (name, Selector::Auto),
        Some((base, "")) => (base, Selector::Base),
        Some((base, "*")) => (base, Selector::All),
        Some((base, slug)) => (base, Selector::Slug(slug.to_string())),
    }
}

/// The name that designates exactly this project's instance (`proj@` for the
/// configured checkout), for devcli's own follow-up commands.
pub fn pinned(project: &str) -> String {
    if project.contains('@') {
        project.to_string()
    } else {
        format!("{project}@")
    }
}

/// A suffix on both the app and the project must agree.
fn combine(app: Selector, project: Selector) -> Result<Selector> {
    match (app, project) {
        (Selector::Auto, s) | (s, Selector::Auto) => Ok(s),
        (a, p) if a == p => Ok(a),
        (a, p) => {
            anyhow::bail!("Conflicting worktree selections: {a:?} on the app, {p:?} on the project")
        }
    }
}

pub(crate) fn app_named(config: &Config, name: &str, project: Option<&str>) -> bool {
    config
        .projects
        .iter()
        .filter(|(p, _)| project.is_none_or(|filter| filter == p.as_str()))
        .any(|(_, p)| {
            p.apps.contains_key(name)
                || p.apps
                    .values()
                    .any(|a| a.alternative_name.as_deref() == Some(name))
        })
}

/// Resolve `app_name` (and `project`) to every instance it designates.
pub fn resolve_all(
    config: &Config,
    app_name: &str,
    project: Option<&str>,
    mode: Mode,
) -> Result<Vec<ResolvedApp>> {
    // A name that really contains `@` in the config is taken as is.
    let (project, project_sel) = match project {
        Some(p) if p.contains('@') && !config.projects.contains_key(p) => {
            let (p, s) = split(p);
            (Some(p), s)
        }
        other => (other, Selector::Auto),
    };
    let (app, app_sel) = if app_name.contains('@') && !app_named(config, app_name, project) {
        split(app_name)
    } else {
        (app_name, Selector::Auto)
    };
    let selector = match (combine(app_sel, project_sel)?, mode) {
        (Selector::Auto, Mode::Exact) => Selector::Base,
        (s, _) => s,
    };

    let base = resolve_app(config, app, project)?;
    match selector {
        Selector::Base => Ok(vec![base]),
        Selector::Auto => Ok(vec![current(base)?]),
        Selector::Slug(slug) => {
            let Some(view) = View::of(&base)? else {
                anyhow::bail!(
                    "'{}' is not in a git repository, so it has no worktree '{slug}'",
                    base.app_name
                );
            };
            let Some((worktree, slot)) = view.find(&slug) else {
                anyhow::bail!(
                    "Project '{}' has no worktree '{slug}' (worktrees: {})",
                    base.project,
                    view.slugs()
                );
            };
            Ok(vec![derive(&base, &view.home, worktree, slot)?])
        }
        Selector::All => {
            let mut all = Vec::new();
            if let Some(view) = View::of(&base)? {
                for (worktree, slot) in &view.slots {
                    // Apps added on one branch only exist in some worktrees.
                    if let Ok(instance) = derive(&base, &view.home, worktree, slot) {
                        all.push(instance);
                    }
                }
            }
            all.insert(0, base);
            Ok(all)
        }
    }
}

/// Resolve `app_name` (and `project`) to exactly one instance.
pub fn resolve(
    config: &Config,
    app_name: &str,
    project: Option<&str>,
    mode: Mode,
) -> Result<ResolvedApp> {
    let mut all = resolve_all(config, app_name, project, mode)?;
    if all.len() != 1 || app_name.ends_with("@*") || project.is_some_and(|p| p.ends_with("@*")) {
        anyhow::bail!("'{app_name}@*' designates several instances; name one worktree instead");
    }
    Ok(all.remove(0))
}

/// The instance of `base` for the current directory: its worktree when the
/// command runs inside another worktree of the same repository.
fn current(base: ResolvedApp) -> Result<ResolvedApp> {
    let Ok(cwd) = std::env::current_dir() else {
        return Ok(base);
    };
    let Some(here) = git::checkout_of(&cwd) else {
        return Ok(base);
    };
    let Some(home) = git::checkout_of(&expand_path(&base.app.path)) else {
        return Ok(base);
    };
    if here.root == home.root {
        return Ok(base);
    }
    if here.common_dir != home.common_dir {
        // Another clone of the same repository: devcli runs worktrees of the
        // configured checkout, not clones; say which one is used.
        if git::origin_url(&here.root).is_some_and(|url| git::origin_url(&home.root) == Some(url))
            && crate::output::human(false)
        {
            eprintln!(
                "note: {} is another clone of the repository of project '{}'; '{}' runs from the \
                 configured checkout, {}",
                here.root.display(),
                base.project,
                base.app_name,
                home.root.display()
            );
        }
        return Ok(base);
    }

    let Some(view) = View::of(&base)? else {
        return Ok(base);
    };
    let Some((worktree, slot)) = view.slots.iter().find(|(w, _)| w.root == here.root) else {
        // A worktree of this repository that git does not list at this path:
        // moved with `mv`. Falling back to the configured checkout would make
        // `stop api` run here stop the main checkout's api.
        anyhow::bail!(
            "{} is a worktree of the repository of project '{}' that git does not list at this path \
             (moved without `git worktree move`?). Run `git worktree repair` here, or name \
             the instance: {app}@SLUG for a worktree, {app}@ for the configured checkout.",
            here.root.display(),
            base.project,
            app = base.app_name
        );
    };
    match derive(&base, &view.home, worktree, slot) {
        Ok(instance) => Ok(instance),
        Err(e) => {
            // The app does not exist on this worktree's branch.
            tracing::warn!(error = %e, "Using the configured checkout");
            if crate::output::human(false) {
                eprintln!("note: {e}; using the configured checkout");
            }
            Ok(base)
        }
    }
}

/// The worktrees an app can run from, besides its configured checkout.
struct View {
    home: Checkout,
    slots: Vec<(Worktree, Slot)>,
}

impl View {
    fn of(base: &ResolvedApp) -> Result<Option<Self>> {
        let Some(home) = git::checkout_of(&expand_path(&base.app.path)) else {
            return Ok(None);
        };
        let worktrees = git::list(&home.common_dir)?;
        let others: Vec<&Worktree> = worktrees
            .iter()
            .filter(|w| w.is_usable() && w.root != home.root)
            .collect();
        let slots = Registry::open().assign(&home.common_dir, &others)?;
        let slots = others.into_iter().cloned().zip(slots).collect();
        Ok(Some(Self { home, slots }))
    }

    fn find(&self, slug: &str) -> Option<(&Worktree, &Slot)> {
        self.slots
            .iter()
            .find(|(_, s)| s.slug == slug)
            .map(|(w, s)| (w, s))
    }

    fn slugs(&self) -> String {
        if self.slots.is_empty() {
            return "none".to_string();
        }
        self.slots
            .iter()
            .map(|(_, s)| s.slug.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// `base`, moved into `worktree`.
fn derive(
    base: &ResolvedApp,
    home: &Checkout,
    worktree: &Worktree,
    slot: &Slot,
) -> Result<ResolvedApp> {
    let base_path = git::canonical(&expand_path(&base.app.path));
    let relative = base_path.strip_prefix(&home.root).unwrap_or(Path::new(""));
    let path = worktree.root.join(relative);
    if !path.is_dir() {
        anyhow::bail!(
            "'{}' is not in worktree '{}' ({} does not exist)",
            base.app_name,
            slot.slug,
            path.display()
        );
    }

    let mut app = base.app.clone();
    app.path = path.to_string_lossy().into_owned();
    Ok(ResolvedApp {
        project: format!("{}@{}", base.project, slot.slug),
        app_name: base.app_name.clone(),
        app,
        worktree: Some(Instance {
            slug: slot.slug.clone(),
            index: slot.index,
            base_project: base.project.clone(),
            root: worktree.root.clone(),
            base_root: home.root.clone(),
            base_path,
            common_dir: home.common_dir.clone(),
            branch: worktree.branch.clone(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worktrees::git::tests::{init_repo, run_git};

    #[test]
    fn names_are_split_on_the_last_at() {
        assert_eq!(split("api"), ("api", Selector::Auto));
        assert_eq!(split("api@"), ("api", Selector::Base));
        assert_eq!(split("api@*"), ("api", Selector::All));
        assert_eq!(split("api@feat"), ("api", Selector::Slug("feat".into())));
        assert_eq!(split("a@b@c"), ("a@b", Selector::Slug("c".into())));
        assert_eq!(pinned("proj"), "proj@");
        assert_eq!(pinned("proj@feat"), "proj@feat");
    }

    #[test]
    fn suffixes_must_agree() {
        assert_eq!(
            combine(Selector::Auto, Selector::Slug("x".into())).unwrap(),
            Selector::Slug("x".into())
        );
        assert_eq!(
            combine(Selector::Base, Selector::Base).unwrap(),
            Selector::Base
        );
        assert!(combine(Selector::Slug("x".into()), Selector::Slug("y".into())).is_err());
    }

    /// A config with one app at `<repo>/apps/api`, and two worktrees.
    struct Fixture {
        _dir: tempfile::TempDir,
        _env: crate::test_utils::TempConfigDir,
        config: Config,
        repo: PathBuf,
        feat: PathBuf,
    }

    fn fixture() -> Fixture {
        let env = crate::test_utils::TempConfigDir::new();
        let dir = tempfile::TempDir::new().unwrap();
        let repo = init_repo(dir.path());
        run_git(
            &repo,
            &["worktree", "add", "-q", "../wt/feat", "-b", "feat"],
        );
        run_git(
            &repo,
            &["worktree", "add", "-q", "../wt/other", "-b", "other"],
        );
        let config: Config = serde_json::from_value(serde_json::json!({
            "projects": { "proj": { "apps": { "api": {
                "type": "custom",
                "path": repo.join("apps/api"),
                "commands": { "local": { "start": "true" } },
                "defaults": { "local": "start" }
            } } } }
        }))
        .unwrap();
        Fixture {
            feat: git::canonical(&dir.path().join("wt/feat")),
            repo: git::canonical(&repo),
            _dir: dir,
            _env: env,
            config,
        }
    }

    #[test]
    fn a_slug_selects_that_worktree() {
        let f = fixture();
        let api = resolve(&f.config, "api@feat", None, Mode::Exact).unwrap();
        assert_eq!(api.project, "proj@feat");
        assert_eq!(api.app_name, "api");
        assert_eq!(PathBuf::from(&api.app.path), f.feat.join("apps/api"));
        let instance = api.worktree.expect("a worktree instance");
        assert_eq!(instance.base_project, "proj");
        assert_eq!(instance.branch.as_deref(), Some("feat"));
        assert_eq!(instance.base_path, f.repo.join("apps/api"));
        assert!(instance.index >= 1);

        // Same answer through the project.
        let again = resolve(&f.config, "api", Some("proj@feat"), Mode::Exact).unwrap();
        assert_eq!(again.project, "proj@feat");

        let err = resolve(&f.config, "api@nope", None, Mode::Exact).unwrap_err();
        assert!(err.to_string().contains("feat, other"), "{err}");
    }

    #[test]
    fn exact_mode_and_an_empty_suffix_mean_the_configured_checkout() {
        let f = fixture();
        for (name, project) in [("api", None), ("api@", None), ("api", Some("proj@"))] {
            let api = resolve(&f.config, name, project, Mode::Exact).unwrap();
            assert_eq!(api.project, "proj");
            assert!(api.worktree.is_none());
        }
    }

    #[test]
    fn star_selects_every_checkout() {
        let f = fixture();
        let all = resolve_all(&f.config, "api@*", None, Mode::FromCwd).unwrap();
        let projects: Vec<_> = all.iter().map(|a| a.project.as_str()).collect();
        assert_eq!(projects.len(), 3);
        assert_eq!(projects[0], "proj");
        assert!(projects.contains(&"proj@feat") && projects.contains(&"proj@other"));
        assert!(resolve(&f.config, "api@*", None, Mode::FromCwd).is_err());
    }

    #[test]
    fn slots_survive_between_resolutions() {
        let f = fixture();
        let first = resolve(&f.config, "api@feat", None, Mode::Exact).unwrap();
        let second = resolve(&f.config, "api@feat", None, Mode::Exact).unwrap();
        assert_eq!(
            first.worktree.unwrap().index,
            second.worktree.unwrap().index
        );
    }

    #[test]
    fn an_app_missing_from_a_worktree_is_reported() {
        let f = fixture();
        std::fs::remove_dir_all(f.feat.join("apps")).unwrap();
        let err = resolve(&f.config, "api@feat", None, Mode::Exact).unwrap_err();
        assert!(err.to_string().contains("not in worktree 'feat'"), "{err}");
        // `@*` simply skips it.
        let all = resolve_all(&f.config, "api@*", None, Mode::Exact).unwrap();
        assert!(all.iter().all(|a| a.project != "proj@feat"));
    }

    #[test]
    fn apps_found_in_a_worktree_are_registered_at_the_main_checkout() {
        use crate::worktrees::{registration_for, Registration};
        let f = fixture();
        assert_eq!(
            registration_for(&f.repo.join("apps/api")),
            Registration::AsIs
        );
        assert_eq!(
            registration_for(&f.feat.join("apps/api")),
            Registration::MainCheckout {
                path: f.repo.join("apps/api"),
                worktree: "feat".into()
            }
        );
        std::fs::create_dir_all(f.feat.join("apps/new")).unwrap();
        assert_eq!(
            registration_for(&f.feat.join("apps/new")),
            Registration::WorktreeOnly {
                worktree: "feat".into()
            }
        );
    }
}
