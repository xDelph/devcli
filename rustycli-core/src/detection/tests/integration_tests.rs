// Integration tests for detection functionality
// Tests complex scenarios with multiple environments and edge cases

#[cfg(test)]
mod tests {
    use crate::detection::*;
    use std::fs;
    use tempfile::TempDir;

    // Helper: Create a temporary directory for testing
    fn create_temp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    // Helper: Create a package.json file
    fn create_package_json(dir: &std::path::Path, name: &str, scripts: Vec<(&str, &str)>) {
        let mut pkg = serde_json::json!({
            "name": name,
            "version": "1.0.0",
            "scripts": {}
        });
        
        for (key, value) in scripts {
            pkg["scripts"][key] = serde_json::json!(value);
        }
        
        let content = serde_json::to_string_pretty(&pkg).unwrap();
        fs::write(dir.join("package.json"), content).unwrap();
    }

    // Test: App with all three environments
    #[test]
    fn test_detect_all_environments() {
        let dir = create_temp_dir();
        
        // Create package.json
        create_package_json(dir.path(), "full-app", vec![
            ("start", "node server.js"),
            ("test", "jest"),
        ]);
        
        // Create Dockerfile
        fs::write(
            dir.path().join("Dockerfile"),
            "FROM node:18\nCMD [\"npm\", \"start\"]\n"
        ).unwrap();
        
        // Create k8s directory
        fs::create_dir(dir.path().join("k8s")).unwrap();
        fs::write(
            dir.path().join("k8s/deployment.yaml"),
            "apiVersion: apps/v1\nkind: Deployment\n"
        ).unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        assert!(detected.local_commands.is_some(), "Should have local commands");
        assert!(detected.docker_commands.is_some(), "Should have docker commands");
        assert!(detected.k8s_commands.is_some(), "Should have k8s commands");
    }

    // Test: Invalid directory
    #[test]
    fn test_detect_invalid_directory() {
        let result = detect_app(std::path::Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    // Test: Empty directory (no app detected)
    #[test]
    fn test_detect_empty_directory() {
        let dir = create_temp_dir();
        
        let result = detect_app(dir.path());
        assert!(result.is_err());
    }

    // Test: Directory name fallback for app name
    #[test]
    fn test_app_name_from_directory() {
        let dir = create_temp_dir();
        let dir_path = dir.path().to_path_buf();
        
        // Create redis.conf (no name in config file)
        fs::write(dir_path.join("redis.conf"), "port 6379\n").unwrap();
        
        let result = detect_app(&dir_path);
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        // App name should be derived from directory name
        assert!(!detected.app_name.is_empty());
    }

    // Test: Tilde path contraction
    #[test]
    fn test_tilde_path_contraction() {
        let dir = create_temp_dir();
        
        // Create a simple redis app
        fs::write(dir.path().join("redis.conf"), "port 6379\n").unwrap();
        
        let result = detect_app(dir.path());
        assert!(result.is_ok());
        
        let detected = result.unwrap();
        
        // Path should be contracted to use ~ if under home directory
        // Note: In tests, temp dirs might not be under home, so we check the logic
        if detected.path.starts_with('/') && std::env::var("HOME").is_ok() {
            let home = std::env::var("HOME").unwrap();
            if dir.path().starts_with(&home) {
                assert!(detected.path.starts_with("~/"), "Path should use tilde notation: {}", detected.path);
            }
        }
    }
}