//! Custom strategy example for app-detector
//!
//! This example demonstrates:
//! - Creating a custom detection strategy
//! - Registering it with the detection engine
//! - Using it alongside built-in strategies

use app_detector::{
    context::DetectionContext,
    engine::DetectionEngine,
    registry::StrategyRegistry,
    strategy::DetectionStrategy,
    types::*,
    Result,
};
use std::collections::HashMap;

/// Custom strategy to detect Python projects
#[derive(Default)]
struct PythonStrategy;

impl DetectionStrategy for PythonStrategy {
    fn id(&self) -> &str {
        "python"
    }

    fn name(&self) -> &str {
        "Python"
    }

    fn category(&self) -> StrategyCategory {
        StrategyCategory::AppType(app_detector::types::AppTypeCategory::Language)
    }

    fn priority(&self) -> usize {
        100 // Same priority as other languages
    }

    fn can_apply(&self, ctx: &DetectionContext) -> bool {
        // Check for Python project markers
        ctx.file_exists("requirements.txt")
            || ctx.file_exists("setup.py")
            || ctx.file_exists("pyproject.toml")
            || ctx.file_exists("Pipfile")
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

        // Find Python files
        let python_files = ctx.glob("**/*.py");
        metadata.insert("python_files".to_string(), serde_json::json!(python_files.len()));

        // Detect Python version (from system)
        let version = detect_python_version();

        // Suggest framework strategies based on dependencies
        let mut suggested = Vec::new();
        if ctx.file_exists("requirements.txt") {
            if let Ok(content) = ctx.read_file("requirements.txt") {
                if content.contains("django") {
                    suggested.push("django".to_string());
                }
                if content.contains("flask") {
                    suggested.push("flask".to_string());
                }
                if content.contains("fastapi") {
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

fn main() {
    println!("Custom Strategy Example");
    println!("======================\n");

    // Create a registry and add both built-in and custom strategies
    let mut registry = StrategyRegistry::with_defaults();
    registry.register(Box::new(PythonStrategy));

    println!("Registered strategies:");
    for strategy in registry.all() {
        println!("  - {} ({})", strategy.name(), strategy.id());
    }
    println!();

    // Create the detection engine
    let engine = DetectionEngine::new(registry);

    // Run detection on current directory
    let dir = ".";
    println!("Analyzing directory: {}\n", dir);

    match engine.detect(dir) {
        Ok(report) => {
            if report.results.is_empty() {
                println!("No project types detected.");
                return;
            }

            println!("Detected {} project type(s):\n", report.results.len());

            for result in &report.results {
                println!("✓ {} ({:?})", result.strategy_id, result.category);

                // Highlight custom strategy
                if result.strategy_id == "python" {
                    println!("  ^ This is our custom Python strategy!");
                }
            }
        }
        Err(e) => {
            eprintln!("Error during detection: {}", e);
            std::process::exit(1);
        }
    }
}
