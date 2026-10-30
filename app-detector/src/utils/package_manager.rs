//! Node.js package manager detection and script command formatting

use crate::context::DetectionContext;
use serde_json::Value;
use std::path::Path;

/// Detect the Node.js package manager from package.json and lockfiles.
///
/// Priority: `packageManager` field → lockfiles → the same in parent
/// directories (up to the repository root) → npm default.
///
/// In a pnpm/yarn/bun workspace the lockfile and `packageManager` live at the
/// monorepo root, not in `apps/web`; looking only in the app's own directory
/// made every workspace app run its scripts with `npm run`.
pub fn detect_node_package_manager(ctx: &DetectionContext) -> String {
    let root = ctx
        .root_path
        .canonicalize()
        .unwrap_or_else(|_| ctx.root_path.clone());
    for (depth, dir) in root.ancestors().enumerate() {
        if depth > MAX_PARENT_LEVELS {
            break;
        }
        if let Some(pm) = package_manager_in(dir) {
            return pm;
        }
        // Do not wander out of the repository the app belongs to.
        if dir.join(".git").exists() {
            break;
        }
    }

    "npm".to_string()
}

/// How many parent directories are searched for a workspace root.
const MAX_PARENT_LEVELS: usize = 6;

fn package_manager_in(dir: &Path) -> Option<String> {
    if let Ok(content) = std::fs::read_to_string(dir.join("package.json"))
        && let Ok(json) = serde_json::from_str::<Value>(&content)
        && let Some(pm) = json.get("packageManager").and_then(|v| v.as_str())
        && let Some(name) = pm.split('@').next()
        && matches!(name, "npm" | "pnpm" | "yarn" | "bun")
    {
        return Some(name.to_string());
    }

    let lockfiles = [
        ("bun.lockb", "bun"),
        ("bun.lock", "bun"),
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "yarn"),
        ("package-lock.json", "npm"),
    ];
    lockfiles
        .iter()
        .find(|(file, _)| dir.join(file).is_file())
        .map(|(_, pm)| pm.to_string())
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

    #[test]
    fn workspace_apps_use_the_root_package_manager() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{"name":"mono","private":true}"#,
        )
        .unwrap();
        let app = dir.path().join("apps/web");
        fs::create_dir_all(&app).unwrap();
        fs::write(app.join("package.json"), r#"{"name":"web"}"#).unwrap();

        let ctx = DetectionContext::new(&app).unwrap();
        assert_eq!(detect_node_package_manager(&ctx), "pnpm");
    }

    #[test]
    fn search_stops_at_the_repository_root() {
        let outer = TempDir::new().unwrap();
        fs::write(outer.path().join("yarn.lock"), "").unwrap();
        let repo = outer.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::write(repo.join("package.json"), r#"{"name":"app"}"#).unwrap();

        let ctx = DetectionContext::new(&repo).unwrap();
        assert_eq!(detect_node_package_manager(&ctx), "npm");
    }
}
