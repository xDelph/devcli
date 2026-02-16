//! Python language detection strategy

use crate::{
    context::DetectionContext,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use std::collections::HashMap;

/// Detects Python projects via requirements.txt, pyproject.toml, or setup.py
#[derive(Default)]
pub struct PythonStrategy;

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
        100 // Languages have high priority
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        ctx.file_exists("requirements.txt")
            || ctx.file_exists("pyproject.toml")
            || ctx.file_exists("setup.py")
            || !ctx.glob("**/*.py").is_empty()
    }

    fn detect(&self, ctx: &DetectionContext) -> Result<DetectionResult> {
        let mut metadata = HashMap::new();

        // Detect package manager
        let package_manager = if ctx.file_exists("poetry.lock") {
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

        // Check for virtual environment
        let has_venv = ctx.file_exists(".venv")
            || ctx.file_exists("venv")
            || ctx.file_exists("env");
        metadata.insert("has_venv".to_string(), serde_json::json!(has_venv));

        // Extract app name from pyproject.toml if available
        if ctx.file_exists("pyproject.toml") {
            if let Ok(content) = ctx.read_file("pyproject.toml") {
                for line in content.lines() {
                    if line.starts_with("name") && line.contains('=') {
                        if let Some(name) = line.split('=').nth(1) {
                            let name = name.trim().trim_matches('"').trim_matches('\'');
                            metadata.insert("package_name".to_string(), serde_json::json!(name));
                        }
                    }
                }
            }
        }

        // Find Python files
        let python_files = ctx.glob("**/*.py");
        metadata.insert("python_files".to_string(), serde_json::json!(python_files.len()));

        // Detect Python version (from system)
        let version = detect_python_version();

        // Suggest framework strategies based on dependencies
        let mut suggested = Vec::new();
        if ctx.file_exists("requirements.txt") {
            if let Ok(content) = ctx.read_file("requirements.txt") {
                let content_lower = content.to_lowercase();
                if content_lower.contains("django") {
                    suggested.push("django".to_string());
                }
                if content_lower.contains("flask") {
                    suggested.push("flask".to_string());
                }
                if content_lower.contains("fastapi") {
                    suggested.push("fastapi".to_string());
                }
            }
        }

        Ok(DetectionResult {
            strategy_id: self.id().to_string(),
            category: self.category(),
            confidence: 1.0,
            data: DetectionData::Language(LanguageInfo {
                name: "Python".to_string(),
                version,
                version_source: Some("python --version".to_string()),
                primary_files: python_files,
                total_lines: None,
                metadata,
            }),
            suggested_strategies: suggested,
        })
    }
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

        // Create requirements.txt
        let requirements = temp_dir.path().join("requirements.txt");
        let mut file = fs::File::create(&requirements).unwrap();
        file.write_all(b"django==4.2.0\nrequests==2.31.0\n").unwrap();

        // Create a Python file
        fs::create_dir(temp_dir.path().join("src")).unwrap();
        fs::File::create(temp_dir.path().join("src/main.py")).unwrap();

        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = PythonStrategy;

        // Test can_apply
        assert!(strategy.can_apply(&ctx));

        // Test detect
        let result = strategy.detect(&ctx).unwrap();
        assert_eq!(result.strategy_id, "python");
        assert_eq!(result.confidence, 1.0);

        // Should suggest Django
        assert!(result.suggested_strategies.contains(&"django".to_string()));

        // Check that it's Language data
        match result.data {
            DetectionData::Language(info) => {
                assert_eq!(info.name, "Python");
                assert!(info.metadata.contains_key("package_manager"));
                assert_eq!(
                    info.metadata.get("package_manager").unwrap(),
                    &serde_json::json!("pip")
                );
            }
            _ => panic!("Expected Language data"),
        }
    }

    #[test]
    fn test_python_no_detection() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = DetectionContext::new(temp_dir.path()).unwrap();
        let strategy = PythonStrategy;

        // Should not apply without Python markers
        assert!(!strategy.can_apply(&ctx));
    }
}
