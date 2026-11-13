//! Reading `pyproject.toml` as TOML.
//!
//! A line scan for `name = ...` took the last such line in any table:
//! `[[tool.uv.index]] name = "pytorch"` named the app "pytorch", and
//! `namespaces = false` named it "false".

use toml::Value;

fn parse(content: &str) -> Option<Value> {
    toml::from_str(content).ok()
}

fn get<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |node, key| node.get(*key))
}

/// The package name: `[project].name`, else `[tool.poetry].name`.
pub fn project_name(content: &str) -> Option<String> {
    let value = parse(content)?;
    [&["project", "name"][..], &["tool", "poetry", "name"][..]]
        .iter()
        .find_map(|path| get(&value, path)?.as_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
}

/// The tool that manages the project, from its `[tool.*]` sections or
/// build backend: `poetry`, `pdm`, `uv`, `hatch`, or `None` (setuptools,
/// flit and anything else are installed with plain pip).
pub fn project_tool(content: &str) -> Option<&'static str> {
    let value = parse(content)?;
    let backend = get(&value, &["build-system", "build-backend"])
        .and_then(Value::as_str)
        .unwrap_or_default();
    let has_tool = |name: &str| get(&value, &["tool", name]).is_some();

    if has_tool("poetry") || backend.starts_with("poetry") {
        Some("poetry")
    } else if has_tool("pdm") || backend.starts_with("pdm") {
        Some("pdm")
    } else if has_tool("uv") || backend.starts_with("uv_build") {
        Some("uv")
    } else if has_tool("hatch") || backend.starts_with("hatchling") {
        Some("hatch")
    } else {
        None
    }
}

/// Console scripts the package installs: `[project.scripts]` and
/// `[tool.poetry.scripts]`, sorted.
pub fn script_names(content: &str) -> Vec<String> {
    let Some(value) = parse(content) else {
        return Vec::new();
    };
    let mut names: Vec<String> = [
        &["project", "scripts"][..],
        &["tool", "poetry", "scripts"][..],
    ]
    .iter()
    .filter_map(|path| get(&value, path)?.as_table())
    .flat_map(|table| table.keys().cloned())
    .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_comes_from_the_project_table_only() {
        let content = r#"
[project]
name = "my-api"

[tool.setuptools]
namespaces = false

[[tool.uv.index]]
name = "pytorch"
url = "https://download.pytorch.org/whl/cpu"
"#;
        assert_eq!(project_name(content).as_deref(), Some("my-api"));
    }

    #[test]
    fn poetry_name_is_the_fallback() {
        let content = "[tool.poetry]\nname = \"legacy\"\nversion = \"0.1.0\"\n";
        assert_eq!(project_name(content).as_deref(), Some("legacy"));
        assert_eq!(project_name("name = \"top-level\"\n"), None);
        assert_eq!(project_name("not toml ["), None);
    }

    #[test]
    fn tools_are_read_from_sections_and_backends() {
        let hatch =
            "[build-system]\nrequires = [\"hatchling\"]\nbuild-backend = \"hatchling.build\"\n";
        assert_eq!(project_tool(hatch), Some("hatch"));
        assert_eq!(project_tool("[tool.pdm]\n"), Some("pdm"));
        assert_eq!(
            project_tool("[tool.poetry]\nname = \"x\"\n"),
            Some("poetry")
        );
        assert_eq!(
            project_tool("[tool.uv]\ndev-dependencies = []\n"),
            Some("uv")
        );
        let setuptools = "[build-system]\nbuild-backend = \"setuptools.build_meta\"\n";
        assert_eq!(project_tool(setuptools), None);
    }

    #[test]
    fn scripts_from_both_tables() {
        let content = r#"
[project.scripts]
serve = "app.main:run"

[tool.poetry.scripts]
cli = "app.cli:main"
"#;
        assert_eq!(script_names(content), vec!["cli", "serve"]);
    }
}
