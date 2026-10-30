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

/// Expand one member pattern, segment by segment. `*` and `?` may appear in
/// any segment (`crates/*`, `pkgs/*-svc`, `services/*/api`); a pattern used to
/// be cut at its first `*`, so `pkgs/*-svc` matched every directory of `pkgs`.
fn expand_single_pattern(root: &Path, pattern: &str) -> Vec<WorkspaceInfo> {
    let segments: Vec<&str> = pattern
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();

    let mut matched: Vec<String> = vec![String::new()];
    for segment in segments {
        let mut next = Vec::new();
        for base in &matched {
            let dir = if base.is_empty() {
                root.to_path_buf()
            } else {
                root.join(base)
            };
            let join = |name: &str| {
                if base.is_empty() {
                    name.to_string()
                } else {
                    format!("{base}/{name}")
                }
            };
            if segment.contains(['*', '?']) {
                let Ok(entries) = fs::read_dir(&dir) else {
                    continue;
                };
                let mut names: Vec<String> = entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|name| {
                        !name.starts_with('.')
                            && !matches!(
                                name.as_str(),
                                "target" | "node_modules" | "dist" | "build"
                            )
                            && glob_segment_match(segment, name)
                    })
                    .collect();
                names.sort();
                next.extend(names.iter().map(|n| join(n)));
            } else if dir.join(segment).is_dir() {
                next.push(join(segment));
            }
        }
        matched = next;
    }

    matched
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(|p| workspace_info(&p))
        .collect()
}

/// Match one path segment against a pattern where `*` is any run of
/// characters and `?` exactly one.
fn glob_segment_match(pattern: &str, name: &str) -> bool {
    fn go(p: &[char], n: &[char]) -> bool {
        match (p.first(), n.first()) {
            (None, None) => true,
            (Some('*'), _) => go(&p[1..], n) || (!n.is_empty() && go(p, &n[1..])),
            (Some('?'), Some(_)) => go(&p[1..], &n[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &n[1..]),
            _ => false,
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    go(&p, &n)
}

fn workspace_info(path: &str) -> WorkspaceInfo {
    WorkspaceInfo {
        path: path.to_string(),
        name: path.split('/').next_back().map(|s| s.to_string()),
        should_detect: true,
    }
}

/// Members of a Cargo workspace whose root manifest is `content`.
///
/// Honours `[workspace].exclude`, keeps only directories that contain a
/// Cargo.toml, and names each member after its `[package].name`: commands are
/// generated as `cargo build -p <name>`, and the folder name (`crates/core`)
/// is often not the package name (`foo-core`).
pub fn expand_cargo_workspace(root: &Path, content: &str) -> Vec<WorkspaceInfo> {
    let Ok(value) = toml::from_str::<toml::Value>(content) else {
        return Vec::new();
    };
    let workspace = value.get("workspace");
    expand_manifest_workspace(
        root,
        string_list(workspace.and_then(|w| w.get("members"))),
        string_list(workspace.and_then(|w| w.get("exclude"))),
        "Cargo.toml",
        &["package", "name"],
    )
}

/// Members of a uv workspace whose root `pyproject.toml` is `content`
/// (`[tool.uv.workspace]` members/exclude, named after `[project].name`).
pub fn expand_uv_workspace(root: &Path, content: &str) -> Vec<WorkspaceInfo> {
    let Ok(value) = toml::from_str::<toml::Value>(content) else {
        return Vec::new();
    };
    let workspace = value
        .get("tool")
        .and_then(|t| t.get("uv"))
        .and_then(|u| u.get("workspace"));
    expand_manifest_workspace(
        root,
        string_list(workspace.and_then(|w| w.get("members"))),
        string_list(workspace.and_then(|w| w.get("exclude"))),
        "pyproject.toml",
        &["project", "name"],
    )
}

fn string_list(value: Option<&toml::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|m| m.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn expand_manifest_workspace(
    root: &Path,
    members: Vec<String>,
    exclude: Vec<String>,
    manifest: &str,
    name_key: &[&str],
) -> Vec<WorkspaceInfo> {
    let excluded: Vec<WorkspaceInfo> = exclude
        .iter()
        .flat_map(|pattern| expand_single_pattern(root, pattern))
        .collect();

    expand_workspace_members(root, &members)
        .into_iter()
        .filter(|m| !excluded.iter().any(|e| e.path == m.path))
        .filter(|m| root.join(&m.path).join(manifest).is_file())
        .map(|mut m| {
            let name = fs::read_to_string(root.join(&m.path).join(manifest))
                .ok()
                .and_then(|c| toml::from_str::<toml::Value>(&c).ok())
                .and_then(|v| {
                    name_key
                        .iter()
                        .try_fold(&v, |node, key| node.get(*key))
                        .and_then(|n| n.as_str().map(str::to_string))
                });
            if name.is_some() {
                m.name = name;
            }
            m
        })
        .collect()
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

    #[test]
    fn glob_segments_match_suffixes_and_inner_wildcards() {
        let dir = TempDir::new().unwrap();
        for d in [
            "pkgs/auth-svc",
            "pkgs/billing-svc",
            "pkgs/shared",
            "services/a/api",
            "services/b/api",
        ] {
            fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        let svc = expand_workspace_members(dir.path(), &["pkgs/*-svc".to_string()]);
        assert_eq!(
            svc.iter().map(|m| m.path.as_str()).collect::<Vec<_>>(),
            vec!["pkgs/auth-svc", "pkgs/billing-svc"]
        );
        let api = expand_workspace_members(dir.path(), &["services/*/api".to_string()]);
        assert_eq!(api.len(), 2);
    }

    #[test]
    fn cargo_workspace_uses_package_names_excludes_and_manifests() {
        let dir = TempDir::new().unwrap();
        for (d, name) in [
            ("crates/core", "foo-core"),
            ("crates/cli", "foo-cli"),
            ("crates/old", "old"),
        ] {
            fs::create_dir_all(dir.path().join(d)).unwrap();
            fs::write(
                dir.path().join(d).join("Cargo.toml"),
                format!("[package]\nname = \"{name}\"\n"),
            )
            .unwrap();
        }
        // A directory without a manifest is not a member.
        fs::create_dir_all(dir.path().join("crates/notes")).unwrap();

        let content = r#"
[workspace]
members = ["crates/*"]
exclude = ["crates/old"]
"#;
        let members = expand_cargo_workspace(dir.path(), content);
        let names: Vec<_> = members.iter().filter_map(|m| m.name.as_deref()).collect();
        assert_eq!(names, vec!["foo-cli", "foo-core"]);
    }
}
