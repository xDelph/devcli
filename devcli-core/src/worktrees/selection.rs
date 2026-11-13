//! Which tracked processes `[APP] [--project P]` designates (`stop`,
//! `status`).
//!
//! Matching processes by app name alone made `stop api` stop whichever `api`
//! came first, possibly another worktree's, and `stop --project proj` run by
//! a sub-agent inside a worktree would have stopped the main checkout's apps.

use super::git;
use super::instance::{app_named, resolve_all_readonly, slug_roots, split, Mode, Selector};
use super::meta;
use crate::config::models::Config;
use crate::utils::path::expand_path;
use crate::Result;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// No filter.
    Everything,
    /// These `(project, app)` instances.
    Instances(Vec<(String, String)>),
    /// An app that is not (or no longer) in the config, by name.
    Named {
        app: String,
        project: Option<String>,
        /// Why the config does not resolve it (with a "did you mean"), for
        /// when no process matches either.
        not_found: String,
    },
    /// Every process of this exact project name (`proj` or `proj@slug`).
    Project(String),
    /// Every instance of a configured project (`proj@*`).
    AllOf(String),
    /// The instances of a configured project running from one worktree.
    Worktree { base: String, root: PathBuf },
    /// `proj@slug`: the instances of a configured project running from the
    /// worktrees called `slug` in its apps' repositories.
    Slug {
        base: String,
        slug: String,
        /// Those worktrees. Empty when no repository has one by that name:
        /// processes are then matched by name, and none is an error.
        roots: Vec<PathBuf>,
        not_found: String,
    },
}

/// The configured project a process belongs to (`proj` for `proj@slug`).
pub fn base_project(metadata: &HashMap<String, String>) -> Option<&str> {
    metadata
        .get(meta::BASE_PROJECT)
        .or_else(|| metadata.get("project"))
        .map(String::as_str)
}

impl Selection {
    pub fn of(config: &Config, app: Option<&str>, project: Option<&str>) -> Result<Self> {
        if let Some(app) = app {
            return match resolve_all_readonly(config, app, project, Mode::FromCwdStrict) {
                Ok(instances) => Ok(Selection::Instances(
                    instances
                        .into_iter()
                        .map(|i| (i.project, i.app_name))
                        .collect(),
                )),
                // An app removed from the config can still have processes:
                // those are found by name.
                Err(e) if !in_config(config, app, project) => Ok(Selection::Named {
                    app: app.to_string(),
                    project: project.map(str::to_string),
                    not_found: e.to_string(),
                }),
                // Any other failure (an ambiguous name, git or the worktree
                // registry failing) used to fall back to matching by name
                // too, across every project and worktree: `stop api` from a
                // worktree then stopped the main checkout's api and other
                // projects' apps called api.
                Err(e) => Err(e),
            };
        }

        let Some(project) = project else {
            return Ok(Selection::Everything);
        };
        if config.projects.contains_key(project) || !project.contains('@') {
            return Ok(match current_worktree_of(config, project) {
                Some(root) => Selection::Worktree {
                    base: project.to_string(),
                    root,
                },
                None => Selection::Project(project.to_string()),
            });
        }
        let (base, selector) = split(project);
        Ok(match selector {
            Selector::Base | Selector::Auto => Selection::Project(base.to_string()),
            // A project no longer in the config: its processes, by name.
            Selector::Slug(_) if !config.projects.contains_key(base) => {
                Selection::Project(project.to_string())
            }
            Selector::Slug(slug) => {
                let (mut roots, mut slugs) = slug_roots(config, base, &slug)?;
                // Run from inside one of them, the current one only.
                let here = std::env::current_dir()
                    .ok()
                    .and_then(|cwd| git::checkout_of(&cwd))
                    .map(|c| c.root);
                if let Some(here) = here.filter(|h| roots.contains(h)) {
                    roots = vec![here];
                }
                slugs.sort();
                let known = if slugs.is_empty() {
                    "none".to_string()
                } else {
                    slugs.join(", ")
                };
                Selection::Slug {
                    base: base.to_string(),
                    not_found: format!(
                        "Project '{base}' has no worktree '{slug}' (worktrees: {known})."
                    ),
                    slug,
                    roots,
                }
            }
            Selector::All => Selection::AllOf(base.to_string()),
        })
    }

    /// For a selection by name of an app that is not in the config, fail
    /// when nothing matches either: a typo used to answer "No processes to
    /// stop" with exit code 0.
    ///
    /// The same for a worktree slug no repository of the project has.
    pub fn check_found(&self, matched: usize) -> Result<()> {
        if matched > 0 {
            return Ok(());
        }
        match self {
            Selection::Named { not_found, .. } => {
                anyhow::bail!("{not_found} No process of that name is tracked either.")
            }
            Selection::Slug {
                roots, not_found, ..
            } if roots.is_empty() => {
                anyhow::bail!("{not_found} No process of that name is tracked either.")
            }
            _ => Ok(()),
        }
    }

    /// For a selection by name, fail when it matches the processes of
    /// several projects instead of picking all of them.
    pub fn check_unambiguous<'a>(
        &self,
        matched: impl IntoIterator<Item = &'a HashMap<String, String>>,
    ) -> Result<()> {
        let Selection::Named {
            app, project: None, ..
        } = self
        else {
            return Ok(());
        };
        let mut projects: Vec<&str> = matched
            .into_iter()
            .filter_map(|m| m.get("project").map(String::as_str))
            .collect();
        projects.sort_unstable();
        projects.dedup();
        if projects.len() > 1 {
            anyhow::bail!(
                "'{app}' matches processes of several projects ({}); name one with --project",
                projects.join(", ")
            );
        }
        Ok(())
    }

    pub fn matches(&self, metadata: &HashMap<String, String>) -> bool {
        let project = metadata.get("project").map(String::as_str);
        let app = metadata.get("app_config_name").map(String::as_str);
        match self {
            Selection::Everything => true,
            Selection::Instances(instances) => instances
                .iter()
                .any(|(p, a)| project == Some(p) && app == Some(a)),
            Selection::Named {
                app: name,
                project: filter,
                ..
            } => {
                let named = app == Some(name)
                    || metadata.get("alternative_name").map(String::as_str) == Some(name);
                named && filter.as_deref().is_none_or(|f| project == Some(f))
            }
            Selection::Project(name) => project == Some(name),
            Selection::AllOf(base) => base_project(metadata) == Some(base),
            Selection::Worktree { base, root } => {
                base_project(metadata) == Some(base)
                    && metadata
                        .get(meta::WORKTREE_ROOT)
                        .map(PathBuf::from)
                        .as_ref()
                        == Some(root)
            }
            Selection::Slug {
                base, slug, roots, ..
            } => {
                project == Some(format!("{base}@{slug}").as_str())
                    && (roots.is_empty()
                        || metadata
                            .get(meta::WORKTREE_ROOT)
                            .is_some_and(|r| roots.contains(&PathBuf::from(r))))
            }
        }
    }
}

/// Whether `app` (with or without a worktree suffix) names an app of the
/// config, in `project` when given.
fn in_config(config: &Config, app: &str, project: Option<&str>) -> bool {
    let project = project.map(|p| {
        if config.projects.contains_key(p) {
            p
        } else {
            split(p).0
        }
    });
    app_named(config, app, project) || app_named(config, split(app).0, project)
}

/// The worktree the current directory is in, when it is a worktree (other
/// than the configured checkout) of a repository holding one of `project`'s
/// apps.
fn current_worktree_of(config: &Config, project: &str) -> Option<PathBuf> {
    let here = git::checkout_of(&std::env::current_dir().ok()?)?;
    let apps = &config.projects.get(project)?.apps;
    let homes: Vec<_> = apps
        .values()
        .filter_map(|app| git::checkout_of(&expand_path(&app.path)))
        .collect();
    let in_repo = homes.iter().any(|h| h.common_dir == here.common_dir);
    let is_home = homes.iter().any(|h| h.root == here.root);
    (in_repo && !is_home).then_some(here.root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn selections_match_the_right_processes() {
        let base = meta(&[("project", "proj"), ("app_config_name", "api")]);
        let feat = meta(&[
            ("project", "proj@feat"),
            ("app_config_name", "api"),
            (meta::BASE_PROJECT, "proj"),
            (meta::WORKTREE_ROOT, "/w/feat"),
        ]);

        let instances = Selection::Instances(vec![("proj@feat".into(), "api".into())]);
        assert!(!instances.matches(&base));
        assert!(instances.matches(&feat));

        let project = Selection::Project("proj".into());
        assert!(project.matches(&base) && !project.matches(&feat));

        let all = Selection::AllOf("proj".into());
        assert!(all.matches(&base) && all.matches(&feat));

        let worktree = Selection::Worktree {
            base: "proj".into(),
            root: "/w/feat".into(),
        };
        assert!(!worktree.matches(&base) && worktree.matches(&feat));
        let other = Selection::Worktree {
            base: "proj".into(),
            root: "/w/other".into(),
        };
        assert!(!other.matches(&feat));
    }

    #[test]
    fn a_slug_selects_its_worktrees_only_and_an_unknown_one_is_an_error() {
        let in_one = meta(&[
            ("project", "proj@feat"),
            ("app_config_name", "api"),
            (meta::BASE_PROJECT, "proj"),
            (meta::WORKTREE_ROOT, "/one/feat"),
        ]);
        // Another repository's unrelated `feat`, not designated here.
        let in_two = meta(&[
            ("project", "proj@feat"),
            ("app_config_name", "web"),
            (meta::BASE_PROJECT, "proj"),
            (meta::WORKTREE_ROOT, "/two/feat"),
        ]);
        let slug = |roots: Vec<PathBuf>| Selection::Slug {
            base: "proj".into(),
            slug: "feat".into(),
            roots,
            not_found: "Project 'proj' has no worktree 'feat'.".into(),
        };
        let one = slug(vec!["/one/feat".into()]);
        assert!(one.matches(&in_one) && !one.matches(&in_two));
        assert!(one.check_found(0).is_ok(), "nothing running there is fine");

        // No such worktree any more: by name, and nothing is an error.
        let gone = slug(Vec::new());
        assert!(gone.matches(&in_one) && gone.matches(&in_two));
        assert!(gone.check_found(1).is_ok());
        let err = gone.check_found(0).unwrap_err();
        assert!(err.to_string().contains("no worktree 'feat'"), "{err}");
    }

    #[test]
    fn project_suffixes() {
        let config: Config = serde_json::from_str(r#"{"projects":{}}"#).unwrap();
        assert_eq!(
            Selection::of(&config, None, Some("proj@*")).unwrap(),
            Selection::AllOf("proj".into())
        );
        assert_eq!(
            Selection::of(&config, None, Some("proj@feat")).unwrap(),
            Selection::Project("proj@feat".into())
        );
        assert_eq!(
            Selection::of(&config, None, Some("proj@")).unwrap(),
            Selection::Project("proj".into())
        );
        assert_eq!(
            Selection::of(&config, None, None).unwrap(),
            Selection::Everything
        );
    }

    #[test]
    fn an_ambiguous_name_is_an_error_not_every_match() {
        let config: Config = serde_json::from_value(serde_json::json!({ "projects": {
            "one": { "apps": { "api": { "type": "x", "path": "/nonexistent/one",
                "commands": {}, "defaults": {} } } },
            "two": { "apps": { "api": { "type": "x", "path": "/nonexistent/two",
                "commands": {}, "defaults": {} } } },
        } }))
        .unwrap();
        let err = Selection::of(&config, Some("api"), None).unwrap_err();
        assert!(err.to_string().contains("ambiguous"), "{err}");
        assert_eq!(
            Selection::of(&config, Some("api"), Some("one")).unwrap(),
            Selection::Instances(vec![("one".into(), "api".into())])
        );

        // An app no longer in the config is still found by name, but not
        // across projects.
        let gone = Selection::of(&config, Some("worker"), None).unwrap();
        assert!(matches!(gone, Selection::Named { .. }));
        let a = meta(&[("project", "one"), ("app_config_name", "worker")]);
        let b = meta(&[("project", "two"), ("app_config_name", "worker")]);
        assert!(gone.check_unambiguous([&a]).is_ok());
        assert!(gone.check_unambiguous([&a, &b]).is_err());
    }
}
