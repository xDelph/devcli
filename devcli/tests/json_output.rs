//! `--json` contract of the configuration commands, through the real binary:
//! stdout holds exactly one JSON value on success, and an error is
//! `{"error": ...}` on stderr with exit code 1 - never a prompt.

use serde_json::Value;
use std::process::{Command, Output, Stdio};

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("create sandbox dir");
        let config = serde_json::json!({
            "projects": { "proj": { "apps": { "api": {
                "type": "custom",
                "path": dir.path(),
                "commands": { "local": { "start": "sleep 1" } },
                "defaults": { "local": "start" }
            } } } }
        });
        std::fs::write(
            dir.path().join("config.json"),
            serde_json::to_vec_pretty(&config).unwrap(),
        )
        .expect("write config");
        Self { dir }
    }

    fn devcli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_devcli"))
            .args(args)
            .env("DEVCLI_CONFIG_DIR", self.dir.path())
            .env("HOME", self.dir.path())
            .env("NO_COLOR", "1")
            .env("DEVCLI_AGENT", "0")
            .env_remove("devcli_CONFIG_DIR")
            .env_remove("LOG_LEVEL")
            .current_dir(self.dir.path())
            .stdin(Stdio::null())
            .output()
            .expect("run devcli")
    }
}

/// The single JSON value on stdout of a successful `devcli ARGS`.
fn json_ok(sandbox: &Sandbox, args: &[&str]) -> Value {
    let out = sandbox.devcli(args);
    assert!(
        out.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "{args:?}: stdout is not one JSON value ({e}): {:?}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

/// The `error` message of a failed `devcli ARGS`, checking the contract.
fn json_err(sandbox: &Sandbox, args: &[&str]) -> String {
    let out = sandbox.devcli(args);
    assert_eq!(out.status.code(), Some(1), "{args:?} did not exit 1");
    assert!(out.stdout.is_empty(), "{args:?} wrote to stdout");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let line = stderr.lines().last().unwrap_or_default();
    let value: Value = serde_json::from_str(line)
        .unwrap_or_else(|e| panic!("{args:?}: stderr is not a JSON error ({e}): {stderr:?}"));
    value["error"].as_str().expect("error message").to_string()
}

#[test]
fn preference_commands_answer_one_document() {
    let sb = Sandbox::new();

    let set = json_ok(&sb, &["--json", "pref", "set", "default-env", "compose"]);
    assert_eq!(set["key"], "default-env");
    assert_eq!(set["value"], "docker-compose");

    let show = json_ok(&sb, &["--json", "pref", "show"]);
    assert_eq!(show["default-env"], "docker-compose");

    let reset = json_ok(&sb, &["--json", "pref", "reset"]);
    assert_eq!(reset["default-env"], "local");
}

#[test]
fn env_commands_answer_one_document() {
    let sb = Sandbox::new();
    let base = ["--json", "env"];
    let run = |rest: &[&str]| {
        let mut args = base.to_vec();
        args.extend_from_slice(rest);
        json_ok(&sb, &args)
    };

    let added = run(&["add", "api", "-s", "dev", "-c", "compose", "-f", ".env.dev"]);
    assert_eq!(added["project"], "proj");
    assert_eq!(added["context"], "docker-compose");

    let default = run(&[
        "set-default",
        "api",
        "-c",
        "local",
        "-s",
        "dev",
        "-p",
        "proj",
    ]);
    assert_eq!(default["stage"], "dev");

    let list = run(&["list", "api"]);
    assert_eq!(list["env_files"]["dev"]["docker-compose"], ".env.dev");
    assert_eq!(list["default_stages"]["local"], "dev");

    let removed = run(&["remove", "api", "-s", "dev"]);
    assert_eq!(removed["removed"]["docker-compose"], ".env.dev");
}

#[test]
fn config_commands_answer_one_document() {
    let sb = Sandbox::new();

    let added = json_ok(
        &sb,
        &[
            "--json",
            "config",
            "add-command",
            "api",
            "local",
            "test",
            "true",
        ],
    );
    assert_eq!(added["command"], "test");
    let default = json_ok(
        &sb,
        &["--json", "config", "set-default", "api", "local", "test"],
    );
    assert_eq!(default["default"], "test");
    let removed = json_ok(
        &sb,
        &["--json", "config", "remove-command", "api", "local", "test"],
    );
    assert_eq!(removed["default_cleared"], true);

    let fresh = Sandbox::new();
    std::fs::remove_file(fresh.dir.path().join("config.json")).unwrap();
    let init = json_ok(&fresh, &["--json", "config", "init"]);
    assert!(init["created"].as_str().unwrap().ends_with("config.json"));
}

#[test]
fn missing_arguments_are_an_error_instead_of_a_prompt() {
    let sb = Sandbox::new();

    let err = json_err(&sb, &["--json", "config", "add-command"]);
    assert!(err.contains("Missing the app name"), "{err}");

    let err = json_err(&sb, &["--json", "config", "set-default", "api"]);
    assert!(err.contains("Missing the environment"), "{err}");

    let err = json_err(&sb, &["--json", "config", "set-default", "api", "local"]);
    assert!(err.contains("Missing the command name"), "{err}");

    let err = json_err(
        &sb,
        &["--json", "config", "edit-command", "api", "local", "start"],
    );
    assert!(err.contains("add-command"), "{err}");
}
