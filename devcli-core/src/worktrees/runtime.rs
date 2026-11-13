//! What an instance's process gets so several worktrees can run the same app
//! side by side: environment variables, shifted ports, its own Docker Compose
//! project, health checks on the shifted ports.

use crate::config::resolver::ResolvedApp;
use crate::Result;
use process_manager::HealthCheck;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The variables every app gets (the configured checkout has slot 0).
pub const WORKTREE: &str = "DEVCLI_WORKTREE";
pub const WORKTREE_INDEX: &str = "DEVCLI_WORKTREE_INDEX";
pub const WORKTREE_PATH: &str = "DEVCLI_WORKTREE_PATH";
pub const WORKTREE_BRANCH: &str = "DEVCLI_WORKTREE_BRANCH";
pub const PORT_OFFSET: &str = "DEVCLI_PORT_OFFSET";
pub const DEVCLI_PORT: &str = "DEVCLI_PORT";

/// A port the instance listens on instead of the configured one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortShift {
    pub from: u16,
    pub to: u16,
}

/// What [`apply`] did, for health checks and the user.
#[derive(Debug, Default)]
pub struct Applied {
    pub shift: Option<PortShift>,
    pub notes: Vec<String>,
}

fn shifted(port: u16, offset: u32) -> Result<u16> {
    u16::try_from(u32::from(port) + offset)
        .map_err(|_| anyhow::anyhow!("Port {port} + worktree offset {offset} is above 65535"))
}

/// Add the instance's variables to `env_vars` (the variables devcli sets on
/// the process, env files included).
///
/// - `DEVCLI_WORKTREE` (slug, empty in the configured checkout),
///   `DEVCLI_WORKTREE_INDEX`, `DEVCLI_PORT_OFFSET` (index × 100), and in a
///   worktree `DEVCLI_WORKTREE_PATH` and `DEVCLI_WORKTREE_BRANCH`.
/// - `PORT` and `DEVCLI_PORT`: the env files' `PORT`, else the declared
///   `port`, shifted by the offset in a worktree (the copies would otherwise
///   all bind the same port). Without either, nothing is set.
/// - In a worktree, for Docker environments: `COMPOSE_PROJECT_NAME`, unless
///   set, since Compose names its project after the compose file's
///   directory, which is the same in every worktree.
pub fn apply(
    resolved: &ResolvedApp,
    environment: &str,
    env_vars: &mut HashMap<String, String>,
) -> Result<Applied> {
    let mut applied = Applied::default();
    let instance = resolved.worktree.as_ref();
    let offset = instance.map_or(0, |i| i.port_offset());

    env_vars.insert(
        WORKTREE.to_string(),
        instance.map(|i| i.slug.clone()).unwrap_or_default(),
    );
    env_vars.insert(
        WORKTREE_INDEX.to_string(),
        instance.map_or(0, |i| i.index).to_string(),
    );
    env_vars.insert(PORT_OFFSET.to_string(), offset.to_string());
    if let Some(i) = instance {
        env_vars.insert(
            WORKTREE_PATH.to_string(),
            i.root.to_string_lossy().into_owned(),
        );
        if let Some(branch) = &i.branch {
            env_vars.insert(WORKTREE_BRANCH.to_string(), branch.clone());
        }
    }

    // The port the app really uses in the configured checkout: the env
    // files' PORT when they set one (it always won there, and still does),
    // else the declared `port`. A worktree shifts that one.
    let from_env = env_vars
        .get("PORT")
        .and_then(|p| p.trim().parse::<u16>().ok());
    let base_port = from_env.or(resolved.app.port);
    if let Some(from) = base_port {
        let to = shifted(from, offset)?;
        env_vars.insert("PORT".to_string(), to.to_string());
        env_vars.insert(DEVCLI_PORT.to_string(), to.to_string());
        if to != from {
            applied.shift = Some(PortShift { from, to });
            applied.notes.push(format!(
                "Port {from} → {to} (worktree '{}')",
                instance.map(|i| i.slug.as_str()).unwrap_or_default()
            ));
        }
    }

    // Where the dependencies listen: in a worktree the API next to this web
    // is on its own shifted port, which a URL hard-coded to the configured
    // port would miss. DEVCLI_PORT_<APP> for each dependency with a port, as
    // it runs for this instance.
    if let Ok(config) = crate::config::load_config()
        && let Ok(chain) = crate::config::dependencies::resolve_dependency_chain(&config, resolved)
    {
        for dep in chain {
            if let Some(port) = dependency_port(&dep, environment)? {
                env_vars.insert(dependency_port_var(&dep.app_name), port.to_string());
            }
        }
    }

    if let Some(i) = instance
        && matches!(environment, "docker-compose" | "docker" | "orbstack")
        && !env_vars.contains_key("COMPOSE_PROJECT_NAME")
        && std::env::var_os("COMPOSE_PROJECT_NAME").is_none()
    {
        // Compose accepts lowercase letters, digits, `-` and `_`.
        let name: String = format!("{}-{}-{}", i.base_project, resolved.app_name, i.slug)
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        applied
            .notes
            .push(format!("Docker Compose project '{name}'"));
        env_vars.insert("COMPOSE_PROJECT_NAME".to_string(), name);
    }

    Ok(applied)
}

/// The port `dep` listens on: the one its running process got, else the
/// one starting it in `environment` gives - its env files' `PORT` (they
/// win over the declared `port`, as in [`apply`]), else its declared
/// `port` - shifted by its worktree offset.
fn dependency_port(dep: &ResolvedApp, environment: &str) -> Result<Option<u16>> {
    if let Some(port) = running_port(dep) {
        return Ok(Some(port));
    }
    let offset = dep.worktree.as_ref().map_or(0, |i| i.port_offset());
    env_file_port(dep, environment)
        .or(dep.app.port)
        .map(|port| shifted(port, offset))
        .transpose()
}

/// `DEVCLI_PORT` (else `PORT`) of the running process of `dep`, if any. The
/// process state is not created just to look.
fn running_port(dep: &ResolvedApp) -> Option<u16> {
    let dir = crate::config::loader::config_home().join("processes");
    if !dir.is_dir() {
        return None;
    }
    let store = process_manager::StateStore::new(dir).ok()?;
    let process =
        crate::process_manager_support::find_process(&store, &dep.project, &dep.app_name, None)
            .ok()??;
    if !store.is_running(&process) {
        return None;
    }
    let env = &process.task.env;
    env.get(DEVCLI_PORT)
        .or_else(|| env.get("PORT"))?
        .trim()
        .parse()
        .ok()
}

/// The `PORT` of the env files `app` would load when started in
/// `environment` (with its default stage there), unshifted.
fn env_file_port(app: &ResolvedApp, environment: &str) -> Option<u16> {
    let preferences = crate::config::load_preferences().ok();
    let stage = app.app.get_default_stage(
        environment,
        preferences
            .as_ref()
            .and_then(|p| p.default_stage.as_deref()),
    );
    let stage = stage.as_deref();
    let working_dir = crate::utils::path::expand_path(&app.app.path);
    // Its own overlay, removed after: the instance's may be in use.
    let overlay = crate::config::loader::config_home()
        .join("env-overlays")
        .join(format!(".lookup-{}", std::process::id()))
        .join(overlay_name(app));
    let port = env_dir_at(app, &working_dir, stage, environment, &overlay)
        .ok()
        .and_then(|(dir, _)| {
            crate::env_flow_support::load_process_runtime(
                &dir,
                app.app.env_files.as_ref(),
                stage,
                environment,
                app.app.dockerfile_path.as_deref(),
            )
            .ok()
            .flatten()
        })
        .and_then(|loaded| loaded.vars.get("PORT")?.trim().parse().ok());
    if let Some(lookup) = overlay.parent() {
        let _ = std::fs::remove_dir_all(lookup);
    }
    port
}

fn overlay_name(resolved: &ResolvedApp) -> String {
    super::registry::sanitize_slug(&format!("{}_{}", resolved.project, resolved.app_name))
}

/// The env files the cascade would load for an app at `root`, relative to
/// it: the app directory's layers, plus those of its Dockerfile directory.
fn cascade_files(
    root: &Path,
    stage: Option<&str>,
    environment: &str,
    dockerfile_path: Option<&str>,
) -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::new()];
    if let Some(parent) = dockerfile_path.and_then(|df| Path::new(df).parent())
        && !parent.as_os_str().is_empty()
    {
        roots.push(parent.to_path_buf());
    }
    let mut files = Vec::new();
    for prefix in roots {
        let layers = env_flow::EnvFlow::from_dir(root.join(&prefix))
            .stage_opt(crate::env_flow_support::effective_stage(stage))
            .context(crate::env_flow_support::context_from_env(environment))
            .layers();
        for layer in layers.into_iter().flatten() {
            let file = prefix.join(&layer.relative_path);
            if !files.contains(&file) {
                files.push(file);
            }
        }
    }
    files
}

/// The directory to read env files from.
///
/// Env files are usually gitignored, so a worktree lacks some or all of the
/// configured checkout's (`.env.local` with the secrets, while `.env` is
/// committed). Each file the cascade loads is taken from the worktree when it
/// has it, else from the configured checkout:
/// - the worktree has them all: the worktree
/// - it has none: the configured checkout
/// - a mix: an overlay directory of links to each file, so the cascade loads
///   (and interpolates) them as usual
///
/// A file pinned in `env_files` is looked up the same way. Files the cascade
/// does not load (`.env.example`, `.envrc`) play no part.
pub fn env_dir(
    resolved: &ResolvedApp,
    working_dir: &Path,
    stage: Option<&str>,
    environment: &str,
) -> Result<(PathBuf, Option<String>)> {
    let overlay = crate::config::loader::config_home()
        .join("env-overlays")
        .join(overlay_name(resolved));
    env_dir_at(resolved, working_dir, stage, environment, &overlay)
}

/// [`env_dir`], building any overlay at `overlay`.
fn env_dir_at(
    resolved: &ResolvedApp,
    working_dir: &Path,
    stage: Option<&str>,
    environment: &str,
    overlay: &Path,
) -> Result<(PathBuf, Option<String>)> {
    let Some(instance) = &resolved.worktree else {
        return Ok((working_dir.to_path_buf(), None));
    };
    let base = &instance.base_path;
    let pinned = crate::env_flow_support::resolve_config_env_path(
        resolved.app.env_files.as_ref(),
        stage,
        environment,
    )?;
    let files = match &pinned {
        Some(path) => vec![PathBuf::from(path)],
        None => cascade_files(
            base,
            stage,
            environment,
            resolved.app.dockerfile_path.as_deref(),
        ),
    };

    let from_base: Vec<&PathBuf> = files
        .iter()
        .filter(|f| base.join(f).is_file() && !working_dir.join(f).is_file())
        .collect();
    if from_base.is_empty() {
        return Ok((working_dir.to_path_buf(), None));
    }
    let names = from_base
        .iter()
        .map(|f| f.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let in_worktree = files.iter().any(|f| working_dir.join(f).is_file());
    if !in_worktree {
        let note = format!(
            "Env files: none in worktree '{}', using {} ({names})",
            instance.slug,
            base.display()
        );
        return Ok((base.clone(), Some(note)));
    }

    let _ = std::fs::remove_dir_all(overlay);
    for file in &files {
        let source = if working_dir.join(file).is_file() {
            working_dir.join(file)
        } else if base.join(file).is_file() {
            base.join(file)
        } else {
            continue;
        };
        let link = overlay.join(file);
        if let Some(parent) = link.parent() {
            std::fs::create_dir_all(parent)?;
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source, &link)?;
        #[cfg(not(unix))]
        std::fs::copy(&source, &link).map(|_| ())?;
    }
    let note = format!(
        "Env files: {names} from {} (missing in worktree '{}')",
        base.display(),
        instance.slug
    );
    Ok((overlay.to_path_buf(), Some(note)))
}

/// `DEVCLI_PORT_<APP>`: the app name upper-cased, other characters as `_`.
pub fn dependency_port_var(app: &str) -> String {
    let name: String = app
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("DEVCLI_PORT_{name}")
}

/// `${NAME}` replaced by the instance's variables (`DEVCLI_*`, `PORT`), so a
/// health check can target the port an instance actually uses.
fn substitute(text: &str, env_vars: &HashMap<String, String>) -> String {
    let mut out = text.to_string();
    for (name, value) in env_vars {
        if name.starts_with("DEVCLI_") || name == "PORT" {
            out = out.replace(&format!("${{{name}}}"), value);
        }
    }
    out
}

/// The health check of an instance: `${...}` variables substituted, and the
/// configured port replaced by the shifted one.
pub fn health_check(
    check: HealthCheck,
    applied: &Applied,
    env_vars: &HashMap<String, String>,
) -> HealthCheck {
    match check {
        HealthCheck::Http {
            url,
            timeout_secs,
            expected_status,
        } => {
            let mut url = substitute(&url, env_vars);
            if let Some(shift) = applied.shift
                && let Ok(mut parsed) = reqwest::Url::parse(&url)
                && parsed.port_or_known_default() == Some(shift.from)
                && parsed.set_port(Some(shift.to)).is_ok()
            {
                url = parsed.to_string();
            }
            HealthCheck::Http {
                url,
                timeout_secs,
                expected_status,
            }
        }
        HealthCheck::Tcp {
            host,
            port,
            timeout_secs,
        } => HealthCheck::Tcp {
            host: substitute(&host, env_vars),
            port: match applied.shift {
                Some(shift) if shift.from == port => shift.to,
                _ => port,
            },
            timeout_secs,
        },
        HealthCheck::Command {
            command,
            timeout_secs,
            expected_exit_code,
        } => HealthCheck::Command {
            command: substitute(&command, env_vars),
            timeout_secs,
            expected_exit_code,
        },
        HealthCheck::Process {} => HealthCheck::Process {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::AppBuilder;
    use crate::worktrees::Instance;
    use std::path::PathBuf;

    fn resolved(port: Option<u16>, slot: Option<u32>) -> ResolvedApp {
        let mut app = AppBuilder::new("nodejs", "/repo/api").build();
        app.port = port;
        ResolvedApp {
            project: slot.map_or("proj".into(), |_| "proj@feat".into()),
            app_name: "api".into(),
            app,
            worktree: slot.map(|index| Instance {
                slug: "feat".into(),
                index,
                base_project: "proj".into(),
                root: PathBuf::from("/w/feat"),
                base_root: PathBuf::from("/repo"),
                base_path: PathBuf::from("/repo/api"),
                common_dir: PathBuf::from("/repo/.git"),
                branch: Some("feat".into()),
            }),
        }
    }

    #[test]
    fn the_configured_checkout_keeps_its_ports() {
        let mut env = HashMap::from([("PORT".to_string(), "3000".to_string())]);
        let applied = apply(&resolved(None, None), "local", &mut env).unwrap();
        assert_eq!(env["PORT"], "3000");
        assert_eq!(env[WORKTREE], "");
        assert_eq!(env[PORT_OFFSET], "0");
        assert!(applied.shift.is_none());
        assert!(!env.contains_key(WORKTREE_PATH));

        let mut env = HashMap::new();
        apply(&resolved(Some(4000), None), "local", &mut env).unwrap();
        assert_eq!(
            (env["PORT"].as_str(), env[DEVCLI_PORT].as_str()),
            ("4000", "4000")
        );
    }

    #[test]
    fn a_worktree_shifts_the_declared_port() {
        let mut env = HashMap::new();
        let applied = apply(&resolved(Some(3000), Some(2)), "local", &mut env).unwrap();
        assert_eq!(env["PORT"], "3200");
        assert_eq!(env[DEVCLI_PORT], "3200");
        assert_eq!(env[WORKTREE], "feat");
        assert_eq!(env[WORKTREE_INDEX], "2");
        assert_eq!(env[PORT_OFFSET], "200");
        assert_eq!(env[WORKTREE_PATH], "/w/feat");
        assert_eq!(env[WORKTREE_BRANCH], "feat");
        assert_eq!(
            applied.shift,
            Some(PortShift {
                from: 3000,
                to: 3200
            })
        );
    }

    #[test]
    fn a_worktree_shifts_the_port_of_its_env_files() {
        let mut env = HashMap::from([("PORT".to_string(), "8080".to_string())]);
        apply(&resolved(None, Some(1)), "local", &mut env).unwrap();
        assert_eq!(env["PORT"], "8180");

        let mut env = HashMap::from([("PORT".to_string(), "65500".to_string())]);
        assert!(apply(&resolved(None, Some(1)), "local", &mut env).is_err());
    }

    #[test]
    fn a_worktree_gets_its_own_compose_project() {
        let mut env = HashMap::new();
        apply(&resolved(None, Some(1)), "docker-compose", &mut env).unwrap();
        assert_eq!(env["COMPOSE_PROJECT_NAME"], "proj-api-feat");

        let mut env = HashMap::from([("COMPOSE_PROJECT_NAME".into(), "mine".into())]);
        apply(&resolved(None, Some(1)), "docker-compose", &mut env).unwrap();
        assert_eq!(env["COMPOSE_PROJECT_NAME"], "mine");

        let mut env = HashMap::new();
        apply(&resolved(None, None), "docker-compose", &mut env).unwrap();
        assert!(!env.contains_key("COMPOSE_PROJECT_NAME"));
    }

    #[test]
    fn health_checks_follow_the_shifted_port() {
        let mut env = HashMap::new();
        let applied = apply(&resolved(Some(3000), Some(1)), "local", &mut env).unwrap();

        let http = health_check(
            HealthCheck::Http {
                url: "http://localhost:3000/health".into(),
                timeout_secs: 1,
                expected_status: 200,
            },
            &applied,
            &env,
        );
        assert!(
            matches!(&http, HealthCheck::Http { url, .. } if url == "http://localhost:3100/health"),
            "{http:?}"
        );

        let templated = health_check(
            HealthCheck::Http {
                url: "http://127.0.0.1:${DEVCLI_PORT}/ready".into(),
                timeout_secs: 1,
                expected_status: 200,
            },
            &applied,
            &env,
        );
        assert!(
            matches!(&templated, HealthCheck::Http { url, .. } if url == "http://127.0.0.1:3100/ready"),
            "{templated:?}"
        );

        let tcp = health_check(
            HealthCheck::Tcp {
                host: "localhost".into(),
                port: 3000,
                timeout_secs: 1,
            },
            &applied,
            &env,
        );
        assert!(matches!(tcp, HealthCheck::Tcp { port: 3100, .. }));

        // Another port (a database the app talks to) is left alone.
        let other = health_check(
            HealthCheck::Tcp {
                host: "localhost".into(),
                port: 5432,
                timeout_secs: 1,
            },
            &applied,
            &env,
        );
        assert!(matches!(other, HealthCheck::Tcp { port: 5432, .. }));
    }

    #[test]
    fn env_files_missing_in_a_worktree_come_from_the_configured_checkout() {
        let _env = crate::test_utils::TempConfigDir::new();
        let dir = tempfile::TempDir::new().unwrap();
        let base = dir.path().join("repo/api");
        let worktree = dir.path().join("w/feat/api");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(base.join(".env"), "A=main\nPORT=3000\n").unwrap();
        std::fs::write(base.join(".env.local"), "SECRET=main\n").unwrap();
        // Not loaded by the cascade: they must not count as "has env files".
        std::fs::write(worktree.join(".env.example"), "SECRET=\n").unwrap();
        std::fs::write(worktree.join(".envrc"), "use nix\n").unwrap();

        let mut app = resolved(None, Some(1));
        app.worktree.as_mut().unwrap().base_path = base.clone();

        // None in the worktree: the configured checkout's directory.
        let (used, note) = env_dir(&app, &worktree, None, "local").unwrap();
        assert_eq!(used, base);
        assert!(note.unwrap().contains("none in worktree 'feat'"));

        // `.env` committed, `.env.local` gitignored: a mix of both.
        std::fs::write(worktree.join(".env"), "A=branch\nPORT=3000\n").unwrap();
        let (used, note) = env_dir(&app, &worktree, None, "local").unwrap();
        assert_ne!(used, worktree);
        assert!(note.unwrap().contains(".env.local"));
        let loaded =
            crate::env_flow_support::load_process_runtime(&used, None, None, "local", None)
                .unwrap()
                .unwrap();
        assert_eq!(loaded.vars["A"], "branch", "the worktree's own file wins");
        assert_eq!(loaded.vars["SECRET"], "main");

        // All of them in the worktree: the worktree.
        std::fs::write(worktree.join(".env.local"), "SECRET=branch\n").unwrap();
        assert_eq!(env_dir(&app, &worktree, None, "local").unwrap().0, worktree);

        // A pinned file is looked up by name.
        app.app.env_files = Some(HashMap::from([(
            "base".to_string(),
            HashMap::from([("local".to_string(), "config/.env.dev".to_string())]),
        )]));
        std::fs::create_dir_all(base.join("config")).unwrap();
        std::fs::write(base.join("config/.env.dev"), "B=1\n").unwrap();
        assert_eq!(env_dir(&app, &worktree, None, "local").unwrap().0, base);

        // The configured checkout always reads its own.
        let main = resolved(None, None);
        assert_eq!(env_dir(&main, &base, None, "local").unwrap().0, base);
    }

    #[test]
    fn the_env_files_port_wins_over_the_declared_one() {
        let mut env = HashMap::from([("PORT".to_string(), "4000".to_string())]);
        apply(&resolved(Some(3000), None), "local", &mut env).unwrap();
        assert_eq!(
            env["PORT"], "4000",
            "the configured checkout keeps its env file's port"
        );
        assert_eq!(env[DEVCLI_PORT], "4000");

        let mut env = HashMap::from([("PORT".to_string(), "4000".to_string())]);
        apply(&resolved(Some(3000), Some(1)), "local", &mut env).unwrap();
        assert_eq!(env["PORT"], "4100");
    }

    /// `DEVCLI_PORT_API` used api's declared port while api listened on its
    /// env file's PORT (which wins).
    #[test]
    fn a_dependency_is_reached_on_its_env_files_port() {
        let _env = crate::test_utils::TempConfigDir::new();
        let dir = tempfile::TempDir::new().unwrap();
        let mut dep = resolved(Some(3000), Some(1));
        dep.app.path = dir.path().to_string_lossy().into_owned();
        assert_eq!(dependency_port(&dep, "local").unwrap(), Some(3100));

        std::fs::write(dir.path().join(".env"), "PORT=4000\n").unwrap();
        assert_eq!(dependency_port(&dep, "local").unwrap(), Some(4100));
        dep.worktree = None;
        assert_eq!(dependency_port(&dep, "local").unwrap(), Some(4000));
        assert!(
            !crate::config::loader::config_home()
                .join("env-overlays")
                .exists(),
            "looking left an overlay behind"
        );

        dep.app.port = None;
        std::fs::remove_file(dir.path().join(".env")).unwrap();
        assert_eq!(dependency_port(&dep, "local").unwrap(), None);
    }

    #[test]
    fn dependency_port_variables_are_named_after_the_app() {
        assert_eq!(dependency_port_var("api"), "DEVCLI_PORT_API");
        assert_eq!(
            dependency_port_var("auth-service.v2"),
            "DEVCLI_PORT_AUTH_SERVICE_V2"
        );
    }
}
