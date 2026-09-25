//! Workspace member expansion for Cargo and Python monorepos

use crate::types::WorkspaceInfo;
use std::fs;
use std::path::Path;

/// Expand workspace member patterns (`crates/*`, `packages/foo`) into paths.
pub fn expand_workspace_members(root: &Path, patterns: &[String]) -> Vec<WorkspaceInfo> {
    let mut members = Vec::new();

    for pattern in patterns {
        members.extend(expand_single_pattern(root, pattern));
    }

    members.sort_by(|a, b| a.path.cmp(&b.path));
    members.dedup_by(|a, b| a.path == b.path);
    members
}

fn expand_single_pattern(root: &Path, pattern: &str) -> Vec<WorkspaceInfo> {
    if pattern.contains('*') {
        expand_glob_pattern(root, pattern)
    } else if root.join(pattern).is_dir() {
        vec![workspace_info(pattern)]
    } else {
        Vec::new()
    }
}

fn expand_glob_pattern(root: &Path, pattern: &str) -> Vec<WorkspaceInfo> {
    let Some((parent, _)) = pattern.split_once('*') else {
        return Vec::new();
    };
    let parent = parent.trim_end_matches('/');
    let parent_path = root.join(parent);

    if !parent_path.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = fs::read_dir(&parent_path) else {
        return Vec::new();
    };

    let mut members = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if matches!(
            name.as_ref(),
            "target" | "node_modules" | ".git" | "dist" | "build"
        ) {
            continue;
        }
        let rel = if parent.is_empty() {
            name.to_string()
        } else {
            format!("{parent}/{name}")
        };
        members.push(workspace_info(&rel));
    }
    members
}

fn workspace_info(path: &str) -> WorkspaceInfo {
    WorkspaceInfo {
        path: path.to_string(),
        name: path.split('/').next_back().map(|s| s.to_string()),
        should_detect: true,
    }
}

/// Parse `[workspace].members` from a Cargo.toml manifest.
pub fn cargo_workspace_members(content: &str) -> Option<Vec<String>> {
    let value: toml::Value = toml::from_str(content).ok()?;
    let members = value.get("workspace")?.get("members")?.as_array()?;
    Some(
        members
            .iter()
            .filter_map(|m| m.as_str().map(|s| s.to_string()))
            .collect(),
    )
}

/// Parse uv workspace members from pyproject.toml.
pub fn uv_workspace_members(content: &str) -> Option<Vec<String>> {
    let value: toml::Value = toml::from_str(content).ok()?;
    let members = value
        .get("tool")?
        .get("uv")?
        .get("workspace")?
        .get("members")?
        .as_array()?;
    Some(
        members
            .iter()
            .filter_map(|m| m.as_str().map(|s| s.to_string()))
            .collect(),
    )
}

/// Discover Python packages as immediate subdirs containing pyproject.toml.
pub fn discover_python_packages(root: &Path) -> Vec<WorkspaceInfo> {
    let mut members = Vec::new();
    for parent in ["packages", "apps", "libs", "services"] {
        let parent_path = root.join(parent);
        if !parent_path.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&parent_path) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && entry.path().join("pyproject.toml").exists() {
                    let rel = format!("{parent}/{}", entry.file_name().to_string_lossy());
                    members.push(workspace_info(&rel));
                }
            }
        }
    }
    members
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn expands_cargo_glob_members() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("crates/api")).unwrap();
        fs::create_dir_all(dir.path().join("crates/lib")).unwrap();
        fs::write(
            dir.path().join("crates/api/Cargo.toml"),
            "[package]\nname=\"api\"\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("crates/lib/Cargo.toml"),
            "[package]\nname=\"lib\"\n",
        )
        .unwrap();

        let members = expand_workspace_members(dir.path(), &["crates/*".to_string()]);
        assert_eq!(members.len(), 2);
        assert!(members.iter().any(|m| m.path == "crates/api"));
        assert!(members.iter().any(|m| m.path == "crates/lib"));
    }

    #[test]
    fn parses_cargo_workspace_members() {
        let content = r#"
[workspace]
members = ["crates/a", "crates/b"]
"#;
        let members = cargo_workspace_members(content).unwrap();
        assert_eq!(members, vec!["crates/a", "crates/b"]);
    }

    #[test]
    fn parses_uv_workspace_members() {
        let content = r#"
[tool.uv.workspace]
members = ["packages/*"]
"#;
        let members = uv_workspace_members(content).unwrap();
        assert_eq!(members, vec!["packages/*"]);
    }
}
