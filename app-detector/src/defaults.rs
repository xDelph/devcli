//! Choosing the default command of a detected app.

use std::collections::HashMap;

/// Command names never chosen as a default: the default is what `devcli start`
/// runs, and picking `kubectl delete` or `npm run deploy` there is destructive.
const NEVER_DEFAULT: &[&str] = &[
    "delete",
    "destroy",
    "uninstall",
    "remove",
    "rm",
    "purge",
    "prune",
    "down",
    "stop",
    "kill",
    "clean",
    "reset",
    "deploy",
    "publish",
    "release",
];

/// Choose the default command among `commands`.
///
/// The first name in `preferred` that exists wins; otherwise the alphabetically
/// first name that is not destructive. The fallback used to be
/// `HashMap::keys().next()` - an arbitrary entry, different from run to run.
pub fn pick_default_command(
    commands: Option<&HashMap<String, String>>,
    preferred: &[&str],
) -> Option<String> {
    let cmds = commands?;
    if let Some(name) = preferred.iter().find(|name| cmds.contains_key(**name)) {
        return Some((*name).to_string());
    }
    let mut names: Vec<&String> = cmds
        .keys()
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            !NEVER_DEFAULT.iter().any(|bad| {
                lower == *bad
                    || lower.starts_with(&format!("{bad}:"))
                    || lower.starts_with(&format!("{bad}-"))
            })
        })
        .collect();
    names.sort();
    names.first().map(|name| (*name).clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds(names: &[&str]) -> HashMap<String, String> {
        names
            .iter()
            .map(|n| (n.to_string(), format!("run {n}")))
            .collect()
    }

    #[test]
    fn preferred_name_wins() {
        let c = cmds(&["build", "dev", "start"]);
        assert_eq!(
            pick_default_command(Some(&c), &["serve", "start"]).as_deref(),
            Some("start")
        );
    }

    #[test]
    fn fallback_is_deterministic_and_never_destructive() {
        let c = cmds(&["delete", "deploy", "lint", "build", "stop"]);
        for _ in 0..20 {
            assert_eq!(
                pick_default_command(Some(&c), &["apply"]).as_deref(),
                Some("build")
            );
        }
        let only_destructive = cmds(&["delete", "uninstall", "deploy:prod"]);
        assert_eq!(pick_default_command(Some(&only_destructive), &[]), None);
    }
}
