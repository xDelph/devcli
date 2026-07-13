//! Node.js package manager detection and script command formatting

use crate::context::DetectionContext;
use serde_json::Value;

/// Detect the Node.js package manager from package.json and lockfiles.
///
/// Priority: `packageManager` field → lockfiles → npm default.
pub fn detect_node_package_manager(ctx: &DetectionContext) -> String {
    if let Ok(content) = ctx.read_file("package.json") {
        if let Ok(json) = serde_json::from_str::<Value>(&content) {
            if let Some(pm) = json.get("packageManager").and_then(|v| v.as_str()) {
                if let Some(name) = pm.split('@').next() {
                    if matches!(name, "npm" | "pnpm" | "yarn" | "bun") {
                        return name.to_string();
                    }
                }
            }
        }
    }

    if ctx.file_exists("bun.lockb") || ctx.file_exists("bun.lock") {
        return "bun".to_string();
    }
    if ctx.file_exists("pnpm-lock.yaml") {
        return "pnpm".to_string();
    }
    if ctx.file_exists("yarn.lock") {
        return "yarn".to_string();
    }

    "npm".to_string()
}

/// Format a package.json script invocation for the given package manager.
pub fn node_script_command(package_manager: &str, script: &str) -> String {
    match package_manager {
        "pnpm" => format!("pnpm run {script}"),
        "yarn" => format!("yarn run {script}"),
        "bun" => format!("bun run {script}"),
        _ => format!("npm run {script}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detects_package_manager_field() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"app","packageManager":"pnpm@9.0.0"}"#,
        )
        .unwrap();
        let ctx = DetectionContext::new(dir.path()).unwrap();
        assert_eq!(detect_node_package_manager(&ctx), "pnpm");
    }

    #[test]
    fn detects_lockfile_priority() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("package.json"), r#"{"name":"app"}"#).unwrap();
        fs::write(dir.path().join("bun.lockb"), "").unwrap();
        let ctx = DetectionContext::new(dir.path()).unwrap();
        assert_eq!(detect_node_package_manager(&ctx), "bun");
    }

    #[test]
    fn formats_script_commands_per_manager() {
        assert_eq!(node_script_command("npm", "dev"), "npm run dev");
        assert_eq!(node_script_command("pnpm", "dev"), "pnpm run dev");
        assert_eq!(node_script_command("yarn", "dev"), "yarn run dev");
        assert_eq!(node_script_command("bun", "dev"), "bun run dev");
    }
}
