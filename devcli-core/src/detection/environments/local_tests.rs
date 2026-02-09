#[cfg(test)]
mod tests {
    use crate::detection::environments::local::{detect_local_commands, suggest_local_default};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_detect_nodejs_local_commands() {
        let temp_dir = create_test_dir();
        let package_json = temp_dir.path().join("package.json");
        fs::write(
            &package_json,
            r#"{
            "name": "test-app",
            "scripts": {
                "start": "node index.js",
                "build": "webpack",
                "test": "jest"
            }
        }"#,
        )
        .unwrap();

        let result = detect_local_commands(temp_dir.path(), "nodejs")
            .unwrap()
            .unwrap();
        assert_eq!(result.get("start"), Some(&"npm run start".to_string()));
        assert_eq!(result.get("build"), Some(&"npm run build".to_string()));
        assert_eq!(result.get("test"), Some(&"npm run test".to_string()));
    }

    #[test]
    fn test_suggest_local_default_nodejs_serve() {
        let mut commands = HashMap::new();
        commands.insert("serve".to_string(), "npm run serve".to_string());
        commands.insert("build".to_string(), "npm run build".to_string());

        let result = suggest_local_default("nodejs", &Some(commands));
        assert_eq!(result, Some("serve".to_string()));
    }

    #[test]
    fn test_suggest_local_default_no_commands() {
        let result = suggest_local_default("nodejs", &None);
        assert_eq!(result, None);
    }
}
