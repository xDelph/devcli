// Tests for command utility functions

use super::command::*;
use std::collections::HashMap;

#[test]
fn test_inject_docker_platform_run() {
    let cmd = "docker run myimage";
    let result = inject_docker_platform(cmd, "linux/amd64");
    assert_eq!(result, "docker run --platform linux/amd64 myimage");
}

#[test]
fn test_inject_docker_platform_build() {
    let cmd = "docker build -t myimage .";
    let result = inject_docker_platform(cmd, "linux/arm64");
    assert_eq!(result, "docker build --platform linux/arm64 -t myimage .");
}

#[test]
fn test_inject_docker_platform_already_present() {
    let cmd = "docker run --platform linux/amd64 myimage";
    let result = inject_docker_platform(cmd, "linux/arm64");
    assert_eq!(result, cmd); // Should not modify
}

#[test]
fn test_inject_docker_platform_non_docker() {
    let cmd = "npm start";
    let result = inject_docker_platform(cmd, "linux/amd64");
    assert_eq!(result, cmd); // Should not modify
}

#[test]
fn test_inject_docker_platform_unsupported_subcommand() {
    let cmd = "docker ps";
    let result = inject_docker_platform(cmd, "linux/amd64");
    assert_eq!(result, cmd); // Should not modify
}

#[test]
fn test_inject_docker_env_file() {
    let cmd = "docker run myimage";
    let result = inject_docker_env_file(cmd, ".env");
    assert_eq!(result, "docker run --env-file .env myimage");
}

#[test]
fn test_inject_docker_env_file_with_flags() {
    let cmd = "docker run --name api --rm -p 3000:3000 myimage";
    let result = inject_docker_env_file(cmd, ".env");
    assert_eq!(
        result,
        "docker run --name api --rm -p 3000:3000 --env-file .env myimage"
    );
}

#[test]
fn test_inject_dockerfile_path_build() {
    let cmd = "docker build -t myimage .";
    let result = inject_dockerfile_path(cmd, "docker/Dockerfile");
    assert_eq!(result, "docker build -f docker/Dockerfile -t myimage .");
}

#[test]
fn test_inject_dockerfile_path_already_present() {
    let cmd = "docker build -f custom/Dockerfile -t myimage .";
    let result = inject_dockerfile_path(cmd, "docker/Dockerfile");
    assert_eq!(result, cmd); // Should not modify
}

#[test]
fn test_inject_dockerfile_path_non_build() {
    let cmd = "docker run myimage";
    let result = inject_dockerfile_path(cmd, "docker/Dockerfile");
    assert_eq!(result, cmd); // Should not modify
}

#[test]
fn test_inject_docker_env_file_already_present() {
    let cmd = "docker run --env-file .env myimage";
    let result = inject_docker_env_file(cmd, ".env");
    assert_eq!(result, cmd); // Should not modify
}

#[test]
fn test_inject_docker_env_file_non_docker() {
    let cmd = "npm start";
    let result = inject_docker_env_file(cmd, ".env");
    assert_eq!(result, cmd); // Should not modify
}

#[test]
fn test_inject_orbstack_env_vars() {
    let cmd = "docker run myimage";
    let mut env_vars = HashMap::new();
    env_vars.insert("NODE_ENV".to_string(), "production".to_string());
    env_vars.insert("PORT".to_string(), "3000".to_string());

    let result = inject_orbstack_env_vars(cmd, &env_vars);
    // Should have env vars as prefix with quotes (sorted alphabetically)
    assert_eq!(
        result,
        "NODE_ENV=\"production\" PORT=\"3000\" docker run myimage"
    );
}

#[test]
fn test_inject_orbstack_env_vars_empty() {
    let cmd = "docker run myimage";
    let env_vars = HashMap::new();

    let result = inject_orbstack_env_vars(cmd, &env_vars);
    assert_eq!(result, cmd); // Should not modify
}

#[test]
fn test_inject_orbstack_env_vars_single() {
    let cmd = "docker run myimage";
    let mut env_vars = HashMap::new();
    env_vars.insert("DEBUG".to_string(), "true".to_string());

    let result = inject_orbstack_env_vars(cmd, &env_vars);
    assert_eq!(result, "DEBUG=\"true\" docker run myimage");
}

// Additional tests for --env-file injection with --context flag

#[test]
fn test_inject_docker_env_file_with_context_orbstack() {
    let cmd = "docker --context orbstack run --name luce-api --rm -p 3000:3000 luce-api";
    let result = inject_docker_env_file(cmd, "docker/.env");
    assert_eq!(
        result,
        "docker --context orbstack run --name luce-api --rm -p 3000:3000 --env-file docker/.env luce-api"
    );
}

#[test]
fn test_inject_docker_env_file_with_context_default() {
    let cmd = "docker --context default run --name api --rm -p 8000:8000 api";
    let result = inject_docker_env_file(cmd, ".env");
    assert_eq!(
        result,
        "docker --context default run --name api --rm -p 8000:8000 --env-file .env api"
    );
}

#[test]
fn test_inject_docker_env_file_create_command() {
    let cmd = "docker create --name test myimage";
    let result = inject_docker_env_file(cmd, ".env");
    assert_eq!(result, "docker create --name test --env-file .env myimage");
}

#[test]
fn test_inject_docker_env_file_build_command_no_injection() {
    // build commands should NOT get --env-file
    let cmd = "docker build -t myimage .";
    let result = inject_docker_env_file(cmd, ".env");
    assert_eq!(result, cmd); // Should not modify
}

#[test]
fn test_inject_dockerfile_path_with_context() {
    let cmd = "docker --context orbstack build -t myimage .";
    let result = inject_dockerfile_path(cmd, "docker/Dockerfile");
    assert_eq!(
        result,
        "docker --context orbstack build -f docker/Dockerfile -t myimage ."
    );
}

#[test]
fn test_inject_dockerfile_path_with_existing_flags() {
    let cmd = "docker build -t myimage --no-cache .";
    let result = inject_dockerfile_path(cmd, "docker/Dockerfile");
    assert_eq!(
        result,
        "docker build -f docker/Dockerfile -t myimage --no-cache ."
    );
}

#[test]
fn test_inject_docker_platform_with_context() {
    let cmd = "docker --context orbstack run myimage";
    let result = inject_docker_platform(cmd, "linux/amd64");
    assert_eq!(
        result,
        "docker --context orbstack run --platform linux/amd64 myimage"
    );
}

#[test]
fn test_multiple_injections_combined() {
    // Test that multiple injections work together
    let cmd = "docker --context orbstack run --name api --rm -p 3000:3000 myimage";

    // First inject platform
    let cmd = inject_docker_platform(cmd, "linux/amd64");
    assert_eq!(
        cmd,
        "docker --context orbstack run --platform linux/amd64 --name api --rm -p 3000:3000 myimage"
    );

    // Then inject env-file
    let cmd = inject_docker_env_file(&cmd, ".env");
    assert_eq!(
        cmd,
        "docker --context orbstack run --platform linux/amd64 --name api --rm -p 3000:3000 --env-file .env myimage"
    );
}

#[test]
fn test_inject_docker_env_file_complex_image_name() {
    // Test with image names that have tags or registry paths
    let cmd = "docker run --rm myregistry.io/myimage:v1.0";
    let result = inject_docker_env_file(cmd, ".env");
    assert_eq!(
        result,
        "docker run --rm --env-file .env myregistry.io/myimage:v1.0"
    );
}

#[test]
fn test_inject_docker_env_file_with_volume_mounts() {
    let cmd = "docker run -v /host:/container --name api myimage";
    let result = inject_docker_env_file(cmd, ".env");
    assert_eq!(
        result,
        "docker run -v /host:/container --name api --env-file .env myimage"
    );
}
