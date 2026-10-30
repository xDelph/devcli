//! Which tracked processes `[APP] [--project P]` designates (`stop`,
//! `status`).
//!
//! Matching processes by app name alone made `stop api` stop whichever `api`
//! came first, possibly another worktree's, and `stop --project proj` run by
//! a sub-agent inside a worktree would have stopped the main checkout's apps.

use super::git;
use super::instance::{resolve_all, split, Mode, Selector};
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
    },
    /// Every process of this exact project name (`proj` or `proj@slug`).
    Project(String),
    /// Every instance of a configured project (`proj@*`).
    AllOf(String),
    /// The instances of a configured project running from one worktree.
    Worktree { base: String, root: PathBuf },
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
            return Ok(match resolve_all(config, app, project, Mode::FromCwd) {
                Ok(instances) => Selection::Instances(
                    instances
                        .into_iter()
                        .map(|i| (i.project, i.app_name))
                        .collect(),
                ),
                Err(_) => Selection::Named {
                    app: app.to_string(),
                    project: project.map(str::to_string),
                },
            });
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
            Selector::Slug(_) => Selection::Project(project.to_string()),
            Selector::All => Selection::AllOf(base.to_string()),
        })
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
        }
    }
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
}
