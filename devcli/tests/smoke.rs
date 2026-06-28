//! Smoke tests for the devcli binary entrypoint.

use std::process::Command;

fn devcli_bin() -> String {
    env!("CARGO_BIN_EXE_devcli").to_string()
}

#[test]
fn help_exits_successfully() {
    let output = Command::new(devcli_bin())
        .arg("--help")
        .output()
        .expect("run devcli --help");

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("start"));
    assert!(stdout.contains("ui"));
}

#[test]
fn version_exits_successfully() {
    let output = Command::new(devcli_bin())
        .arg("--version")
        .output()
        .expect("run devcli --version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("devcli"));
}

#[test]
fn missing_subcommand_exits_with_error() {
    let output = Command::new(devcli_bin())
        .output()
        .expect("run devcli with no args");

    assert!(!output.status.success());
}

#[test]
fn hidden_internal_spawner_not_in_help() {
    let output = Command::new(devcli_bin())
        .arg("--help")
        .output()
        .expect("run devcli --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("internal-spawner"));
}
