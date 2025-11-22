#[cfg(test)]
mod tests {
    use crate::detection::app_types::{detect_app_type, extract_app_name};
    use std::fs;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn test_detect_nx_app_type() {
        let temp_dir = create_test_dir();
        let nx_json = temp_dir.path().join("nx.json");
        fs::write(&nx_json, r#"{"version": 2}"#).unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "nx");
    }

    #[test]
    fn test_detect_nodejs_app_type() {
        let temp_dir = create_test_dir();
        let package_json = temp_dir.path().join("package.json");
        fs::write(&package_json, r#"{"name": "test-app"}"#).unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "nodejs");
    }

    #[test]
    fn test_detect_python_app_type_requirements() {
        let temp_dir = create_test_dir();
        let requirements = temp_dir.path().join("requirements.txt");
        fs::write(&requirements, "flask==2.0.0").unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "python");
    }

    #[test]
    fn test_detect_python_app_type_pyproject() {
        let temp_dir = create_test_dir();
        let pyproject = temp_dir.path().join("pyproject.toml");
        fs::write(&pyproject, r#"[project]
name = "test-app"
version = "1.0.0""#).unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "python");
    }

    #[test]
    fn test_detect_redis_app_type() {
        let temp_dir = create_test_dir();
        let redis_conf = temp_dir.path().join("redis.conf");
        fs::write(&redis_conf, "port 6379").unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "redis");
    }

    #[test]
    fn test_detect_traefik_app_type_yml() {
        let temp_dir = create_test_dir();
        let traefik_yml = temp_dir.path().join("traefik.yml");
        fs::write(&traefik_yml, "api:\n  dashboard: true").unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "traefik");
    }

    #[test]
    fn test_detect_traefik_app_type_toml() {
        let temp_dir = create_test_dir();
        let traefik_toml = temp_dir.path().join("traefik.toml");
        fs::write(&traefik_toml, "[api]\ndashboard = true").unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "traefik");
    }

    #[test]
    fn test_detect_redis_via_docker_compose() {
        let temp_dir = create_test_dir();
        let docker_compose = temp_dir.path().join("docker-compose.yml");
        fs::write(&docker_compose, r#"
services:
  redis:
    image: redis:latest
    ports:
      - "6379:6379"
"#).unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "redis");
    }

    #[test]
    fn test_detect_traefik_via_docker_compose() {
        let temp_dir = create_test_dir();
        let docker_compose = temp_dir.path().join("docker-compose.yml");
        fs::write(&docker_compose, r#"
services:
  traefik:
    image: traefik:latest
    ports:
      - "80:80"
"#).unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "traefik");
    }

    #[test]
    fn test_detect_redis_in_subdirectory() {
        let temp_dir = create_test_dir();
        let redis_dir = temp_dir.path().join("redis");
        fs::create_dir(&redis_dir).unwrap();
        let redis_conf = redis_dir.join("redis.conf");
        fs::write(&redis_conf, "port 6379").unwrap();
        let result = detect_app_type(&redis_dir).unwrap();
        assert_eq!(result, "redis");
    }

    #[test]
    fn test_detect_traefik_in_subdirectory() {
        let temp_dir = create_test_dir();
        let traefik_dir = temp_dir.path().join("traefik");
        fs::create_dir(&traefik_dir).unwrap();
        let traefik_yml = traefik_dir.join("traefik.yml");
        fs::write(&traefik_yml, "api:\n  dashboard: true").unwrap();
        let result = detect_app_type(&traefik_dir).unwrap();
        assert_eq!(result, "traefik");
    }

    #[test]
    fn test_detect_app_type_priority() {
        // Test that nx has priority over nodejs
        let temp_dir = create_test_dir();
        let nx_json = temp_dir.path().join("nx.json");
        fs::write(&nx_json, r#"{"version": 2}"#).unwrap();
        let package_json = temp_dir.path().join("package.json");
        fs::write(&package_json, r#"{"name": "test-app"}"#).unwrap();
        let result = detect_app_type(temp_dir.path()).unwrap();
        assert_eq!(result, "nx");
    }

    #[test]
    fn test_detect_app_type_no_match() {
        let temp_dir = create_test_dir();
        let random_file = temp_dir.path().join("random.txt");
        fs::write(&random_file, "nothing special").unwrap();
        let result = detect_app_type(temp_dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No supported app type detected"));
    }

    #[test]
    fn test_extract_app_name_from_package_json() {
        let temp_dir = create_test_dir();
        let package_json = temp_dir.path().join("package.json");
        fs::write(&package_json, r#"{"name": "my-awesome-app"}"#).unwrap();
        let result = extract_app_name(temp_dir.path(), "nodejs").unwrap();
        assert_eq!(result, "my-awesome-app");
    }

    #[test]
    fn test_extract_app_name_scoped_package() {
        let temp_dir = create_test_dir();
        let package_json = temp_dir.path().join("package.json");
        fs::write(&package_json, r#"{"name": "@scope/my-app"}"#).unwrap();
        let result = extract_app_name(temp_dir.path(), "nodejs").unwrap();
        assert_eq!(result, "my-app");
    }

    #[test]
    fn test_extract_app_name_from_pyproject_toml() {
        let temp_dir = create_test_dir();
        let pyproject = temp_dir.path().join("pyproject.toml");
        fs::write(&pyproject, r#"[project]
name = "python-app"
version = "1.0.0""#).unwrap();
        let result = extract_app_name(temp_dir.path(), "python").unwrap();
        assert_eq!(result, "python-app");
    }

    #[test]
    fn test_extract_app_name_from_pyproject_toml_quoted() {
        let temp_dir = create_test_dir();
        let pyproject = temp_dir.path().join("pyproject.toml");
        fs::write(&pyproject, r#"name = "quoted-app""#).unwrap();
        let result = extract_app_name(temp_dir.path(), "python").unwrap();
        assert_eq!(result, "quoted-app");
    }

    #[test]
    fn test_extract_app_name_fallback_to_directory() {
        let temp_dir = create_test_dir();
        let dir_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
        let result = extract_app_name(temp_dir.path(), "redis").unwrap();
        assert_eq!(result, dir_name);
    }

    #[test]
    fn test_extract_app_name_invalid_json() {
        let temp_dir = create_test_dir();
        let package_json = temp_dir.path().join("package.json");
        fs::write(&package_json, "invalid json").unwrap();
        let result = extract_app_name(temp_dir.path(), "nodejs");
        // Should fall back to directory name
        assert!(result.is_ok());
    }
}
