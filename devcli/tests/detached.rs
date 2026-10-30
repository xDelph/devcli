//! End-to-end tests of app lifetimes (detached and attached `start`) and of
//! `--json` output, driven through the real binary.
//!
//! Each test gets its own config directory (and `HOME`), so nothing touches
//! `~/.devcli`. `PATH` is reduced to the system directories so an installed
//! `pm-daemon` is never picked up; a freshly built one next to the test binary
//! may still be, and exits on its own once `stop --all` empties the state.

use serde_json::Value;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const TICKER: &str = "i=0; while true; do echo tick $i; i=$((i+1)); sleep 0.1; done";

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("create sandbox dir");
        let config = serde_json::json!({
            "projects": { "smoke": { "apps": { "ticker": {
                "type": "custom",
                "path": dir.path(),
                "commands": { "local": {
                    "start": format!("sh -c '{TICKER}'"),
                    "once": "echo hello from once"
                } },
                "defaults": { "local": "start" }
            }, "web": {
                "type": "custom",
                "path": dir.path(),
                "commands": { "local": { "start": format!("sh -c '{TICKER}'") } },
                "defaults": { "local": "start" },
                "dependencies": [{ "project": "smoke", "app": "ticker" }]
            } } } }
        });
        std::fs::write(
            dir.path().join("config.json"),
            serde_json::to_vec_pretty(&config).unwrap(),
        )
        .expect("write config");
        Self { dir }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_devcli"));
        cmd.args(args)
            .env("DEVCLI_CONFIG_DIR", self.dir.path())
            .env("HOME", self.dir.path())
            .env("PATH", "/usr/bin:/bin")
            .env("NO_COLOR", "1")
            .env_remove("devcli_CONFIG_DIR")
            .env_remove("LOG_LEVEL")
            .current_dir(self.dir.path());
        cmd
    }

    fn devcli(&self, args: &[&str]) -> Output {
        self.command(args).output().expect("run devcli")
    }

    /// Start an attached `devcli start ARGS` in the background; its output
    /// goes to `attached.out` in the sandbox.
    fn spawn_attached(&self, args: &[&str]) -> Child {
        let out = std::fs::File::create(self.dir.path().join("attached.out")).unwrap();
        let mut full = vec!["start"];
        full.extend_from_slice(args);
        self.command(&full)
            .stdin(Stdio::null())
            .stdout(out.try_clone().unwrap())
            .stderr(out)
            .spawn()
            .expect("spawn attached devcli start")
    }

    fn attached_output(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("attached.out")).unwrap_or_default()
    }

    /// `(app, pid)` of every running process, per `status`.
    fn running(&self) -> Vec<(String, u32)> {
        let out = self.devcli(&["--json", "status"]);
        let status = single_json(&out.stdout, &["--json", "status"]);
        status["processes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["running"] == Value::Bool(true))
            .map(|p| {
                (
                    p["app"].as_str().unwrap_or_default().to_string(),
                    p["pid"].as_u64().unwrap_or_default() as u32,
                )
            })
            .collect()
    }

    /// Run `devcli --json ARGS`, require success, and parse stdout as exactly
    /// one JSON value (anything else on stdout makes `from_slice` fail).
    fn json(&self, args: &[&str]) -> Value {
        let mut full = vec!["--json"];
        full.extend_from_slice(args);
        let out = self.devcli(&full);
        assert!(
            out.status.success(),
            "devcli {full:?} failed: {}\nstdout: {}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        );
        single_json(&out.stdout, &full)
    }

    fn log_lines(&self) -> usize {
        let logs = self.json(&["logs", "ticker", "--project", "smoke", "-n", "100000"]);
        logs["lines"]
            .as_array()
            .map(|l| {
                l.iter()
                    .filter(|s| s.as_str().unwrap_or("").starts_with("tick"))
                    .count()
            })
            .unwrap_or(0)
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // Never leave the ticker running, even when an assertion failed.
        let _ = self.devcli(&["--json", "stop", "--all"]);
    }
}

fn single_json(stdout: &[u8], args: &[&str]) -> Value {
    serde_json::from_slice(stdout).unwrap_or_else(|e| {
        panic!(
            "stdout of devcli {args:?} is not a single JSON value ({e}):\n{}",
            String::from_utf8_lossy(stdout)
        )
    })
}

fn pid_alive(pid: u32) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn send_signal(child: &Child, signal: &str) {
    let status = Command::new("/bin/kill")
        .args([signal, &child.id().to_string()])
        .status()
        .expect("run kill");
    assert!(status.success(), "kill {signal} failed");
}

/// Wait for `child` to exit, killing it if it takes longer than `timeout`.
fn wait_exit(child: &mut Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

/// Poll `check` until it holds or `timeout` expires.
fn wait_for(timeout: Duration, mut check: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    check()
}

#[test]
fn detached_app_keeps_logging_after_the_cli_exits() {
    let sb = Sandbox::new();

    let started = sb.json(&["start", "ticker", "--project", "smoke"]);
    assert_eq!(started["started"], serde_json::json!(["ticker"]));
    assert_eq!(started["detached"], Value::Bool(true));

    // The `start` process has exited; the app must still be running and its
    // output must keep reaching the log file.
    assert!(
        wait_for(Duration::from_secs(10), || sb.log_lines() >= 3),
        "no output captured after start; files: {:?}",
        std::fs::read_dir(sb.path().join("logs")).map(|d| d.count())
    );
    let before = sb.log_lines();
    assert!(
        wait_for(Duration::from_secs(10), || sb.log_lines() > before + 3),
        "log stopped growing after the CLI exited (stuck at {before} lines)"
    );

    let status = sb.json(&["status"]);
    let processes = status["processes"].as_array().expect("processes array");
    assert_eq!(processes.len(), 1, "status: {status}");
    assert_eq!(processes[0]["app"], "ticker");
    assert_eq!(processes[0]["running"], Value::Bool(true));

    let stopped = sb.json(&["stop", "--all"]);
    assert_eq!(stopped["stopped"], 1, "stop: {stopped}");
    assert_eq!(stopped["failed"], 0, "stop: {stopped}");

    // Nothing runs any more: `status` exits 1, still with JSON on stdout.
    let out = sb.devcli(&["--json", "status"]);
    assert_eq!(out.status.code(), Some(1));
    single_json(&out.stdout, &["--json", "status"]);
}

#[test]
fn json_stdout_is_a_single_value_for_restart_and_run() {
    let sb = Sandbox::new();

    sb.json(&["start", "ticker", "--project", "smoke"]);
    let restarted = sb.json(&["restart", "ticker", "--project", "smoke"]);
    assert_eq!(
        restarted["detached"],
        Value::Bool(true),
        "restart: {restarted}"
    );

    let dry = sb.json(&["stop", "--all", "--dry-run"]);
    assert_eq!(dry["dry_run"], Value::Bool(true));
    assert_eq!(dry["count"], 1, "dry-run: {dry}");

    let stopped = sb.json(&["stop", "ticker", "--project", "smoke"]);
    assert_eq!(stopped["stopped"], 1, "stop: {stopped}");

    let run = sb.json(&["run", "ticker", "once", "--project", "smoke"]);
    assert_eq!(run["command_variant"], "once");
    let log_file = run["log_file"].as_str().expect("log_file").to_string();
    assert!(
        wait_for(Duration::from_secs(10), || {
            std::fs::read_to_string(&log_file)
                .map(|s| s.contains("hello from once"))
                .unwrap_or(false)
        }),
        "`run` output never reached {log_file}"
    );
}

#[test]
fn usage_errors_are_json_on_stderr() {
    let sb = Sandbox::new();
    let out = sb.devcli(&["--json", "no-such-command"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        out.stdout.is_empty(),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let err = single_json(&out.stderr, &["--json", "no-such-command"]);
    assert!(err["error"].is_string(), "stderr: {err}");
}

/// Without `--detached`, `start` behaves like `docker compose up`: Ctrl+C
/// stops the app and the dependency it started for it.
#[test]
fn attached_start_stops_its_apps_and_dependencies_on_ctrl_c() {
    let sb = Sandbox::new();
    let mut attached = sb.spawn_attached(&["web", "--project", "smoke"]);

    assert!(
        wait_for(Duration::from_secs(15), || sb.running().len() == 2),
        "web and its dependency never both ran: {:?}\n{}",
        sb.running(),
        sb.attached_output()
    );
    let pids: Vec<u32> = sb.running().into_iter().map(|(_, pid)| pid).collect();

    send_signal(&attached, "-INT");
    let status = wait_exit(&mut attached, Duration::from_secs(20));
    assert!(
        status.is_some_and(|s| s.success()),
        "attached start did not exit cleanly ({status:?}):\n{}",
        sb.attached_output()
    );

    assert_eq!(sb.running(), vec![], "apps left running after Ctrl+C");
    for pid in pids {
        assert!(!pid_alive(pid), "process {pid} survived Ctrl+C");
    }
}

/// An attached session only stops what it started: a dependency that was
/// already running before it keeps running.
#[test]
fn attached_start_leaves_apps_it_did_not_start() {
    let sb = Sandbox::new();
    sb.json(&["start", "ticker", "--project", "smoke"]);
    assert!(wait_for(Duration::from_secs(10), || sb.running().len() == 1));

    let mut attached = sb.spawn_attached(&["web", "--project", "smoke"]);
    assert!(
        wait_for(Duration::from_secs(15), || sb.running().len() == 2),
        "web never started: {:?}\n{}",
        sb.running(),
        sb.attached_output()
    );

    send_signal(&attached, "-TERM");
    let status = wait_exit(&mut attached, Duration::from_secs(20));
    assert!(
        status.is_some_and(|s| s.success()),
        "attached start did not exit cleanly ({status:?}):\n{}",
        sb.attached_output()
    );

    let left: Vec<String> = sb.running().into_iter().map(|(app, _)| app).collect();
    assert_eq!(left, vec!["ticker".to_string()]);
}

/// `restart --detached` returns and leaves the new instance running, even
/// without `--json`.
#[test]
fn restart_detached_returns_and_keeps_the_app_running() {
    let sb = Sandbox::new();
    sb.json(&["start", "ticker", "--project", "smoke"]);
    assert!(wait_for(Duration::from_secs(10), || sb.running().len() == 1));
    let old_pid = sb.running()[0].1;

    let mut restart = sb
        .command(&["restart", "ticker", "--project", "smoke", "--detached"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn restart");
    let status = wait_exit(&mut restart, Duration::from_secs(20));
    assert!(
        status.is_some_and(|s| s.success()),
        "restart --detached did not return ({status:?})"
    );

    let running = sb.running();
    assert_eq!(running.len(), 1, "running: {running:?}");
    assert_ne!(running[0].1, old_pid, "the app was not restarted");
}
