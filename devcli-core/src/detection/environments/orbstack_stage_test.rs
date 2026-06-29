// Tests for stage-specific environment file detection in OrbStack
// These tests verify the priority order for finding .env files based on deployment stage

#[cfg(test)]
mod tests {
    use crate::env_flow_support::{find_env_file, load_env_vars_for_runtime};
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    /// Test that stage-specific file at root level is prioritized over base .env
    #[test]
    fn test_stage_specific_file_priority_at_root() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        // Create both base and stage-specific env files
        let base_env = app_path.join(".env");
        let mut file = fs::File::create(&base_env).unwrap();
        writeln!(file, "ENV=base").unwrap();
        file.flush().unwrap();

        let stage_env = app_path.join(".env.dev");
        let mut file = fs::File::create(&stage_env).unwrap();
        writeln!(file, "ENV=dev").unwrap();
        file.flush().unwrap();

        // With stage specified, should find .env.dev
        let result = find_env_file(app_path, None, Some("dev")).unwrap();
        assert_eq!(result, Some(".env.dev".to_string()));

        // Load vars should get dev values
        let env_vars = load_env_vars_for_runtime(app_path, None, Some("dev")).unwrap();
        assert_eq!(env_vars.get("ENV"), Some(&"dev".to_string()));
    }

    /// Test that base .env is used when stage-specific file doesn't exist
    #[test]
    fn test_fallback_to_base_env_when_stage_missing() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        // Create only base .env file
        let base_env = app_path.join(".env");
        let mut file = fs::File::create(&base_env).unwrap();
        writeln!(file, "ENV=base").unwrap();
        file.flush().unwrap();

        // With stage specified but file missing, should fall back to .env
        let result = find_env_file(app_path, None, Some("qa")).unwrap();
        assert_eq!(result, Some(".env".to_string()));

        // Load vars should get base values
        let env_vars = load_env_vars_for_runtime(app_path, None, Some("qa")).unwrap();
        assert_eq!(env_vars.get("ENV"), Some(&"base".to_string()));
    }

    /// Test that no stage specified uses base .env file
    #[test]
    fn test_no_stage_uses_base_env() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        // Create both files
        let base_env = app_path.join(".env");
        let mut file = fs::File::create(&base_env).unwrap();
        writeln!(file, "ENV=base").unwrap();
        file.flush().unwrap();

        let stage_env = app_path.join(".env.prod");
        let mut file = fs::File::create(&stage_env).unwrap();
        writeln!(file, "ENV=prod").unwrap();
        file.flush().unwrap();

        // Without stage, should use base .env
        let result = find_env_file(app_path, None, None).unwrap();
        assert_eq!(result, Some(".env".to_string()));

        let env_vars = load_env_vars_for_runtime(app_path, None, None).unwrap();
        assert_eq!(env_vars.get("ENV"), Some(&"base".to_string()));
    }

    /// Test stage-specific file at Dockerfile level has highest priority
    #[test]
    fn test_stage_file_at_dockerfile_level_priority() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        // Create docker subdirectory
        let docker_dir = app_path.join("docker");
        fs::create_dir(&docker_dir).unwrap();

        // Create Dockerfile
        let dockerfile = docker_dir.join("Dockerfile");
        fs::File::create(&dockerfile).unwrap();

        // Create all four possible env files
        let root_base = app_path.join(".env");
        let mut file = fs::File::create(&root_base).unwrap();
        writeln!(file, "ENV=root_base").unwrap();
        file.flush().unwrap();

        let root_stage = app_path.join(".env.qa");
        let mut file = fs::File::create(&root_stage).unwrap();
        writeln!(file, "ENV=root_qa").unwrap();
        file.flush().unwrap();

        let docker_base = docker_dir.join(".env");
        let mut file = fs::File::create(&docker_base).unwrap();
        writeln!(file, "ENV=docker_base").unwrap();
        file.flush().unwrap();

        let docker_stage = docker_dir.join(".env.qa");
        let mut file = fs::File::create(&docker_stage).unwrap();
        writeln!(file, "ENV=docker_qa").unwrap();
        file.flush().unwrap();

        // With stage and dockerfile_path, should find docker/.env.qa
        let result = find_env_file(app_path, Some("docker/Dockerfile"), Some("qa")).unwrap();
        assert_eq!(result, Some("docker/.env.qa".to_string()));

        let env_vars =
            load_env_vars_for_runtime(app_path, Some("docker/Dockerfile"), Some("qa")).unwrap();
        assert_eq!(env_vars.get("ENV"), Some(&"docker_qa".to_string()));
    }

    /// Test priority order: docker stage > root stage > docker base > root base
    #[test]
    fn test_priority_order_with_missing_files() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        // Create docker subdirectory
        let docker_dir = app_path.join("docker");
        fs::create_dir(&docker_dir).unwrap();
        let dockerfile = docker_dir.join("Dockerfile");
        fs::File::create(&dockerfile).unwrap();

        // Scenario 1: Only root stage file exists
        let root_stage = app_path.join(".env.preprod");
        let mut file = fs::File::create(&root_stage).unwrap();
        writeln!(file, "ENV=root_preprod").unwrap();
        file.flush().unwrap();

        let result = find_env_file(app_path, Some("docker/Dockerfile"), Some("preprod")).unwrap();
        assert_eq!(result, Some(".env.preprod".to_string()));

        // Clean up for next scenario
        fs::remove_file(&root_stage).unwrap();

        // Scenario 2: Only docker base file exists
        let docker_base = docker_dir.join(".env");
        let mut file = fs::File::create(&docker_base).unwrap();
        writeln!(file, "ENV=docker_base").unwrap();
        file.flush().unwrap();

        let result = find_env_file(app_path, Some("docker/Dockerfile"), Some("preprod")).unwrap();
        assert_eq!(result, Some("docker/.env".to_string()));

        // Scenario 3: Only root base file exists
        fs::remove_file(&docker_base).unwrap();
        let root_base = app_path.join(".env");
        let mut file = fs::File::create(&root_base).unwrap();
        writeln!(file, "ENV=root_base").unwrap();
        file.flush().unwrap();

        let result = find_env_file(app_path, Some("docker/Dockerfile"), Some("preprod")).unwrap();
        assert_eq!(result, Some(".env".to_string()));
    }

    /// Test that no env file returns None
    #[test]
    fn test_no_env_file_returns_none() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        // No env files created
        let result = find_env_file(app_path, None, Some("dev")).unwrap();
        assert_eq!(result, None);

        let env_vars = load_env_vars_for_runtime(app_path, None, Some("dev")).unwrap();
        assert!(env_vars.is_empty());
    }

    /// Test all supported stages (dev, qa, preprod, prod)
    #[test]
    fn test_all_stage_types() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        let stages = ["dev", "qa", "preprod", "prod"];

        for stage in &stages {
            // Create stage-specific file
            let stage_file = app_path.join(format!(".env.{}", stage));
            let mut file = fs::File::create(&stage_file).unwrap();
            writeln!(file, "STAGE={}", stage).unwrap();
            file.flush().unwrap();

            // Verify it's found
            let result = find_env_file(app_path, None, Some(stage)).unwrap();
            assert_eq!(result, Some(format!(".env.{}", stage)));

            // Verify vars are loaded
            let env_vars = load_env_vars_for_runtime(app_path, None, Some(stage)).unwrap();
            assert_eq!(env_vars.get("STAGE"), Some(&stage.to_string()));

            // Clean up
            fs::remove_file(&stage_file).unwrap();
        }
    }

    /// Test that dockerfile_path from config is used correctly with stages
    #[test]
    fn test_dockerfile_path_with_nested_directory() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        // Create nested directory structure: build/docker/
        let build_dir = app_path.join("build");
        fs::create_dir(&build_dir).unwrap();
        let docker_dir = build_dir.join("docker");
        fs::create_dir(&docker_dir).unwrap();

        // Create Dockerfile
        let dockerfile = docker_dir.join("Dockerfile");
        fs::File::create(&dockerfile).unwrap();

        // Create stage-specific env file at Dockerfile level
        let docker_stage = docker_dir.join(".env.prod");
        let mut file = fs::File::create(&docker_stage).unwrap();
        writeln!(file, "ENV=docker_prod").unwrap();
        file.flush().unwrap();

        // Should find build/docker/.env.prod
        let result =
            find_env_file(app_path, Some("build/docker/Dockerfile"), Some("prod")).unwrap();
        assert_eq!(result, Some("build/docker/.env.prod".to_string()));

        let env_vars =
            load_env_vars_for_runtime(app_path, Some("build/docker/Dockerfile"), Some("prod"))
                .unwrap();
        assert_eq!(env_vars.get("ENV"), Some(&"docker_prod".to_string()));
    }

    /// Test backward compatibility: existing code without stage parameter still works
    #[test]
    fn test_backward_compatibility_no_stage() {
        let temp_dir = TempDir::new().unwrap();
        let app_path = temp_dir.path();

        // Create docker subdirectory
        let docker_dir = app_path.join("docker");
        fs::create_dir(&docker_dir).unwrap();
        let dockerfile = docker_dir.join("Dockerfile");
        fs::File::create(&dockerfile).unwrap();

        // Create base env files
        let docker_env = docker_dir.join(".env");
        let mut file = fs::File::create(&docker_env).unwrap();
        writeln!(file, "ENV=docker").unwrap();
        file.flush().unwrap();

        let root_env = app_path.join(".env");
        let mut file = fs::File::create(&root_env).unwrap();
        writeln!(file, "ENV=root").unwrap();
        file.flush().unwrap();

        // Without stage, should prioritize docker/.env over root .env
        let result = find_env_file(app_path, Some("docker/Dockerfile"), None).unwrap();
        assert_eq!(result, Some("docker/.env".to_string()));

        let env_vars =
            load_env_vars_for_runtime(app_path, Some("docker/Dockerfile"), None).unwrap();
        assert_eq!(env_vars.get("ENV"), Some(&"docker".to_string()));
    }
}
