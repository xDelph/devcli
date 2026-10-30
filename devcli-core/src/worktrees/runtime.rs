//! What an instance's process gets so several worktrees can run the same app
//! side by side: environment variables, shifted ports, its own Docker Compose
//! project, health checks on the shifted ports.

use crate::config::resolver::ResolvedApp;
use crate::Result;
use process_manager::HealthCheck;
use std::collections::HashMap;

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
/// - With `port` declared: `PORT` and `DEVCLI_PORT`, shifted by the offset.
///   Without it, in a worktree, a numeric `PORT` from the env files is
///   shifted, since the copies would otherwise all bind the same port.
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

    let declared = resolved.app.port;
    let from_env = || {
        env_vars
            .get("PORT")
            .and_then(|p| p.trim().parse::<u16>().ok())
    };
    let base_port = match (declared, instance) {
        (Some(port), _) => Some(port),
        (None, Some(_)) => from_env(),
        (None, None) => None,
    };
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
}
