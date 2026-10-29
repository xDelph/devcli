//! Versions of the toolchains installed on this machine, probed once.

use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// `<program> --version`, run at most once per program per process.
///
/// Every strategy used to spawn `node --version` / `rustc --version` /
/// `python3 --version` on each detection, and a single directory scan detects
/// many directories.
pub fn tool_version_output(program: &str) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(cached) = cache.lock().ok().and_then(|c| c.get(program).cloned()) {
        return cached;
    }

    let output = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string());

    if let Ok(mut cache) = cache.lock() {
        cache.insert(program.to_string(), output.clone());
    }
    output
}
