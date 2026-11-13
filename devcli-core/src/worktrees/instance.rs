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
    /// From the command line, to start an app: the current worktree wins,
    /// and an app that does not exist on its branch runs from the configured
    /// checkout (with a note).
    FromCwd,
    /// From the command line, to act on an instance (`stop`, `restart`,
    /// `logs`, `status`, `health-check`): the current worktree wins, and an
    /// app that does not exist on its branch is an error. Falling back to the
    /// configured checkout there would make `stop api` run in a worktree stop
    /// the main checkout's api.
    FromCwdStrict,
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
///
/// A project of the config is pinned with a trailing `@`, even when its own
/// name contains one (`org@team` → `org@team@`); an instance name
/// (`proj@slug`) is already exact.
pub fn pinned(config: &Config, project: &str) -> String {
    if config.projects.contains_key(project) || !project.contains('@') {
        format!("{project}@")
    } else {
        project.to_string()
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
    resolve_all_with(config, app_name, project, mode, true)
}

/// [`resolve_all`] for commands that only look (`status`, which processes
/// `stop` selects): worktree slots are computed, never recorded.
pub(crate) fn resolve_all_readonly(
    config: &Config,
    app_name: &str,
    project: Option<&str>,
    mode: Mode,
) -> Result<Vec<ResolvedApp>> {
    resolve_all_with(config, app_name, project, mode, false)
}

fn resolve_all_with(
    config: &Config,
    app_name: &str,
    project: Option<&str>,
    mode: Mode,
    persist: bool,
) -> Result<Vec<ResolvedApp>> {
    let cwd = std::env::current_dir().ok();
    resolve_all_at(config, app_name, project, mode, persist, cwd.as_deref())
}

/// [`resolve_all_with`] as if run from `cwd`.
fn resolve_all_at(
    config: &Config,
    app_name: &str,
    project: Option<&str>,
    mode: Mode,
    persist: bool,
    cwd: Option<&Path>,
) -> Result<Vec<ResolvedApp>> {
    let (app, project, selector) = parse(config, app_name, project)?;
    let selector = match (selector, mode) {
        (Selector::Auto, Mode::Exact) => Selector::Base,
        (s, _) => s,
    };

    let base = resolve_app(config, app, project)?;
    match selector {
        Selector::Base => Ok(vec![base]),
        Selector::Auto => Ok(vec![current(base, persist, mode, cwd)?]),
        Selector::Slug(slug) => {
            let Some(view) = View::of(&base, persist)? else {
                anyhow::bail!(
                    "'{}' is not in a git repository, so it has no worktree '{slug}'",
                    base.app_name
                );
            };
            let Some((worktree, slot, gone)) = view.find(&slug) else {
                anyhow::bail!(
                    "Project '{}' has no worktree '{slug}' (worktrees: {})",
                    base.project,
                    view.slugs()
                );
            };
            Ok(vec![derive(&base, &view.home, worktree, slot, !gone)?])
        }
        Selector::All => {
            let mut all = Vec::new();
            if let Some(view) = View::of(&base, persist)? {
                for (worktree, slot) in &view.slots {
                    // Apps added on one branch only exist in some worktrees.
                    if let Ok(instance) = derive(&base, &view.home, worktree, slot, true) {
                        all.push(instance);
                    }
                }
                // What still runs from a worktree that is gone can be
                // stopped or looked at, not started.
                if !persist {
                    for (worktree, slot) in &view.gone {
                        all.extend(derive(&base, &view.home, worktree, slot, false));
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
    one(
        resolve_all(config, app_name, project, mode)?,
        app_name,
        project,
    )
}

/// [`resolve`] for commands that only look (`logs`, `status`,
/// `health-check`, dependency lookups): worktree slots are computed, never
/// recorded.
pub fn resolve_readonly(
    config: &Config,
    app_name: &str,
    project: Option<&str>,
    mode: Mode,
) -> Result<ResolvedApp> {
    one(
        resolve_all_readonly(config, app_name, project, mode)?,
        app_name,
        project,
    )
}

/// `(project@slug, app)` when the name selects a worktree by slug (`api@feat`
/// or `--project proj@feat`), whether or not that worktree still exists: the
/// logs of an instance outlive the worktree it ran from.
pub fn slug_instance_name(
    config: &Config,
    app_name: &str,
    project: Option<&str>,
) -> Option<(String, String)> {
    let Ok((app, project, Selector::Slug(slug))) = parse(config, app_name, project) else {
        return None;
    };
    let base = resolve_app(config, app, project).ok()?;
    Some((format!("{}@{slug}", base.project), base.app_name))
}

/// The app name, project and selector of `app_name` and `project`, their
/// suffixes removed. A name that really contains `@` in the config is taken
/// as is.
fn parse<'a>(
    config: &Config,
    app_name: &'a str,
    project: Option<&'a str>,
) -> Result<(&'a str, Option<&'a str>, Selector)> {
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
    Ok((app, project, combine(app_sel, project_sel)?))
}

fn one(mut all: Vec<ResolvedApp>, app_name: &str, project: Option<&str>) -> Result<ResolvedApp> {
    if all.len() != 1 || app_name.ends_with("@*") || project.is_some_and(|p| p.ends_with("@*")) {
        let name = match project {
            Some(p) => format!("{app_name} --project {p}"),
            None => app_name.to_string(),
        };
        anyhow::bail!("'{name}' designates several instances; name one worktree instead");
    }
    Ok(all.remove(0))
}

/// Whether `here` is a worktree of the superproject `home` is a submodule
/// of (or a checkout of the submodule inside such a worktree), other than
/// the superproject checkout holding `home`.
///
/// The submodule is a repository of its own: its worktrees are not the
/// superproject's, so devcli has no instance of the app there.
fn in_superproject_worktree(here: &Checkout, home: &Checkout) -> bool {
    let Some(home_super) = git::superproject(&home.root).and_then(|r| git::checkout_of(&r)) else {
        return false;
    };
    let here_super = git::superproject(&here.root).and_then(|r| git::checkout_of(&r));
    std::iter::once(here.clone())
        .chain(here_super)
        .any(|c| c.common_dir == home_super.common_dir && c.root != home_super.root)
}

/// The instance of `base` for the current directory: its worktree when the
/// command runs inside another worktree of the same repository.
fn current(
    base: ResolvedApp,
    persist: bool,
    mode: Mode,
    cwd: Option<&Path>,
) -> Result<ResolvedApp> {
    let strict = mode == Mode::FromCwdStrict;
    let Some(cwd) = cwd else {
        return Ok(base);
    };
    let Some(here) = git::checkout_of(cwd) else {
        return Ok(base);
    };
    let Some(home) = git::checkout_of(&expand_path(&base.app.path)) else {
        return Ok(base);
    };
    if here.root == home.root {
        return Ok(base);
    }
    if here.common_dir != home.common_dir && in_superproject_worktree(&here, &home) {
        let what = format!(
            "'{}' lives in the git submodule {}: a worktree of its superproject, like {}, holds \
             no instance of it",
            base.app_name,
            home.root.display(),
            here.root.display()
        );
        if strict {
            anyhow::bail!(
                "{what}. Name the instance: {app}@ for the configured checkout, {app}@SLUG for a \
                 worktree of the submodule.",
                app = base.app_name
            );
        }
        if crate::output::human(false) {
            eprintln!("note: {what}; using the configured checkout");
        }
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

    let Some(view) = View::of(&base, persist)? else {
        return Ok(base);
    };
    let Some((worktree, slot)) = view.slots.iter().find(|(w, _)| w.root == here.root) else {
        // A worktree of this repository that git does not list at this path:
        // moved with `mv`. Falling back to the configured checkout would make
        // `stop api` run here stop the main checkout's api.
        let slug = view
            .moved_to(&here.root)
            .map_or("SLUG", |slot| slot.slug.as_str());
        anyhow::bail!(
            "{} is a worktree of the repository of project '{}' that git does not list at this path \
             (moved without `git worktree move`?). Run `git worktree repair` here, or name \
             the instance: {app}@{slug} for a worktree, {app}@ for the configured checkout.",
            here.root.display(),
            base.project,
            app = base.app_name
        );
    };
    match derive(&base, &view.home, worktree, slot, true) {
        Ok(instance) => Ok(instance),
        // The app does not exist on this worktree's branch. `stop api` and
        // the like must not then reach the main checkout's api.
        Err(e) if strict => anyhow::bail!(
            "{e}, the worktree this command runs in. Name the instance: {app}@ for the \
             configured checkout, {app}@SLUG for another worktree.",
            app = base.app_name
        ),
        Err(e) => {
            // `start api` runs it from the configured checkout instead.
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
    /// Worktrees moved or deleted while an app still runs from them: they
    /// can be named (to stop it, read its logs), not started in.
    gone: Vec<(Worktree, Slot)>,
}

impl View {
    fn of(base: &ResolvedApp, persist: bool) -> Result<Option<Self>> {
        let Some(home) = git::checkout_of(&expand_path(&base.app.path)) else {
            return Ok(None);
        };
        let worktrees = git::list(&home.common_dir)?;
        let others: Vec<&Worktree> = worktrees
            .iter()
            .filter(|w| w.is_usable() && w.root != home.root)
            .collect();
        let registry = Registry::open();
        let mut held = super::registry::running_from();
        let assignment = if persist {
            registry.assign(&home.common_dir, &others, &mut held)?
        } else {
            registry.peek(&home.common_dir, &others, &mut held)?
        };
        let slots = others.into_iter().cloned().zip(assignment.live).collect();
        let gone = assignment
            .held
            .into_iter()
            .map(|slot| {
                // Git still lists a deleted or moved worktree (prunable)
                // until `git worktree prune`.
                let listed = worktrees.iter().find(|w| w.root == slot.root);
                let worktree = Worktree {
                    root: slot.root.clone(),
                    admin_name: listed.and_then(|w| w.admin_name.clone()),
                    branch: listed.and_then(|w| w.branch.clone()),
                    bare: false,
                    prunable: true,
                    locked: listed.is_some_and(|w| w.locked),
                };
                (worktree, slot)
            })
            .collect();
        Ok(Some(Self { home, slots, gone }))
    }

    /// The worktree `slug` names, and whether it is gone.
    fn find(&self, slug: &str) -> Option<(&Worktree, &Slot, bool)> {
        let live = self.slots.iter().map(|(w, s)| (w, s, false));
        let gone = self.gone.iter().map(|(w, s)| (w, s, true));
        live.chain(gone).find(|(_, s, _)| s.slug == slug)
    }

    /// The slot of a worktree moved with `mv` to `root`, while an app still
    /// runs from it: git keeps its admin directory under the same name.
    fn moved_to(&self, root: &Path) -> Option<&Slot> {
        let admin = git::admin_name_of(root)?;
        self.gone
            .iter()
            .find(|(w, _)| w.admin_name.as_deref() == Some(admin.as_str()))
            .map(|(_, s)| s)
    }

    fn slugs(&self) -> String {
        if self.slots.is_empty() && self.gone.is_empty() {
            return "none".to_string();
        }
        self.slots
            .iter()
            .chain(&self.gone)
            .map(|(_, s)| s.slug.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// `base`, moved into `worktree`. `require_dir`: the app's directory must
/// exist there (always, but for a worktree that is gone).
fn derive(
    base: &ResolvedApp,
    home: &Checkout,
    worktree: &Worktree,
    slot: &Slot,
    require_dir: bool,
) -> Result<ResolvedApp> {
    let base_path = git::canonical(&expand_path(&base.app.path));
    let relative = base_path.strip_prefix(&home.root).unwrap_or(Path::new(""));
    let path = worktree.root.join(relative);
    if require_dir && !path.is_dir() {
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
        let config: Config = serde_json::from_value(serde_json::json!({ "projects": {
            "proj": { "apps": {} }, "org@team": { "apps": {} }
        } }))
        .unwrap();
        assert_eq!(pinned(&config, "proj"), "proj@");
        assert_eq!(pinned(&config, "proj@feat"), "proj@feat");
        assert_eq!(pinned(&config, "org@team"), "org@team@");
        assert_eq!(split("org@team@"), ("org@team", Selector::Base));
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

    /// `stop api` run in a worktree whose branch has no api used to stop the
    /// main checkout's api; only `start api` falls back to it.
    #[test]
    fn an_app_missing_from_the_current_worktree_is_an_error_except_to_start_it() {
        let f = fixture();
        let here = Some(f.feat.as_path());
        let found =
            resolve_all_at(&f.config, "api", None, Mode::FromCwdStrict, false, here).unwrap();
        assert_eq!(found[0].project, "proj@feat");

        std::fs::remove_dir_all(f.feat.join("apps")).unwrap();
        let err =
            resolve_all_at(&f.config, "api", None, Mode::FromCwdStrict, false, here).unwrap_err();
        let err = err.to_string();
        assert!(err.contains("not in worktree 'feat'"), "{err}");
        assert!(err.contains("api@ for the configured checkout"), "{err}");

        let started = resolve_all_at(&f.config, "api", None, Mode::FromCwd, false, here).unwrap();
        assert_eq!(started[0].project, "proj");
        assert!(started[0].worktree.is_none());
    }

    #[test]
    fn several_instances_are_named_as_given() {
        let f = fixture();
        let err = resolve(&f.config, "api@*", None, Mode::FromCwd).unwrap_err();
        assert!(err.to_string().starts_with("'api@*' designates"), "{err}");
        let err = resolve(&f.config, "api", Some("proj@*"), Mode::FromCwd).unwrap_err();
        assert!(
            err.to_string().starts_with("'api --project proj@*'"),
            "{err}"
        );
    }

    /// An app inside a submodule belongs to the submodule's repository: a
    /// worktree of the superproject has no instance of it.
    #[test]
    fn a_superproject_worktree_is_not_a_worktree_of_its_submodules() {
        let _env = crate::test_utils::TempConfigDir::new();
        let dir = tempfile::TempDir::new().unwrap();
        let sub = init_repo(&dir.path().join("s"));
        let sup = init_repo(&dir.path().join("p"));
        run_git(
            &sup,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                sub.to_str().unwrap(),
                "sub",
            ],
        );
        run_git(&sup, &["commit", "-q", "-m", "sub"]);
        run_git(&sup, &["worktree", "add", "-q", "../wt/feat", "-b", "feat"]);
        let config: Config = serde_json::from_value(serde_json::json!({
            "projects": { "proj": { "apps": { "api": {
                "type": "custom",
                "path": sup.join("sub/apps/api"),
                "commands": { "local": { "start": "true" } },
                "defaults": { "local": "start" }
            } } } }
        }))
        .unwrap();
        let here = git::canonical(&dir.path().join("p/wt/feat"));

        let err = resolve_all_at(
            &config,
            "api",
            None,
            Mode::FromCwdStrict,
            false,
            Some(&here),
        )
        .unwrap_err();
        assert!(err.to_string().contains("submodule"), "{err}");
        let started =
            resolve_all_at(&config, "api", None, Mode::FromCwd, false, Some(&here)).unwrap();
        assert_eq!(started[0].project, "proj");
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
