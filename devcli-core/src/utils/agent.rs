//! Is devcli being run by an AI coding agent?
//!
//! Agent harnesses mark the environment of the shell commands they run.
//! devcli uses that to refuse operations that reach beyond the apps an agent
//! works on (`stop --all`), which would take down what the user and other
//! agents are running.
//!
//! This is a guardrail against mistakes, not a security boundary: any
//! process can set or unset these variables, and an agent with a shell can
//! signal processes directly.

/// Variables naming the agent that set them. A value of `0`, `false`, `no`
/// or `off` does not count.
const MARKERS: &[(&str, &str)] = &[
    // Generic convention, set by several harnesses.
    ("AI_AGENT", "an AI agent"),
    // Set only on the commands Claude Code runs itself (v2.1.172+).
    ("CLAUDE_CODE_CHILD_SESSION", "Claude Code"),
    // Also set in the integrated terminal of its IDE extensions, where a
    // person types: they can set DEVCLI_AGENT=0 there.
    ("CLAUDECODE", "Claude Code"),
    ("CODEX_SANDBOX", "Codex"),
    ("CODEX_THREAD_ID", "Codex"),
    ("CODEX_CI", "Codex"),
    ("GEMINI_CLI", "Gemini CLI"),
    ("QWEN_CODE", "Qwen Code"),
    ("CURSOR_AGENT", "Cursor"),
    ("CURSOR_SANDBOX", "Cursor"),
    ("OPENCODE", "OpenCode"),
    ("PI_CODING_AGENT", "pi"),
    ("AMP_CURRENT_THREAD_ID", "Amp"),
    ("COPILOT_AGENT_SESSION_ID", "GitHub Copilot"),
    ("COPILOT_CLI", "GitHub Copilot"),
    ("CLINE_ACTIVE", "Cline"),
    ("WINDSURF_CASCADE_TERMINAL", "Windsurf"),
    ("KIRO_AGENT_PATH", "Kiro"),
    ("AUGMENT_AGENT", "Augment"),
];

/// Forces the answer: `1` (any true value) for a harness devcli does not
/// recognise, `0` to turn detection off.
pub const OVERRIDE: &str = "DEVCLI_AGENT";

fn is_false(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off"
    )
}

/// The agent running devcli, if any, from the given environment lookup.
pub fn detect_with(env: impl Fn(&str) -> Option<String>) -> Option<&'static str> {
    if let Some(value) = env(OVERRIDE) {
        return (!is_false(&value)).then_some("an AI agent");
    }
    MARKERS
        .iter()
        .find(|(name, _)| env(name).is_some_and(|v| !is_false(&v)))
        .map(|(_, agent)| *agent)
}

/// The agent running devcli, if any.
pub fn detect() -> Option<&'static str> {
    detect_with(|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn with(vars: &[(&str, &str)]) -> Option<&'static str> {
        let vars: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        detect_with(|name| vars.get(name).cloned())
    }

    #[test]
    fn agents_are_recognised_by_their_markers() {
        assert_eq!(with(&[]), None);
        assert_eq!(with(&[("CLAUDECODE", "1")]), Some("Claude Code"));
        assert_eq!(with(&[("CODEX_SANDBOX", "seatbelt")]), Some("Codex"));
        assert_eq!(with(&[("GEMINI_CLI", "1")]), Some("Gemini CLI"));
        assert_eq!(with(&[("PI_CODING_AGENT", "true")]), Some("pi"));
        assert_eq!(with(&[("AI_AGENT", "something-new")]), Some("an AI agent"));
    }

    #[test]
    fn false_values_do_not_count() {
        assert_eq!(with(&[("CLAUDECODE", "0")]), None);
        assert_eq!(with(&[("AI_AGENT", "false"), ("OPENCODE", "")]), None);
    }

    #[test]
    fn the_override_wins() {
        assert_eq!(with(&[(OVERRIDE, "1")]), Some("an AI agent"));
        assert_eq!(with(&[(OVERRIDE, "0"), ("CLAUDECODE", "1")]), None);
    }
}
