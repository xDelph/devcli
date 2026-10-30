//! Logs command - read an app's captured stdout/stderr logs.
//!
//! Per-app logs are written by the process manager to
//! `~/.devcli/logs/{project}_{app}_{environment}_{YYYYMMDD}.log`.
//!
//! Designed for both humans (`devcli logs api`) and agents
//! (`devcli logs api --json --lines 100`).
//!
//! Example: `devcli logs api`
//! Example: `devcli logs api --project qm --env local -n 50`
//! Example: `devcli logs api --follow`
//! Example: `devcli logs api --json`

use crate::config::load_config;
use crate::output;
use crate::process_manager_support::logs_dir;
use crate::Result;
use anyhow::Context;
use serde::Serialize;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Arguments for the logs command
#[derive(Debug)]
pub struct LogsArgs {
    /// App name (or alternative name)
    pub app_name: String,
    /// Optional project to disambiguate the app name
    pub project: Option<String>,
    /// Optional environment filter (local, docker, orbstack, k8s)
    pub environment: Option<String>,
    /// Number of trailing lines to print
    pub lines: usize,
    /// Keep streaming new lines as they are written
    pub follow: bool,
}

#[derive(Serialize)]
struct LogsOutput {
    project: String,
    app: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    lines: Vec<String>,
}

/// Execute the logs command
#[tracing::instrument(skip(args), fields(app = %args.app_name, project = ?args.project, env = ?args.environment, lines = args.lines, follow = args.follow))]
pub async fn logs_command(args: LogsArgs) -> Result<()> {
    let config = load_config()?;
    let resolved = crate::worktrees::resolve(
        &config,
        &args.app_name,
        args.project.as_deref(),
        crate::worktrees::Mode::FromCwd,
    )
    .context("Failed to resolve app")?;

    let file = find_latest_log(
        &resolved.project,
        &resolved.app_name,
        args.environment.as_deref(),
    )?;

    let json = output::json_enabled();

    let Some(path) = file else {
        if json {
            output::print_json(&LogsOutput {
                project: resolved.project,
                app: resolved.app_name,
                environment: args.environment,
                file: None,
                lines: Vec::new(),
            })?;
        } else {
            println!(
                "No logs found for {}/{}",
                resolved.project, resolved.app_name
            );
        }
        return Ok(());
    };

    let tail = tail_lines(&path, args.lines)?;

    if json {
        // AGENTS.md documents `environment` on every successful answer; when the
        // caller did not filter, report the environment of the file we read.
        let environment = args
            .environment
            .clone()
            .or_else(|| environment_from_log_name(&path, &resolved.project, &resolved.app_name));
        output::print_json(&LogsOutput {
            project: resolved.project.clone(),
            app: resolved.app_name.clone(),
            environment,
            file: Some(path.display().to_string()),
            lines: tail,
        })?;
    } else {
        for line in &tail {
            println!("{line}");
        }
    }

    if args.follow {
        follow(
            &path,
            &resolved.project,
            &resolved.app_name,
            args.environment.as_deref(),
            json,
        )
        .await?;
    }

    Ok(())
}

/// Find the most recently modified log file for an app.
fn find_latest_log(project: &str, app: &str, environment: Option<&str>) -> Result<Option<PathBuf>> {
    let dir = logs_dir()?;

    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&dir).context("Failed to read log directory")? {
        let entry = entry.context("Failed to read log directory entry")?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        // Parse the name instead of matching a prefix: `proj_api_` is also a
        // prefix of app `api_worker`'s logs.
        match environment_from_log_name(&path, project, app) {
            Some(env) if environment.is_none_or(|wanted| wanted == env) => {}
            _ => continue,
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        candidates.push((modified, path));
    }

    candidates.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    Ok(candidates.into_iter().next().map(|(_, path)| path))
}

/// Environment part of `{project}_{app}_{env}_{YYYYMMDD}.log`, if the file is a
/// log of exactly `project/app`.
fn environment_from_log_name(path: &Path, project: &str, app: &str) -> Option<String> {
    crate::process_manager_support::LogFileName::from_path(path)
        .filter(|name| name.belongs_to(project, app))
        .map(|name| name.environment)
}

/// Read the last `n` lines of a file without loading it entirely.
fn tail_lines(path: &Path, n: usize) -> Result<Vec<String>> {
    if n == 0 {
        return Ok(Vec::new());
    }

    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open log file {}", path.display()))?;
    let len = file.metadata()?.len();

    // Read at most the last 1 MiB; enough for a useful tail of a daily log.
    const CHUNK: u64 = 1024 * 1024;
    let start = len.saturating_sub(CHUNK);
    file.seek(SeekFrom::Start(start))?;

    // Bytes, not a String: the 1 MiB window can start inside a multi-byte
    // character, and apps write whatever bytes they like. `read_to_string` made
    // the whole command fail on either.
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)?;
    let buf = String::from_utf8_lossy(&raw);

    let mut lines: Vec<String> = buf.lines().map(str::to_string).collect();
    // Drop a partial first line when we started mid-file.
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    if lines.len() > n {
        lines = lines.split_off(lines.len() - n);
    }
    Ok(lines)
}

/// Stream appended lines until the process is interrupted.
///
/// Keeps following across the two ways the file can change under us: a size
/// rotation truncates it in place (start again from the top), and a restart on
/// a later day writes to a new dated file (switch to it).
async fn follow(
    path: &Path,
    project: &str,
    app: &str,
    environment: Option<&str>,
    json: bool,
) -> Result<()> {
    const POLL: Duration = Duration::from_millis(250);
    const POLLS_BETWEEN_FILE_CHECKS: u32 = 8;

    let open = |path: &Path| -> Result<BufReader<std::fs::File>> {
        let file = std::fs::File::open(path)
            .with_context(|| format!("Failed to open log file {}", path.display()))?;
        Ok(BufReader::new(file))
    };

    let mut current = path.to_path_buf();
    let mut reader = open(&current)?;
    let mut position = reader.seek(SeekFrom::End(0))?;
    let mut pending: Vec<u8> = Vec::new();
    let mut idle_polls = 0u32;

    loop {
        let read = reader.read_until(b'\n', &mut pending)?;
        if read > 0 {
            position += read as u64;
            if pending.ends_with(b"\n") {
                emit_followed_line(&pending, json)?;
                pending.clear();
            }
            idle_polls = 0;
            continue;
        }

        let len = std::fs::metadata(&current).map(|m| m.len()).unwrap_or(0);
        if len < position {
            reader.seek(SeekFrom::Start(0))?;
            position = 0;
            pending.clear();
            continue;
        }

        idle_polls += 1;
        if idle_polls.is_multiple_of(POLLS_BETWEEN_FILE_CHECKS)
            && let Some(latest) = find_latest_log(project, app, environment)?
            && latest != current
        {
            reader = open(&latest)?;
            current = latest;
            position = 0;
            pending.clear();
            continue;
        }

        tokio::time::sleep(POLL).await;
    }
}

fn emit_followed_line(raw: &[u8], json: bool) -> Result<()> {
    let line = String::from_utf8_lossy(raw);
    let line = line.trim_end_matches(['\n', '\r']);
    if json {
        output::print_json_line(&serde_json::json!({ "line": line }))?;
    } else {
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        writeln!(out, "{line}")?;
        out.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tail_lines_returns_last_n() {
        let dir = std::env::temp_dir().join(format!("devcli-logs-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("proj_app_local_20240101.log");
        std::fs::write(&path, "a\nb\nc\nd\n").unwrap();

        let lines = tail_lines(&path, 2).unwrap();
        assert_eq!(lines, vec!["c".to_string(), "d".to_string()]);

        let all = tail_lines(&path, 100).unwrap();
        assert_eq!(all.len(), 4);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_environment_from_log_name() {
        let env = |name: &str| environment_from_log_name(Path::new(name), "proj", "api");
        assert_eq!(env("proj_api_local_20240101.log").as_deref(), Some("local"));
        assert_eq!(
            env("proj_api_docker-compose_20240101.log").as_deref(),
            Some("docker-compose")
        );
        assert_eq!(env("other_api_local_20240101.log"), None);
        assert_eq!(env("proj_api_local.log"), None);
        // Another app whose name starts with `api_`.
        assert_eq!(env("proj_api_worker_local_20240101.log"), None);
    }

    #[test]
    fn test_tail_lines_survives_invalid_utf8() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("proj_app_local_20240101.log");
        std::fs::write(&path, b"ok\nbad \xff byte\nlast\n").unwrap();

        let lines = tail_lines(&path, 10).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[2], "last");
    }
}
