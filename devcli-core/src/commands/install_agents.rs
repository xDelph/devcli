// install-agents / uninstall-agents — tell AI coding agents that devcli exists
//
// Agents have no way to discover devcli on their own, so we write a block of
// rules into the instruction files they load. The block is fenced by markers so
// it can be refreshed or removed without touching a line the user wrote.
//
// Each agent reads its own files, so "AGENTS.md everywhere" is not enough:
//   - global (`--global`): the user-level file of every installed agent
//       Claude Code → ~/.claude/CLAUDE.md
//       Codex       → $CODEX_HOME/AGENTS.md (default ~/.codex; AGENTS.override.md wins)
//       Gemini CLI  → ~/.gemini/GEMINI.md
//     These load into every session of every project, so they get a short
//     block pointing at the full contract, written once to ~/.devcli/AGENTS.md.
//   - local (default, or `--local DIR`): DIR/AGENTS.md with the full contract
//     (read by Codex, Cursor, Copilot, and by Claude Code when the project has
//     no CLAUDE.md). When DIR has a CLAUDE.md, Claude Code ignores AGENTS.md,
//     so CLAUDE.md also gets an `@AGENTS.md` import.

use crate::config::loader::user_config_home;
use crate::Result;
use std::path::{Path, PathBuf};

/// The two lines fencing a block devcli owns.
#[derive(Clone, Copy)]
struct Markers {
    start: &'static str,
    end: &'static str,
}

/// The rules block. HTML comments: invisible once the markdown is rendered,
/// and stripped by Claude Code before the file reaches the model.
const RULES: Markers = Markers {
    start: "<!-- devcli:rules:start -->",
    end: "<!-- devcli:rules:end -->",
};

/// Markers written by earlier versions; recognised so those blocks are
/// upgraded or removed instead of duplicated.
const LEGACY_RULES: Markers = Markers {
    start: "# ===== XDELPH/DEVCLI rules start =====",
    end: "# ===== XDELPH/DEVCLI rules stop =====",
};

/// The `@AGENTS.md` import added to a project's CLAUDE.md.
const IMPORT: Markers = Markers {
    start: "<!-- devcli:import:start -->",
    end: "<!-- devcli:import:end -->",
};

const ALL_RULES: &[Markers] = &[RULES, LEGACY_RULES];

/// The full contract. Kept in the binary so `install-agents` works from any
/// install method (brew, install.sh, cargo) without shipping a side file.
const RULES_BODY: &str = include_str!("../../../AGENTS.md");

/// The short block for user-level files; `{reference}` is the full contract.
const GLOBAL_BODY: &str = "\
## devcli

`devcli` is installed. It starts, stops and inspects the local development apps
the user registered with it; prefer it over launching dev servers by hand.

- Always pass `--json` and check the exit code: stdout then holds exactly one
  JSON value, and errors are `{\"error\": \"...\"}` on stderr with exit code 1.
- Never rely on prompts: pass `--project`, `--env` and `--yes` explicitly.
- `devcli --json config list` lists the projects and apps devcli knows.
- `devcli --json status` shows what runs (exit code 1 when nothing does).
- `devcli --json start APP --project P` starts an app in the background;
  `devcli --json stop APP --project P` (or `stop --all`) stops it.
- `devcli --json logs APP --project P -n 200` returns its recent output.

Before scripting devcli further, read the full contract (every command, JSON
shape and exit code): {reference}";

/// Agents with a user-level instruction file devcli can write to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
    Gemini,
}

impl Agent {
    pub const ALL: [Agent; 3] = [Agent::Claude, Agent::Codex, Agent::Gemini];

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Gemini => "gemini",
        }
    }

    pub fn parse(name: &str) -> Result<Self> {
        Agent::ALL
            .into_iter()
            .find(|a| a.name() == name)
            .ok_or_else(|| {
                anyhow::anyhow!("Unknown agent '{name}' (expected claude, codex or gemini)")
            })
    }

    /// The agent's per-user directory.
    fn home(self, homes: &Homes) -> PathBuf {
        match self {
            Agent::Claude => homes.home.join(".claude"),
            Agent::Codex => homes
                .codex_home
                .clone()
                .unwrap_or_else(|| homes.home.join(".codex")),
            Agent::Gemini => homes.home.join(".gemini"),
        }
    }

    /// The user-level file the agent loads into every session.
    fn instructions_file(self, homes: &Homes) -> PathBuf {
        let dir = self.home(homes);
        match self {
            Agent::Claude => dir.join("CLAUDE.md"),
            Agent::Gemini => dir.join("GEMINI.md"),
            // Codex uses the first non-empty one of the two.
            Agent::Codex => {
                let override_file = dir.join("AGENTS.override.md");
                let in_use = std::fs::read_to_string(&override_file)
                    .is_ok_and(|content| !content.trim().is_empty());
                if in_use {
                    override_file
                } else {
                    dir.join("AGENTS.md")
                }
            }
        }
    }
}

/// Where the user-level files live, resolved once from the environment.
struct Homes {
    home: PathBuf,
    codex_home: Option<PathBuf>,
    /// devcli's own per-user directory (reference file, backups).
    devcli: PathBuf,
}

impl Homes {
    fn from_env() -> Result<Self> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        let codex_home = std::env::var("CODEX_HOME")
            .ok()
            .filter(|dir| !dir.trim().is_empty())
            .map(PathBuf::from);
        Ok(Self {
            home,
            codex_home,
            devcli: user_config_home(),
        })
    }

    fn reference_file(&self) -> PathBuf {
        self.devcli.join("AGENTS.md")
    }
}

/// Arguments for `install-agents`.
pub struct InstallAgentsArgs {
    /// Install into the user-level files of the agents (instead of a project).
    pub global: bool,
    /// Install into the given directory (default: the current directory).
    pub local: Option<String>,
    /// With `global`: only these agents (default: every agent found).
    pub agents: Vec<String>,
}

/// Arguments for `uninstall-agents`.
pub struct UninstallAgentsArgs {
    /// Remove from the user-level files of the agents.
    pub global: bool,
    /// Remove from the given directory (default: the current directory).
    pub local: Option<String>,
    /// With `global`: only these agents (default: all of them).
    pub agents: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Installed,
    Updated,
    Unchanged,
    Removed,
    /// There was nothing to remove.
    Absent,
}

impl Action {
    fn as_str(self) -> &'static str {
        match self {
            Action::Installed => "installed",
            Action::Updated => "updated",
            Action::Unchanged => "unchanged",
            Action::Removed => "removed",
            Action::Absent => "absent",
        }
    }
}

/// What happened to one file.
struct Outcome {
    /// The agent the file belongs to; `None` for a project's AGENTS.md.
    agent: Option<Agent>,
    path: PathBuf,
    action: Action,
    backup: Option<PathBuf>,
}

/// Locate the managed block in `content`, as a byte range that includes the
/// newline ending the block.
///
/// Errors on a damaged block: with a start marker but no end marker, guessing
/// where the block stops could overwrite the user's own text.
fn find_block(content: &str, path: &Path, kinds: &[Markers]) -> Result<Option<(usize, usize)>> {
    let mut found = None;
    for markers in kinds {
        let starts = content.matches(markers.start).count();
        let ends = content.matches(markers.end).count();
        if starts == 0 && ends == 0 {
            continue;
        }

        let start = content.find(markers.start);
        let end = content.find(markers.end);
        let well_formed = starts == 1
            && ends == 1
            && found.is_none()
            && matches!((start, end), (Some(s), Some(e)) if e > s);
        if !well_formed {
            anyhow::bail!(
                "{} has a damaged devcli block ({} start and {} end marker(s), \
                 or blocks of two versions); fix the markers by hand so no text \
                 of yours is overwritten",
                path.display(),
                starts,
                ends
            );
        }

        let (start, end) = (start.unwrap_or_default(), end.unwrap_or_default());
        let mut stop = end + markers.end.len();
        if content[stop..].starts_with('\n') {
            stop += 1;
        }
        found = Some((start, stop));
    }
    Ok(found)
}

/// `block` fenced by `markers`, ending with a newline.
fn fenced(markers: Markers, body: &str) -> String {
    format!("{}\n{}\n{}\n", markers.start, body.trim_end(), markers.end)
}

/// `content` with the block put in place: replaced where it already is,
/// appended after a blank line otherwise.
fn with_block(content: &str, path: &Path, kinds: &[Markers], block: &str) -> Result<String> {
    Ok(match find_block(content, path, kinds)? {
        Some((start, end)) => format!("{}{}{}", &content[..start], block, &content[end..]),
        None if content.trim().is_empty() => block.to_string(),
        None => format!("{}\n\n{}", content.trim_end(), block),
    })
}

/// `content` without the block (and the blank line `with_block` put before
/// it), or `None` when there is no block.
fn without_block(content: &str, path: &Path, kinds: &[Markers]) -> Result<Option<String>> {
    let Some((start, end)) = find_block(content, path, kinds)? else {
        return Ok(None);
    };
    let before = content[..start].trim_end_matches('\n');
    let after = content[end..].trim_start_matches('\n');
    Ok(Some(match (before.is_empty(), after.is_empty()) {
        (true, _) => after.to_string(),
        (false, true) => format!("{before}\n"),
        (false, false) => format!("{before}\n\n{after}"),
    }))
}

/// Current content of `path`, or `None` if it does not exist.
///
/// Any other failure is an error: treating an unreadable or non-UTF-8 file as
/// empty would replace the user's file with the devcli block.
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

/// Save the previous content of `path` under devcli's own directory, so a
/// backup never lands next to the file (and in the user's repository).
fn backup(homes: &Homes, path: &Path, previous: &str) -> Result<PathBuf> {
    let name = path
        .to_string_lossy()
        .trim_start_matches('/')
        .replace(['/', '\\', ':'], "__");
    let backup_path = homes.devcli.join("backups").join("agents").join(name);
    if let Some(parent) = backup_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&backup_path, previous)?;
    Ok(backup_path)
}

/// Write `content` to `path` (backing up what was there), or delete `path`
/// when `content` is empty.
fn replace_file(
    homes: &Homes,
    path: &Path,
    previous: Option<&str>,
    content: &str,
) -> Result<Option<PathBuf>> {
    let saved = previous.map(|p| backup(homes, path, p)).transpose()?;
    if content.is_empty() {
        std::fs::remove_file(path)?;
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        config_manager::utils::fs::write_atomic(path, content.as_bytes())?;
    }
    Ok(saved)
}

/// Put `block` into `path`, leaving everything else in the file alone.
fn install_block(
    homes: &Homes,
    agent: Option<Agent>,
    path: &Path,
    kinds: &[Markers],
    block: &str,
) -> Result<Outcome> {
    let previous = read_existing(path)?;
    let current = previous.as_deref().unwrap_or_default();
    let updated = with_block(current, path, kinds, block)?;

    let (action, backup) = if previous.as_deref() == Some(updated.as_str()) {
        (Action::Unchanged, None)
    } else {
        let had_block = find_block(current, path, kinds)?.is_some();
        let backup = replace_file(homes, path, previous.as_deref(), &updated)?;
        let action = if had_block {
            Action::Updated
        } else {
            Action::Installed
        };
        (action, backup)
    };
    Ok(Outcome {
        agent,
        path: path.to_path_buf(),
        action,
        backup,
    })
}

/// Take the block out of `path`; a file left empty is deleted.
fn remove_block(
    homes: &Homes,
    agent: Option<Agent>,
    path: &Path,
    kinds: &[Markers],
) -> Result<Outcome> {
    let mut outcome = Outcome {
        agent,
        path: path.to_path_buf(),
        action: Action::Absent,
        backup: None,
    };
    let Some(previous) = read_existing(path)? else {
        return Ok(outcome);
    };
    let Some(rest) = without_block(&previous, path, kinds)? else {
        return Ok(outcome);
    };
    outcome.backup = replace_file(homes, path, Some(&previous), &rest)?;
    outcome.action = Action::Removed;
    Ok(outcome)
}

/// The agents a global command acts on: the ones asked for, or by default
/// every agent whose directory exists (install) / all of them (uninstall).
fn selected_agents(names: &[String], homes: &Homes, installed_only: bool) -> Result<Vec<Agent>> {
    if !names.is_empty() {
        return names.iter().map(|n| Agent::parse(n)).collect();
    }
    if !installed_only {
        return Ok(Agent::ALL.to_vec());
    }
    let found: Vec<Agent> = Agent::ALL
        .into_iter()
        .filter(|a| a.home(homes).is_dir())
        .collect();
    if found.is_empty() {
        anyhow::bail!(
            "No supported agent found (looked for {}, {} and {}). \
             Pass --agent claude, codex or gemini to install anyway.",
            Agent::Claude.home(homes).display(),
            Agent::Codex.home(homes).display(),
            Agent::Gemini.home(homes).display()
        );
    }
    Ok(found)
}

/// Write the full contract to `path`, a file devcli owns as a whole.
fn write_reference(path: &Path) -> Result<Outcome> {
    let content = format!("{}\n", RULES_BODY.trim_end());
    let previous = read_existing(path)?;
    let action = match previous.as_deref() {
        Some(p) if p == content => Action::Unchanged,
        Some(_) => Action::Updated,
        None => Action::Installed,
    };
    if action != Action::Unchanged {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        config_manager::utils::fs::write_atomic(path, content.as_bytes())?;
    }
    Ok(Outcome {
        agent: None,
        path: path.to_path_buf(),
        action,
        backup: None,
    })
}

fn install_global(homes: &Homes, agents: &[String]) -> Result<Vec<Outcome>> {
    let agents = selected_agents(agents, homes, true)?;

    let reference = homes.reference_file();
    let mut outcomes = vec![write_reference(&reference)?];

    let body = GLOBAL_BODY.replace("{reference}", &reference.display().to_string());
    let block = fenced(RULES, &body);
    for agent in agents {
        let file = agent.instructions_file(homes);
        outcomes.push(install_block(homes, Some(agent), &file, ALL_RULES, &block)?);
    }
    Ok(outcomes)
}

fn uninstall_global(homes: &Homes, agents: &[String]) -> Result<Vec<Outcome>> {
    let mut outcomes = Vec::new();
    for agent in selected_agents(agents, homes, false)? {
        let file = agent.instructions_file(homes);
        outcomes.push(remove_block(homes, Some(agent), &file, ALL_RULES)?);
    }

    // Keep the reference while another agent's block still points at it.
    let still_used = Agent::ALL.into_iter().any(|agent| {
        read_existing(&agent.instructions_file(homes))
            .ok()
            .flatten()
            .is_some_and(|c| c.contains(RULES.start) || c.contains(LEGACY_RULES.start))
    });
    let reference = homes.reference_file();
    if !still_used && reference.exists() {
        std::fs::remove_file(&reference)?;
        outcomes.push(Outcome {
            agent: None,
            path: reference,
            action: Action::Removed,
            backup: None,
        });
    }
    Ok(outcomes)
}

/// The project's CLAUDE.md and the import line that reaches AGENTS.md from it.
fn claude_files(dir: &Path) -> [(PathBuf, &'static str); 2] {
    [
        (dir.join("CLAUDE.md"), "@AGENTS.md"),
        (dir.join(".claude").join("CLAUDE.md"), "@../AGENTS.md"),
    ]
}

/// Whether `content` already imports AGENTS.md outside devcli's own block.
fn imports_agents_md(content: &str, import: &str) -> bool {
    let alternative = import.replacen('@', "@./", 1);
    content
        .lines()
        .map(str::trim)
        .any(|line| line == import || line == alternative)
}

fn install_local(homes: &Homes, dir: &Path) -> Result<Vec<Outcome>> {
    let agents_md = dir.join("AGENTS.md");
    let mut outcomes = vec![install_block(
        homes,
        None,
        &agents_md,
        ALL_RULES,
        &fenced(RULES, &managed_body(RULES_BODY)),
    )?];

    // Claude Code ignores AGENTS.md as soon as the project has a CLAUDE.md.
    for (file, import) in claude_files(dir) {
        let Some(content) = read_existing(&file)? else {
            continue;
        };
        let ours = find_block(&content, &file, &[IMPORT])?.is_some();
        if !ours && imports_agents_md(&content, import) {
            continue;
        }
        outcomes.push(install_block(
            homes,
            Some(Agent::Claude),
            &file,
            &[IMPORT],
            &fenced(IMPORT, import),
        )?);
    }
    Ok(outcomes)
}

fn uninstall_local(homes: &Homes, dir: &Path) -> Result<Vec<Outcome>> {
    let mut outcomes = vec![remove_block(
        homes,
        None,
        &dir.join("AGENTS.md"),
        ALL_RULES,
    )?];
    for (file, _) in claude_files(dir) {
        let outcome = remove_block(homes, Some(Agent::Claude), &file, &[IMPORT])?;
        if outcome.action == Action::Removed {
            outcomes.push(outcome);
        }
    }
    Ok(outcomes)
}

/// The full contract as it appears inside a project's block.
///
/// Scrubbed of any literal marker: embedding one verbatim would nest blocks
/// and make `find_block` match the wrong offsets.
fn managed_body(body: &str) -> String {
    let mut body = format!(
        "<!-- Managed by `devcli install-agents`. Edit outside these markers; \
         edits inside are overwritten on update and removed on uninstall. -->\n\n{}",
        body.trim()
    );
    for markers in [RULES, LEGACY_RULES, IMPORT] {
        body = body
            .replace(markers.start, "(devcli marker)")
            .replace(markers.end, "(devcli marker)");
    }
    body
}

fn local_dir(local: Option<&str>) -> Result<PathBuf> {
    Ok(match local {
        Some(dir) => PathBuf::from(dir),
        None => std::env::current_dir()?,
    })
}

fn report(outcomes: &[Outcome], homes: &Homes, global: bool) -> Result<()> {
    if crate::output::json_enabled() {
        let targets: Vec<_> = outcomes
            .iter()
            .map(|o| {
                serde_json::json!({
                    "agent": o.agent.map(Agent::name),
                    "path": o.path.display().to_string(),
                    "action": o.action.as_str(),
                    "backup": o.backup.as_ref().map(|b| b.display().to_string()),
                })
            })
            .collect();
        let reference = global.then(|| homes.reference_file().display().to_string());
        return crate::output::print_json(&serde_json::json!({
            "targets": targets,
            "reference": reference,
        }));
    }

    for o in outcomes {
        let who = match o.agent {
            Some(agent) => agent.name(),
            None if o.path == homes.reference_file() => "reference",
            None => "project",
        };
        println!("{:<10} {:<10} {}", o.action.as_str(), who, o.path.display());
    }
    if let Some(b) = outcomes.iter().find_map(|o| o.backup.as_ref()) {
        println!(
            "Previous versions saved under {}",
            b.parent().unwrap_or(b).display()
        );
    }
    Ok(())
}

/// Install (or refresh) the devcli rules.
#[tracing::instrument(skip(args))]
pub async fn install_agents_command(args: &InstallAgentsArgs) -> Result<()> {
    let homes = Homes::from_env()?;
    let outcomes = if args.global {
        install_global(&homes, &args.agents)?
    } else {
        install_local(&homes, &local_dir(args.local.as_deref())?)?
    };
    report(&outcomes, &homes, args.global)
}

/// Remove the devcli rules, leaving the rest of each file untouched.
#[tracing::instrument(skip(args))]
pub async fn uninstall_agents_command(args: &UninstallAgentsArgs) -> Result<()> {
    let homes = Homes::from_env()?;
    let outcomes = if args.global {
        uninstall_global(&homes, &args.agents)?
    } else {
        uninstall_local(&homes, &local_dir(args.local.as_deref())?)?
    };
    report(&outcomes, &homes, args.global)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct Sandbox {
        _dir: TempDir,
        homes: Homes,
        project: PathBuf,
    }

    fn sandbox() -> Sandbox {
        let dir = TempDir::new().unwrap();
        let home = dir.path().join("home");
        let project = dir.path().join("project");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&project).unwrap();
        let homes = Homes {
            devcli: home.join(".devcli"),
            home,
            codex_home: None,
        };
        Sandbox {
            _dir: dir,
            homes,
            project,
        }
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn actions(outcomes: &[Outcome]) -> Vec<&'static str> {
        outcomes.iter().map(|o| o.action.as_str()).collect()
    }

    #[test]
    fn install_then_uninstall_restores_the_file_exactly() {
        let sb = sandbox();
        let file = sb.project.join("AGENTS.md");
        std::fs::write(&file, "# my notes\n").unwrap();

        install_local(&sb.homes, &sb.project).unwrap();
        let installed = read(&file);
        assert!(installed.starts_with("# my notes\n\n<!-- devcli:rules:start -->\n"));
        assert!(installed.ends_with("<!-- devcli:rules:end -->\n"));

        // Re-running changes nothing: no rewrite, no newline creeping in.
        let again = install_local(&sb.homes, &sb.project).unwrap();
        assert_eq!(actions(&again), vec!["unchanged"]);
        assert_eq!(read(&file), installed);

        uninstall_local(&sb.homes, &sb.project).unwrap();
        assert_eq!(read(&file), "# my notes\n");
    }

    #[test]
    fn a_file_created_by_install_is_deleted_by_uninstall() {
        let sb = sandbox();
        install_local(&sb.homes, &sb.project).unwrap();
        assert!(sb.project.join("AGENTS.md").exists());
        uninstall_local(&sb.homes, &sb.project).unwrap();
        assert!(!sb.project.join("AGENTS.md").exists());
    }

    #[test]
    fn backups_stay_out_of_the_project() {
        let sb = sandbox();
        std::fs::write(sb.project.join("AGENTS.md"), "# mine\n").unwrap();
        let outcomes = install_local(&sb.homes, &sb.project).unwrap();

        let backup = outcomes[0].backup.clone().expect("backup taken");
        assert!(backup.starts_with(&sb.homes.devcli));
        assert_eq!(read(&backup), "# mine\n");
        let names: Vec<_> = std::fs::read_dir(&sb.project)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("AGENTS.md")]);
    }

    #[test]
    fn a_legacy_block_is_upgraded_in_place() {
        let sb = sandbox();
        let file = sb.project.join("AGENTS.md");
        let legacy = format!(
            "# head\n\n{}\nold rules\n\n{}\n\n# foot\n",
            LEGACY_RULES.start, LEGACY_RULES.end
        );
        std::fs::write(&file, legacy).unwrap();

        let outcomes = install_local(&sb.homes, &sb.project).unwrap();
        assert_eq!(actions(&outcomes), vec!["updated"]);
        let content = read(&file);
        assert!(!content.contains(LEGACY_RULES.start));
        assert!(!content.contains("old rules"));
        assert_eq!(content.matches(RULES.start).count(), 1);
        assert!(content.starts_with("# head\n"));
        assert!(content.ends_with("# foot\n"));

        uninstall_local(&sb.homes, &sb.project).unwrap();
        assert_eq!(read(&file), "# head\n\n# foot\n");
    }

    #[test]
    fn a_claude_md_gets_an_import_of_agents_md() {
        let sb = sandbox();
        let claude = sb.project.join("CLAUDE.md");
        std::fs::write(&claude, "# Project rules\n").unwrap();

        let outcomes = install_local(&sb.homes, &sb.project).unwrap();
        assert_eq!(outcomes[1].agent, Some(Agent::Claude));
        assert_eq!(
            read(&claude),
            "# Project rules\n\n<!-- devcli:import:start -->\n@AGENTS.md\n<!-- devcli:import:end -->\n"
        );

        uninstall_local(&sb.homes, &sb.project).unwrap();
        assert_eq!(read(&claude), "# Project rules\n");
    }

    #[test]
    fn a_claude_md_that_already_imports_agents_md_is_left_alone() {
        let sb = sandbox();
        let claude = sb.project.join(".claude").join("CLAUDE.md");
        std::fs::create_dir_all(claude.parent().unwrap()).unwrap();
        std::fs::write(&claude, "@../AGENTS.md\n").unwrap();

        let outcomes = install_local(&sb.homes, &sb.project).unwrap();
        assert_eq!(outcomes.len(), 1, "only AGENTS.md is touched");
        assert_eq!(read(&claude), "@../AGENTS.md\n");
    }

    #[test]
    fn global_install_targets_the_files_each_agent_reads() {
        let sb = sandbox();
        std::fs::create_dir_all(sb.homes.home.join(".claude")).unwrap();
        std::fs::create_dir_all(sb.homes.home.join(".codex")).unwrap();
        std::fs::write(sb.homes.home.join(".claude/CLAUDE.md"), "# me\n").unwrap();

        let outcomes = install_global(&sb.homes, &[]).unwrap();
        let paths: Vec<_> = outcomes.iter().map(|o| o.path.clone()).collect();
        let reference = sb.homes.reference_file();
        assert_eq!(
            paths,
            vec![
                reference.clone(),
                sb.homes.home.join(".claude/CLAUDE.md"),
                sb.homes.home.join(".codex/AGENTS.md"),
            ],
            "no ~/.gemini: Gemini is skipped"
        );

        assert!(read(&reference).contains("Exit codes"));
        let claude = read(&sb.homes.home.join(".claude/CLAUDE.md"));
        assert!(claude.starts_with("# me\n\n"));
        assert!(claude.contains(&reference.display().to_string()));
        assert!(
            claude.len() < 2000,
            "the user-level block stays short: it loads in every session"
        );

        let again = install_global(&sb.homes, &[]).unwrap();
        assert_eq!(actions(&again), vec!["unchanged"; 3]);

        let removed = uninstall_global(&sb.homes, &[]).unwrap();
        assert!(removed.iter().any(|o| o.path == reference));
        assert_eq!(read(&sb.homes.home.join(".claude/CLAUDE.md")), "# me\n");
        assert!(!sb.homes.home.join(".codex/AGENTS.md").exists());
        assert!(!reference.exists());
    }

    #[test]
    fn global_install_without_any_agent_asks_for_one() {
        let sb = sandbox();
        let err = install_global(&sb.homes, &[])
            .err()
            .expect("no agent found");
        assert!(err.to_string().contains("--agent"));

        install_global(&sb.homes, &["gemini".to_string()]).unwrap();
        assert!(sb.homes.home.join(".gemini/GEMINI.md").exists());
        assert!(install_global(&sb.homes, &["cursor".to_string()]).is_err());
    }

    #[test]
    fn codex_override_file_is_used_when_it_has_content() {
        let mut sb = sandbox();
        let codex = sb.homes.home.join("elsewhere");
        std::fs::create_dir_all(&codex).unwrap();
        std::fs::write(codex.join("AGENTS.override.md"), "# override\n").unwrap();
        sb.homes.codex_home = Some(codex.clone());

        install_global(&sb.homes, &["codex".to_string()]).unwrap();
        assert!(read(&codex.join("AGENTS.override.md")).contains(RULES.start));
        assert!(!codex.join("AGENTS.md").exists());
    }

    #[test]
    fn uninstalling_one_agent_keeps_the_reference_for_the_others() {
        let sb = sandbox();
        install_global(&sb.homes, &["claude".into(), "gemini".into()]).unwrap();
        uninstall_global(&sb.homes, &["claude".into()]).unwrap();
        assert!(sb.homes.reference_file().exists());
        uninstall_global(&sb.homes, &["gemini".into()]).unwrap();
        assert!(!sb.homes.reference_file().exists());
    }

    #[test]
    fn body_mentions_json_and_exit_codes() {
        // The installed rules must actually teach agents the contract.
        assert!(RULES_BODY.contains("--json"));
        assert!(RULES_BODY.contains("Exit codes"));
    }

    #[test]
    fn embedded_body_never_contains_the_markers() {
        // AGENTS.md is embedded verbatim, so a literal marker in it would nest
        // blocks and break every offset.
        for markers in [RULES, LEGACY_RULES, IMPORT] {
            assert!(!RULES_BODY.contains(markers.start));
            assert!(!RULES_BODY.contains(markers.end));
        }
    }

    #[test]
    fn a_damaged_block_is_refused() {
        let path = Path::new("AGENTS.md");
        let start_only = format!("# mine\n{}\nold rules\n# my notes\n", RULES.start);
        assert!(find_block(&start_only, path, ALL_RULES).is_err());
        let end_first = format!("{}\n# mine\n{}\n", RULES.end, RULES.start);
        assert!(find_block(&end_first, path, ALL_RULES).is_err());
        let both_versions = format!(
            "{}\n{}\n{}\n{}\n",
            RULES.start, RULES.end, LEGACY_RULES.start, LEGACY_RULES.end
        );
        assert!(find_block(&both_versions, path, ALL_RULES).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn a_non_utf8_agents_md_is_left_alone() {
        let sb = sandbox();
        let file = sb.project.join("AGENTS.md");
        std::fs::write(&file, b"# notes \xff\xfe\n").unwrap();

        assert!(install_local(&sb.homes, &sb.project).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"# notes \xff\xfe\n");
    }
}
