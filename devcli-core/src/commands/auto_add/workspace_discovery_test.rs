// Cargo workspace discovery tests for auto-add

#[cfg(test)]
mod tests {
    use crate::commands::auto_add::single_app::discover_all_apps;
    use std::fs;
    use tempfile::TempDir;

    fn create_temp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    /// The workspace-root app is named after the repository directory. Derive it
    /// at runtime so these tests survive a repo/directory rename.
    fn expected_root_app_name(repo_root: &std::path::Path) -> String {
        repo_root
            .canonicalize()
            .unwrap_or_else(|_| repo_root.to_path_buf())
            .file_name()
            .expect("repo root has a file name")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn detect_repo_root_workspace() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        if !repo_root.join("Cargo.toml").exists() {
            return;
        }
        let detected = crate::detection::detect_app(repo_root).unwrap();
        assert_eq!(detected.app_type, "rust");
        assert!(detected.local_commands.is_some());
    }

    #[test]
    fn detect_each_workspace_member() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        for member in [
            "app-detector",
            "config-manager",
            "devcli",
            "devcli-core",
            "env-flow",
            "process-manager",
            "website",
        ] {
            let path = repo_root.join(member);
            if !path.exists() {
                continue;
            }
            let detected = crate::detection::detect_app(&path).unwrap();
            let expected = if member == "website" {
                "nodejs"
            } else {
                "rust"
            };
            assert_eq!(detected.app_type, expected, "unexpected type for {member}");
        }
    }

    #[test]
    fn detect_app_relative_dot_path() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        if !repo_root.join("Cargo.toml").exists() {
            return;
        }

        // The working directory is process-global: config_home() looks for a
        // ./.devcli, so a concurrent test changing it would resolve the wrong
        // config root. Serialise with the shared env guard.
        let _guard = crate::test_utils::env_guard();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo_root).unwrap();
        let result = crate::detection::detect_app(std::path::Path::new("."));
        std::env::set_current_dir(original).unwrap();

        assert!(
            result.is_ok(),
            "detect_app('.') should work from repo root: {:?}",
            result
        );
        assert_eq!(result.unwrap().app_type, "rust");
    }

    #[test]
    fn discover_relative_repo_root_includes_workspace() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        if !repo_root.join("website").join("package.json").exists() {
            return;
        }

        let _guard = crate::test_utils::env_guard();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo_root).unwrap();
        let apps = discover_all_apps(std::path::Path::new(".")).unwrap();
        std::env::set_current_dir(original).unwrap();

        let root_name = expected_root_app_name(repo_root);
        let names: Vec<_> = apps.iter().map(|a| a.app_name.as_str()).collect();
        assert!(
            names.contains(&root_name.as_str()),
            "expected workspace root '{root_name}' when scanning '.', got {names:?}"
        );
    }

    #[test]
    fn discover_real_repo_root() {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("devcli-core has parent");
        if !repo_root.join("website").join("package.json").exists() {
            return;
        }

        let apps = discover_all_apps(repo_root).unwrap();
        let root_name = expected_root_app_name(repo_root);
        let names: Vec<_> = apps.iter().map(|a| a.app_name.as_str()).collect();
        let types: Vec<_> = apps.iter().map(|a| a.app_type.as_str()).collect();
        assert!(
            names.contains(&root_name.as_str()),
            "expected workspace root app '{root_name}', got {names:?}"
        );
        assert!(
            types.iter().filter(|t| **t == "rust").count() >= 1,
            "expected rust workspace at root, got {types:?}"
        );
        assert!(
            types.contains(&"nodejs"),
            "expected nodejs website in repo root discovery, got {types:?}"
        );
        assert!(
            apps.len() <= 3,
            "cargo workspace should not list every member crate separately, got {names:?}"
        );
    }

    #[test]
    fn discover_cargo_workspace_with_website_sibling() {
        let root_dir = create_temp_dir();

        fs::write(
            root_dir.path().join("Cargo.toml"),
            r#"[workspace]
members = ["devcli-core", "env-flow"]
resolver = "2"
"#,
        )
        .unwrap();
        for member in ["devcli-core", "env-flow"] {
            let member_dir = root_dir.path().join(member);
            fs::create_dir_all(member_dir.join("src")).unwrap();
            fs::write(
                member_dir.join("Cargo.toml"),
                format!("[package]\nname = \"{member}\"\nversion = \"0.1.0\"\n"),
            )
            .unwrap();
            fs::write(member_dir.join("src/lib.rs"), "").unwrap();
        }

        let website_dir = root_dir.path().join("website");
        fs::create_dir(&website_dir).unwrap();
        fs::write(
            website_dir.join("package.json"),
            r#"{"name":"devcli-website","scripts":{"dev":"astro dev"}}"#,
        )
        .unwrap();

        let apps = discover_all_apps(root_dir.path()).unwrap();
        let types: Vec<_> = apps.iter().map(|a| a.app_type.as_str()).collect();
        let names: Vec<_> = apps.iter().map(|a| a.app_name.as_str()).collect();
        assert!(
            types.contains(&"rust"),
            "expected rust workspace at root in {types:?}"
        );
        assert!(
            types.contains(&"nodejs"),
            "expected nodejs website in {types:?}"
        );
        assert_eq!(
            apps.len(),
            2,
            "expected workspace + website only, got {names:?}"
        );
    }

    // Build a fake node monorepo: root package.json + sub-apps in standard
    // container dirs (apps/, packages/, libs/, modules/).
    fn write_package_json(dir: &std::path::Path, name: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("package.json"),
            format!(r#"{{"name":"{name}","scripts":{{"dev":"echo {name}"}}}}"#),
        )
        .unwrap();
    }

    #[test]
    fn discover_node_monorepo_apps_in_containers() {
        let root_dir = create_temp_dir();
        write_package_json(root_dir.path(), "monorepo");

        write_package_json(&root_dir.path().join("apps/web"), "web");
        write_package_json(&root_dir.path().join("apps/api"), "api");
        write_package_json(&root_dir.path().join("packages/shared"), "shared");
        write_package_json(&root_dir.path().join("libs/ui"), "ui");
        write_package_json(&root_dir.path().join("modules/worker"), "worker");
        write_package_json(&root_dir.path().join("services/cron"), "cron");

        // Non-container subdir with its own package.json still gets discovered
        write_package_json(&root_dir.path().join("standalone"), "standalone");

        let apps = discover_all_apps(root_dir.path()).unwrap();
        let names: Vec<String> = apps.iter().map(|a| a.app_name.clone()).collect();

        assert!(
            names.contains(&"monorepo".to_string()),
            "root app missing: {names:?}"
        );
        for expected in ["web", "api", "shared", "ui", "worker", "cron", "standalone"] {
            assert!(
                names.contains(&expected.to_string()),
                "expected {expected} discovered, got {names:?}"
            );
        }
    }

    #[test]
    fn discover_container_recursion_is_one_level_only() {
        let root_dir = create_temp_dir();
        write_package_json(root_dir.path(), "monorepo");

        // apps/web should be found, apps/web/deeper must not
        write_package_json(&root_dir.path().join("apps/web"), "web");
        write_package_json(&root_dir.path().join("apps/web/deeper"), "deeper");

        let apps = discover_all_apps(root_dir.path()).unwrap();
        let names: Vec<String> = apps.iter().map(|a| a.app_name.clone()).collect();

        assert!(
            names.contains(&"web".to_string()),
            "should find apps/web: {names:?}"
        );
        assert!(
            !names.contains(&"deeper".to_string()),
            "should not recurse past one level: {names:?}"
        );
    }
}
