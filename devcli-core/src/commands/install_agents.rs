// install-agents / uninstall-agents — manage the devcli rules block in AGENTS.md
//
// Agents have no way to discover devcli on their own, so we drop a block of
// rules into the agent-standard `AGENTS.md` file. The block is fenced by
// explicit markers so it can be refreshed or removed without touching a single
// line the user wrote themselves.
//
// `AGENTS.md` is the de-facto standard (agents.md, read natively by Claude
// Code, Cursor, Codex, Copilot, Gemini, Windsurf, Zed, Amp, JetBrains, Aider):
//   - local  → <project>/AGENTS.md
//   - global → ~/.agents/AGENTS.md

use crate::Result;
use std::path::{Path, PathBuf};

/// Opening marker of the managed block. Everything between the two markers is
/// owned by devcli; everything around it belongs to the user.
pub const MARKER_START: &str = "# ===== XDELPH/DEVCLI rules start =====";
/// Closing marker of the managed block.
pub const MARKER_STOP: &str = "# ===== XDELPH/DEVCLI rules stop =====";

/// The rules themselves. Kept in the binary so `install-agents` works from any
/// install method (brew, install.sh, cargo) without shipping a side file.
const RULES_BODY: &str = include_str!("../../../AGENTS.md");

/// Arguments for `install-agents`.
pub struct InstallAgentsArgs {
    /// Install for the current user (~/.agents/AGENTS.md).
    pub global: bool,
    /// Install into the given directory's AGENTS.md.
    pub local: Option<String>,
}

/// Arguments for `uninstall-agents`.
pub struct UninstallAgentsArgs {
    /// Remove from the current user's AGENTS.md.
    pub global: bool,
    /// Remove from the given directory's AGENTS.md.
    pub local: Option<String>,
}

/// Resolve where the AGENTS.md lives for the given mode.
fn target_path(global: bool, local: Option<&str>) -> Result<PathBuf> {
    if let Some(dir) = local {
        return Ok(Path::new(dir).join("AGENTS.md"));
    }
    if global {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        return Ok(home.join(".agents").join("AGENTS.md"));
    }
    // Neither flag: default to the current directory.
    Ok(std::env::current_dir()?.join("AGENTS.md"))
}

/// Render the managed block, header included.
fn render_block() -> String {
    format!(
        "{MARKER_START}\n\
         <!-- Managed by `devcli install-agents`. Edit outside these markers; \
         edits inside are overwritten on update and removed on uninstall. -->\n\
         \n\
         {RULES_BODY}\n\
         \n\
         {MARKER_STOP}\n"
    )
}

/// Locate the current block in `content`, as (start, end) byte offsets.
fn find_block(content: &str) -> Option<(usize, usize)> {
    let start = content.find(MARKER_START)?;
    let stop_at = content[start..].find(MARKER_STOP)? + start;
    let end = stop_at + MARKER_STOP.len();
    Some((start, end))
}

/// Write `content` to `path`, backing up any previous file first.
///
/// The backup is the *previous* content, so it always represents "before devcli
/// touched this", and it is written before the write can fail.
fn write_with_backup(path: &Path, content: &str) -> Result<Option<PathBuf>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let backup = match std::fs::read_to_string(path) {
        Ok(existing) => {
            let backup_path =
                path.with_extension(match path.extension().and_then(|e| e.to_str()) {
                    Some(ext) => format!("{ext}.backup"),
                    None => "backup".to_string(),
                });
            std::fs::write(&backup_path, &existing)?;
            Some(backup_path)
        }
        Err(_) => None,
    };

    std::fs::write(path, content)?;
    Ok(backup)
}

/// Install (or refresh) the devcli rules block.
#[tracing::instrument(skip(args))]
pub async fn install_agents_command(args: &InstallAgentsArgs) -> Result<()> {
    let path = target_path(args.global, args.local.as_deref())?;
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let block = render_block();

    let (updated, action) = match find_block(&existing) {
        Some((start, end)) => {
            tracing::info!("refreshing existing devcli block");
            (
                format!("{}{}{}", &existing[..start], block, &existing[end..]),
                "updated",
            )
        }
        None => {
            tracing::info!("installing a new devcli block");
            let content = if existing.trim().is_empty() {
                block
            } else {
                format!("{}\n{}", existing.trim_end(), block)
            };
            (content, "installed")
        }
    };

    let backup = write_with_backup(&path, &updated)?;

    if crate::output::json_enabled() {
        crate::output::print_json(&serde_json::json!({
            "action": action,
            "path": path.display().to_string(),
            "backup": backup.map(|b| b.display().to_string()),
            "agents": "AGENTS.md (Claude Code, Cursor, Codex, Copilot, Gemini, Windsurf, Zed, Amp, JetBrains, Aider)",
        }))?;
    } else {
        println!("devcli rules installed in {}", path.display());
        println!("Agents reading this file will now know devcli exists.");
        if let Some(b) = backup {
            println!("Previous file backed up to {}", b.display());
        }
    }
    Ok(())
}

/// Remove the devcli rules block, leaving the rest of the file untouched.
#[tracing::instrument(skip(args))]
pub async fn uninstall_agents_command(args: &UninstallAgentsArgs) -> Result<()> {
    let path = target_path(args.global, args.local.as_deref())?;

    let existing = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            if crate::output::json_enabled() {
                crate::output::print_json(&serde_json::json!({
                    "action": "noop", "path": path.display().to_string(),
                }))?;
            } else {
                println!("No AGENTS.md at {} — nothing to remove.", path.display());
            }
            return Ok(());
        }
    };

    let (start, end) = match find_block(&existing) {
        Some(b) => b,
        None => {
            if crate::output::json_enabled() {
                crate::output::print_json(&serde_json::json!({
                    "action": "noop", "path": path.display().to_string(),
                }))?;
            } else {
                println!(
                    "No devcli block found in {} — nothing to remove.",
                    path.display()
                );
            }
            return Ok(());
        }
    };

    // Swallow the blank line that followed the block so removal is clean.
    let mut rest = format!("{}{}", &existing[..start], &existing[end..]);
    rest = rest.trim_start_matches('\n').to_string();

    let backup = write_with_backup(&path, &rest)?;

    if crate::output::json_enabled() {
        crate::output::print_json(&serde_json::json!({
            "action": "removed",
            "path": path.display().to_string(),
            "backup": backup.map(|b| b.display().to_string()),
        }))?;
    } else {
        println!("devcli rules removed from {}", path.display());
        if let Some(b) = backup {
            println!("Previous file backed up to {}", b.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_is_detected_and_replaced() {
        let first = format!("# mine\n\n{}\nrest\n", render_block());
        let (s, _e) = find_block(&first).expect("block found");
        let second = format!("{}{}tail\n", &first[..s], render_block());
        let (s2, e2) = find_block(&second).unwrap();
        // Replacement keeps one block only and preserves the user's content.
        assert_eq!(second.matches(MARKER_START).count(), 1);
        assert!(second.starts_with("# mine"));
        assert!(second[..s2].contains("# mine"));
        assert!(second[e2..].contains("tail"));
    }

    #[test]
    fn removal_preserves_user_content() {
        let content = format!("# head\n\n{}\n\n# foot\n", render_block());
        let (s, e) = find_block(&content).unwrap();
        let rest = format!("{}{}", &content[..s], &content[e..]);
        assert!(rest.contains("# head"));
        assert!(rest.contains("# foot"));
        assert!(!rest.contains(MARKER_START));
        assert!(!rest.contains(MARKER_STOP));
    }

    #[test]
    fn no_block_is_a_noop() {
        assert!(find_block("# just my notes\n").is_none());
    }

    #[test]
    fn body_mentions_json_and_exit_codes() {
        // The installed rules must actually teach agents the contract.
        assert!(RULES_BODY.contains("--json"));
        assert!(RULES_BODY.contains("Exit codes"));
    }
}
