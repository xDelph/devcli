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

use crate::config::{load_config, resolve_app};
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
    let resolved = resolve_app(&config, &args.app_name, args.project.as_deref())
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
        output::print_json(&LogsOutput {
            project: resolved.project,
            app: resolved.app_name,
            environment: args.environment,
            file: Some(path.display().to_string()),
            lines: tail,
        })?;
    } else {
        for line in &tail {
            println!("{line}");
        }
    }

    if args.follow {
        follow(&path, json)?;
    }

    Ok(())
}

/// Find the most recently modified log file for an app.
fn find_latest_log(project: &str, app: &str, environment: Option<&str>) -> Result<Option<PathBuf>> {
    let dir = logs_dir()?;
    let prefix = match environment {
        Some(env) => format!("{project}_{app}_{env}_"),
        None => format!("{project}_{app}_"),
    };

    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&dir).context("Failed to read log directory")? {
        let entry = entry.context("Failed to read log directory entry")?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with(&prefix) || !name.ends_with(".log") {
            continue;
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

    let mut buf = String::new();
    file.read_to_string(&mut buf)?;

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
fn follow(path: &Path, json: bool) -> Result<()> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open log file {}", path.display()))?;
    file.seek(SeekFrom::End(0))?;
    let mut reader = BufReader::new(file);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        if json {
            let value = serde_json::json!({ "line": line.trim_end_matches(['\n', '\r']) });
            writeln!(out, "{value}")?;
        } else {
            out.write_all(line.as_bytes())?;
        }
        out.flush()?;
    }
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
}
