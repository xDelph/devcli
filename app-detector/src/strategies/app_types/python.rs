//! Python language and workspace detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    utils::workspace::{discover_python_packages, expand_workspace_members, uv_workspace_members},
    Result,
};
use std::collections::HashMap;

/// Detects Python projects via requirements.txt, pyproject.toml, or .py files
#[derive(Default)]
pub struct PythonStrategy;

fn has_project_python_sources(ctx: &DetectionContext) -> bool {
    ctx.glob("**/*.py")
        .into_iter()
        .any(|path| !is_fixture_python_path(&path))
}

fn is_fixture_python_path(path: &std::path::Path) -> bool {
    let parts: Vec<_> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    parts.first() == Some(&"tests") || parts.contains(&"fixtures")
}

impl DetectionStrategy for PythonStrategy {
    fn id(&self) -> &str {
        "python"
    }

    fn name(&self) -> &str {
        "Python"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(AppTypeCategory::Language)
    }

    fn priority(&self) -> usize {
        100
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // Cargo workspaces often keep Python fixtures under tests/ — don't trump Rust.
        if ctx.file_exists("Cargo.toml") {
            return false;
        }

        ctx.file_exists("requirements.txt")
            || ctx.file_exists("pyproject.toml")
            || ctx.file_exists("setup.py")
            || has_project_python_sources(ctx)
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        if ctx.file_exists("pyproject.toml") {
            let content = ctx.read_file("pyproject.toml")?;
            if let Some(members) = uv_workspace_members(&content) {
                if !members.is_empty() {
                    return detect_workspace(ctx, &content, members, "uv");
                }
            }

            let discovered = discover_python_packages(&ctx.root_path);
            if discovered.len() >= 2 {
                return detect_workspace_from_members(ctx, &content, discovered, "python");
            }
        }

        detect_single_package(ctx)
    }
}

fn detect_workspace(
    ctx: &DetectionContext,
    content: &str,
    member_patterns: Vec<String>,
    tool: &str,
) -> Result<DetectionResult> {
    let workspace_info = expand_workspace_members(&ctx.root_path, &member_patterns);
    detect_workspace_from_members(ctx, content, workspace_info, tool)
}

fn detect_workspace_from_members(
    ctx: &DetectionContext,
    content: &str,
    workspace_info: Vec<WorkspaceInfo>,
    tool: &str,
) -> Result<DetectionResult> {
    let workspaces: Vec<String> = workspace_info.iter().map(|w| w.path.clone()).collect();
    let mut metadata = package_metadata(ctx, content);
    metadata.insert(
        "workspace_count".to_string(),
        serde_json::json!(workspace_info.len()),
    );

    Ok(DetectionResult {
        strategy_id: "python".to_string(),
        category: StrategyCategory::AppType(AppTypeCategory::Monorepo),
        confidence: 1.0,
        data: DetectionData::Monorepo(MonorepoInfo {
            tool: tool.to_string(),
            version: detect_python_version(),
            config_file: std::path::PathBuf::from("pyproject.toml"),
            workspace_info,
            workspaces,
            metadata,
        }),
        suggested_strategies: vec![],
    })
}

fn detect_single_package(ctx: &DetectionContext) -> Result<DetectionResult> {
    let content = ctx
        .file_exists("pyproject.toml")
        .then(|| ctx.read_file("pyproject.toml"))
        .transpose()?
        .unwrap_or_default();

    let mut metadata = package_metadata(ctx, &content);
    let python_files = ctx.glob("**/*.py");
    metadata.insert(
        "python_files".to_string(),
        serde_json::json!(python_files.len()),
    );

    let mut suggested = Vec::new();
    if ctx.file_exists("requirements.txt") {
        if let Ok(req) = ctx.read_file("requirements.txt") {
            let lower = req.to_lowercase();
            if lower.contains("django") {
                suggested.push("django".to_string());
            }
            if lower.contains("flask") {
                suggested.push("flask".to_string());
            }
            if lower.contains("fastapi") {
                suggested.push("fastapi".to_string());
            }
        }
    }

    Ok(DetectionResult {
        strategy_id: "python".to_string(),
        category: self_category(),
        confidence: 1.0,
        data: DetectionData::Language(LanguageInfo {
            name: "Python".to_string(),
            version: detect_python_version(),
            version_source: Some("python --version".to_string()),
            primary_files: python_files,
            total_lines: None,
            metadata,
        }),
        suggested_strategies: suggested,
    })
}

fn self_category() -> StrategyCategory {
    StrategyCategory::AppType(AppTypeCategory::Language)
}

fn package_metadata(ctx: &DetectionContext, pyproject: &str) -> HashMap<String, serde_json::Value> {
    let mut metadata = HashMap::new();

    let package_manager = if ctx.file_exists("uv.lock") {
        "uv"
    } else if ctx.file_exists("poetry.lock") {
        "poetry"
    } else if ctx.file_exists("Pipfile.lock") {
        "pipenv"
    } else if ctx.file_exists("requirements.txt") {
        "pip"
    } else {
        "unknown"
    };
    metadata.insert(
        "package_manager".to_string(),
        serde_json::json!(package_manager),
    );

    let has_venv = ctx.file_exists(".venv") || ctx.file_exists("venv") || ctx.file_exists("env");
    metadata.insert("has_venv".to_string(), serde_json::json!(has_venv));

    if !pyproject.is_empty() {
        for line in pyproject.lines() {
            if line.starts_with("name") && line.contains('=') {
                if let Some(name) = line.split('=').nth(1) {
                    let name = name.trim().trim_matches('"').trim_matches('\'');
                    metadata.insert("package_name".to_string(), serde_json::json!(name));
                }
            }
        }
    }

    metadata
}

fn detect_python_version() -> Option<String> {
    use std::process::Command;

    Command::new("python3")
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|s| s.split_whitespace().nth(1).map(|v| v.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_python_detection() {
        let temp_dir = TempDir::new().unwrap();

        let requirements = temp_dir.path().join("requirements.txt");
        let mut file = fs::File::create(&requirements).unwrap();
        file.write_all(b"django==4.2.0\nrequests==2.31.0\n")
            .unwrap();

        fs::create_dir(temp_dir.path().join("src")).unwrap();
        fs::File::create(temp_dir.path().join("src/main.py")).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = PythonStrategy;

        assert!(strategy.can_apply(&ctx));
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "python");
        assert!(result.suggested_strategies.contains(&"django".to_string()));

        match result.data {
            DetectionData::Language(info) => {
                assert_eq!(info.metadata.get("package_manager").unwrap(), "pip");
            }
            _ => panic!("Expected Language data"),
        }
    }

    #[test]
    fn test_python_uv_workspace_detection() {
        let temp_dir = TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("packages/api")).unwrap();
        fs::create_dir_all(temp_dir.path().join("packages/lib")).unwrap();
        fs::write(
            temp_dir.path().join("pyproject.toml"),
            r#"[tool.uv.workspace]
members = ["packages/*"]
"#,
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("packages/api/pyproject.toml"),
            "name = \"api\"\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("packages/lib/pyproject.toml"),
            "name = \"lib\"\n",
        )
        .unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let result = PythonStrategy.detect(&ctx).unwrap();

        match result.data {
            DetectionData::Monorepo(info) => {
                assert_eq!(info.tool, "uv");
                assert_eq!(info.workspace_info.len(), 2);
            }
            _ => panic!("Expected Monorepo data"),
        }
    }

    #[test]
    fn test_python_no_detection() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        assert!(!PythonStrategy.can_apply(&ctx));
    }
}
