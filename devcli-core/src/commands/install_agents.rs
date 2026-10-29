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
///
/// The body is scrubbed of any literal marker: the rules live in the repo's
/// AGENTS.md, and if that file ever contained a marker, embedding it verbatim
/// would nest blocks and make `find_block` match the wrong offsets.
fn render_block() -> String {
    let body = RULES_BODY
        .replace(MARKER_START, "(see marker definition above)")
        .replace(MARKER_STOP, "(see marker definition above)");
    format!(
        "{MARKER_START}\n\
         <!-- Managed by `devcli install-agents`. Edit outside these markers; \
         edits inside are overwritten on update and removed on uninstall. -->\n\
         \n\
         {body}\n\
         \n\
         {MARKER_STOP}\n"
    )
}

/// Locate the current block in `content`, as (start, end) byte offsets.
///
/// Errors on a damaged block: with a start marker but no stop marker, the old
/// code appended a second block, and the next run then treated everything from
/// the first start to the new stop - the user's own text included - as the
/// block to replace or remove.
fn find_block(content: &str, path: &Path) -> Result<Option<(usize, usize)>> {
    let starts = content.matches(MARKER_START).count();
    let stops = content.matches(MARKER_STOP).count();
    if starts == 0 && stops == 0 {
        return Ok(None);
    }

    let well_formed = starts == 1 && stops == 1 && {
        let start = content.find(MARKER_START).unwrap_or(usize::MAX);
        content.find(MARKER_STOP).is_some_and(|stop| stop > start)
    };
    if !well_formed {
        anyhow::bail!(
            "{} has {} devcli start and {} stop marker(s) instead of one block; \
             fix the markers by hand so no text of yours is overwritten",
            path.display(),
            starts,
            stops
        );
    }

    let start = content.find(MARKER_START).unwrap_or_default();
    let stop_at = content.find(MARKER_STOP).unwrap_or_default();
    Ok(Some((start, stop_at + MARKER_STOP.len())))
}

/// Current content of `path`, or `None` if it does not exist.
///
/// Any other failure is an error: falling back to "" for an unreadable or
/// non-UTF-8 AGENTS.md made `install-agents` replace the whole file with the
/// devcli block (and the backup failed the same way, so nothing was kept).
fn read_existing(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::anyhow!(
            "Cannot read {} ({e}); not modifying it",
            path.display()
        )),
    }
}

/// Write `content` to `path`, backing up the previous version first.
///
/// The backup holds the file as it was just before this run (it is replaced
/// on every run), and it is written before the file itself can be touched.
fn write_with_backup(
    path: &Path,
    previous: Option<&str>,
    content: &str,
) -> Result<Option<PathBuf>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let backup = match previous {
        Some(existing) => {
            let backup_path =
                path.with_extension(match path.extension().and_then(|e| e.to_str()) {
                    Some(ext) => format!("{ext}.backup"),
                    None => "backup".to_string(),
                });
            std::fs::write(&backup_path, existing)?;
            Some(backup_path)
        }
        None => None,
    };

    config_manager::utils::fs::write_atomic(path, content.as_bytes())?;
    Ok(backup)
}

/// Install (or refresh) the devcli rules block.
#[tracing::instrument(skip(args))]
pub async fn install_agents_command(args: &InstallAgentsArgs) -> Result<()> {
    let path = target_path(args.global, args.local.as_deref())?;
    let previous = read_existing(&path)?;
    let existing = previous.clone().unwrap_or_default();
    let block = render_block();

    let (updated, action) = match find_block(&existing, &path)? {
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

    let backup = write_with_backup(&path, previous.as_deref(), &updated)?;

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

    let existing = match read_existing(&path)? {
        Some(c) => c,
        None => {
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

    let (start, end) = match find_block(&existing, &path)? {
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

    let backup = write_with_backup(&path, Some(&existing), &rest)?;

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
        let (s, _e) = find_block(&first, Path::new("AGENTS.md"))
            .unwrap()
            .expect("block found");
        let second = format!("{}{}tail\n", &first[..s], render_block());
        let (s2, e2) = find_block(&second, Path::new("AGENTS.md"))
            .unwrap()
            .unwrap();
        // Replacement keeps one block only and preserves the user's content.
        assert_eq!(second.matches(MARKER_START).count(), 1);
        assert!(second.starts_with("# mine"));
        assert!(second[..s2].contains("# mine"));
        assert!(second[e2..].contains("tail"));
    }

    #[test]
    fn removal_preserves_user_content() {
        let content = format!("# head\n\n{}\n\n# foot\n", render_block());
        let (s, e) = find_block(&content, Path::new("AGENTS.md"))
            .unwrap()
            .unwrap();
        let rest = format!("{}{}", &content[..s], &content[e..]);
        assert!(rest.contains("# head"));
        assert!(rest.contains("# foot"));
        assert!(!rest.contains(MARKER_START));
        assert!(!rest.contains(MARKER_STOP));
    }

    #[test]
    fn no_block_is_a_noop() {
        assert!(find_block("# just my notes\n", Path::new("AGENTS.md"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn body_mentions_json_and_exit_codes() {
        // The installed rules must actually teach agents the contract.
        assert!(RULES_BODY.contains("--json"));
        assert!(RULES_BODY.contains("Exit codes"));
    }

    #[test]
    fn embedded_body_never_contains_the_markers() {
        // Regression guard: this file (AGENTS.md) is embedded verbatim, so a
        // literal marker in it would nest blocks and break every offset. The
        // AGENTS.md prose must therefore never write them out.
        assert!(
            !RULES_BODY.contains(MARKER_START),
            "AGENTS.md must not contain the start marker literally"
        );
        assert!(
            !RULES_BODY.contains(MARKER_STOP),
            "AGENTS.md must not contain the stop marker literally"
        );
    }

    #[test]
    fn rendered_block_contains_exactly_one_marker_pair() {
        let block = render_block();
        assert_eq!(block.matches(MARKER_START).count(), 1);
        assert_eq!(block.matches(MARKER_STOP).count(), 1);
    }

    #[test]
    fn a_damaged_block_is_refused() {
        let path = Path::new("AGENTS.md");
        let start_only = format!("# mine\n{MARKER_START}\nold rules\n# my notes\n");
        assert!(find_block(&start_only, path).is_err());
        let stop_first = format!("{MARKER_STOP}\n# mine\n{MARKER_START}\n");
        assert!(find_block(&stop_first, path).is_err());
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn a_non_utf8_agents_md_is_left_alone() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("AGENTS.md");
        std::fs::write(&file, b"# notes \xff\xfe\n").unwrap();

        let args = InstallAgentsArgs {
            global: false,
            local: Some(dir.path().to_string_lossy().into_owned()),
        };
        assert!(install_agents_command(&args).await.is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"# notes \xff\xfe\n");
    }
}
