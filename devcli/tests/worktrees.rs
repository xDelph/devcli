//! End-to-end tests of git worktree support, driven through the real binary
//! the way sub-agents use it: each one works in its own worktree and runs
//! plain `devcli` commands from there.
//!
//! Each test gets its own config directory, `HOME` and repository, and stops
//! everything it started.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// Prints its worktree variables, then stays up.
const APP: &str = "echo wt=$DEVCLI_WORKTREE index=$DEVCLI_WORKTREE_INDEX port=$PORT \
                   key=$API_KEY; sleep 30";

struct Sandbox {
    dir: tempfile::TempDir,
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

impl Sandbox {
    /// A repository with `api/` (port 3000, gitignored `.env`) and `web/`
    /// (depends on api), registered in devcli.
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("sandbox");
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let repo = root.join("repo");
        for app in ["api", "web"] {
            std::fs::create_dir_all(repo.join(app)).unwrap();
            std::fs::write(repo.join(app).join("main.txt"), app).unwrap();
        }
        std::fs::write(repo.join(".gitignore"), ".env\n").unwrap();
        std::fs::write(repo.join("api/.env"), "API_KEY=from-main\n").unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "init"]);

        let app = |path: PathBuf, deps: Value| {
            serde_json::json!({
                "type": "custom",
                "path": path,
                "port": 3000,
                "commands": { "local": { "start": format!("sh -c '{APP}'") } },
                "defaults": { "local": "start" },
                "dependencies": deps,
            })
        };
        let config = serde_json::json!({ "projects": { "shop": { "apps": {
            "api": app(repo.join("api"), serde_json::json!([])),
            "web": app(repo.join("web"), serde_json::json!([{ "project": "shop", "app": "api" }])),
        } } } });
        std::fs::create_dir_all(root.join("home")).unwrap();
        std::fs::write(
            root.join("home/config.json"),
            serde_json::to_vec_pretty(&config).unwrap(),
        )
        .unwrap();
        Self { dir }
    }

    fn root(&self) -> PathBuf {
        std::fs::canonicalize(self.dir.path()).unwrap()
    }

    fn repo(&self) -> PathBuf {
        self.root().join("repo")
    }

    /// `git worktree add` the way agent harnesses do, under the repository.
    fn add_worktree(&self, name: &str) -> PathBuf {
        let path = self.repo().join(".claude/worktrees").join(name);
        git(
            &self.repo(),
            &["worktree", "add", "-q", path.to_str().unwrap(), "-b", name],
        );
        path
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let home = self.root().join("home");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_devcli"));
        cmd.args(args)
            .current_dir(cwd)
            .env("DEVCLI_CONFIG_DIR", &home)
            .env("HOME", &home)
            .env("PATH", "/usr/bin:/bin")
            .env("NO_COLOR", "1")
            .env_remove("devcli_CONFIG_DIR")
            .env_remove("LOG_LEVEL");
        cmd
    }

    /// `devcli --json ARGS` run from `cwd`, which must succeed with one JSON
    /// value on stdout.
    fn json(&self, cwd: &Path, args: &[&str]) -> Value {
        let mut full = vec!["--json"];
        full.extend_from_slice(args);
        let out: Output = self.command(cwd, &full).output().expect("run devcli");
        assert!(
            out.status.success(),
            "devcli {full:?} in {}: {}",
            cwd.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "not one JSON value ({e}): {}",
                String::from_utf8_lossy(&out.stdout)
            )
        })
    }

    /// `(process id, worktree slug)` of every tracked process.
    fn processes(&self) -> Vec<(String, Option<String>)> {
        let out = self
            .command(&self.root(), &["--json", "status"])
            .output()
            .unwrap();
        let status: Value = serde_json::from_slice(&out.stdout).unwrap();
        let mut all: Vec<_> = status["processes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["id"].as_str().unwrap().to_string(),
                    p["worktree"]["slug"].as_str().map(str::to_string),
                )
            })
            .collect();
        all.sort();
        all
    }

    /// The first line the instance printed.
    fn first_log_line(&self, cwd: &Path, app: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let logs = self.json(cwd, &["logs", app, "-n", "50"]);
            if let Some(line) = logs["lines"].as_array().and_then(|l| l.first()) {
                return line.as_str().unwrap().to_string();
            }
            assert!(Instant::now() < deadline, "{app} printed nothing");
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = self
            .command(&self.root(), &["--json", "stop", "--all"])
            .output();
    }
}

#[test]
fn a_sub_agent_runs_its_worktrees_copy_without_touching_the_main_checkout() {
    let sb = Sandbox::new();
    let main = sb.repo();
    let agent = sb.add_worktree("agent-1");

    sb.json(&main, &["start", "api"]);
    let started = sb.json(&agent.join("api"), &["start", "api"]);
    assert_eq!(started["started"], serde_json::json!(["api@agent-1"]));

    assert_eq!(
        sb.processes(),
        vec![
            ("shop.api.local".into(), None),
            ("shop@agent-1.api.local".into(), Some("agent-1".into())),
        ]
    );

    // Its own port, its own variables, and the main checkout's gitignored
    // env file since the worktree has none.
    assert_eq!(
        sb.first_log_line(&main, "api"),
        "wt= index=0 port=3000 key=from-main"
    );
    assert_eq!(
        sb.first_log_line(&agent, "api"),
        "wt=agent-1 index=1 port=3100 key=from-main"
    );

    // From the worktree, `stop api` and `stop --project shop` only ever
    // reach the worktree's instance.
    let dry = sb.json(&agent, &["stop", "--project", "shop", "--dry-run"]);
    assert_eq!(dry["count"], 1);
    assert_eq!(dry["processes"][0]["project"], "shop@agent-1");
    sb.json(&agent, &["stop", "api"]);
    assert_eq!(sb.processes(), vec![("shop.api.local".into(), None)]);

    // And from the main checkout, `stop api` is the main checkout's.
    sb.json(&agent, &["start", "api"]);
    sb.json(&main, &["stop", "api"]);
    assert_eq!(
        sb.processes(),
        vec![("shop@agent-1.api.local".into(), Some("agent-1".into()))]
    );
}

#[test]
fn dependencies_start_in_the_same_worktree() {
    let sb = Sandbox::new();
    let agent = sb.add_worktree("agent-1");

    sb.json(&agent.join("web"), &["start", "web"]);
    assert_eq!(
        sb.processes(),
        vec![
            ("shop@agent-1.api.local".into(), Some("agent-1".into())),
            ("shop@agent-1.web.local".into(), Some("agent-1".into())),
        ]
    );
}

#[test]
fn every_instance_can_be_named_from_anywhere() {
    let sb = Sandbox::new();
    sb.add_worktree("agent-1");
    sb.add_worktree("agent-2");
    let elsewhere = sb.root();

    let started = sb.json(&elsewhere, &["start", "api@*"]);
    let mut names: Vec<_> = started["started"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(names, vec!["api", "api@agent-1", "api@agent-2"]);

    let listing = sb.json(&elsewhere, &["worktrees"]);
    let worktrees = listing["repositories"][0]["worktrees"].as_array().unwrap();
    let offsets: Vec<_> = worktrees.iter().map(|w| w["port_offset"].clone()).collect();
    assert_eq!(offsets, vec![0, 100, 200]);
    assert!(worktrees.iter().all(|w| w["apps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["app"] == "api" && a["running"] == Value::Bool(true))));

    sb.json(&elsewhere, &["stop", "api@agent-2"]);
    assert_eq!(sb.processes().len(), 2);
    let stopped = sb.json(&elsewhere, &["stop", "api@*"]);
    assert_eq!(stopped["stopped"], 2);

    // An unknown worktree is an error naming the ones that exist.
    let out = sb
        .command(&elsewhere, &["--json", "start", "api@nope"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err: Value = serde_json::from_slice(&out.stderr).unwrap();
    let message = err["error"].as_str().unwrap();
    assert!(message.contains("agent-1, agent-2"), "{message}");
}

#[test]
fn parallel_sub_agents_get_distinct_slots_and_ports() {
    let sb = Sandbox::new();
    let agents: Vec<PathBuf> = (1..=5)
        .map(|i| sb.add_worktree(&format!("agent-{i}")))
        .collect();

    // All at once, as an orchestrator fanning out sub-agents would.
    let children: Vec<_> = agents
        .iter()
        .map(|dir| {
            sb.command(&dir.join("api"), &["--json", "start", "api"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let mut ports: Vec<String> = agents
        .iter()
        .map(|dir| {
            let line = sb.first_log_line(dir, "api");
            line.split_whitespace()
                .find_map(|w| w.strip_prefix("port="))
                .unwrap()
                .to_string()
        })
        .collect();
    ports.sort();
    ports.dedup();
    assert_eq!(ports.len(), 5, "two agents share a port: {ports:?}");
}
