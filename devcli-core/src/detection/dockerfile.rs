// Dockerfile parser module
// Extracts stage names from multi-stage Docker builds
// Parses Dockerfile to find "FROM ... AS stage-name" patterns

use crate::Result;
use std::fs;
use std::path::Path;

// Represents a build stage in a Dockerfile
// Each stage has a name and can be built separately
#[derive(Debug, Clone)]
pub struct DockerStage {
    pub name: String, // Stage name (e.g., "test", "build", "production")
}

// Parse a Dockerfile and extract all build stages
// Args:
//   - dockerfile_path: Path to the Dockerfile
// Returns: List of stage names found in the Dockerfile
//
// Examples of FROM statements we need to parse:
//   - FROM node:18 AS build
//   - FROM python:3.11-slim as test
//   - FROM node:18 as production
//   - FROM nginx:alpine AS runner
pub fn parse_dockerfile(dockerfile_path: &Path) -> Result<Vec<DockerStage>> {
    // Read the Dockerfile contents
    let content = fs::read_to_string(dockerfile_path)?;

    let mut stages = Vec::new();

    // Process each line of the Dockerfile
    for line in content.lines() {
        // Remove leading/trailing whitespace
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Convert to uppercase for case-insensitive matching
        let line_upper = line.to_uppercase();

        // Check if this line is a FROM statement
        // FROM statements can have stage names with "AS stage-name"
        if line_upper.starts_with("FROM ") {
            // Split the line into parts
            // Example: "FROM node:18 AS build" -> ["FROM", "node:18", "AS", "build"]
            let parts: Vec<&str> = line.split_whitespace().collect();

            // Look for "AS" keyword (case-insensitive)
            // The stage name comes right after "AS"
            for i in 0..parts.len() {
                if parts[i].to_uppercase() == "AS" && i + 1 < parts.len() {
                    // Found a stage! The next part is the stage name
                    let stage_name = parts[i + 1].to_string();

                    stages.push(DockerStage { name: stage_name });
                    break;
                }
            }
        }
    }

    Ok(stages)
}

// Check if a Dockerfile exists and has multi-stage builds
// Args:
//   - dockerfile_path: Path to the Dockerfile
// Returns: true if the file exists and has at least one named stage
#[allow(dead_code)]
pub fn has_multi_stage_build(dockerfile_path: &Path) -> bool {
    if !dockerfile_path.exists() {
        return false;
    }

    // Try to parse the Dockerfile
    if let Ok(stages) = parse_dockerfile(dockerfile_path) {
        // Multi-stage if we found at least one named stage
        !stages.is_empty()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Helper to create a temporary Dockerfile for testing
    fn create_test_dockerfile(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn test_parse_single_stage() {
        let dockerfile = create_test_dockerfile("FROM node:18 AS build\nRUN npm install\n");

        let stages = parse_dockerfile(dockerfile.path()).unwrap();
        assert_eq!(stages.len(), 1);
        assert_eq!(stages[0].name, "build");
    }

    #[test]
    fn test_parse_multi_stage() {
        let dockerfile = create_test_dockerfile(
            r#"
FROM node:18 AS build
RUN npm install
RUN npm run build

FROM node:18 AS test
RUN npm test

FROM nginx:alpine AS production
COPY --from=build /app/dist /usr/share/nginx/html
"#,
        );

        let stages = parse_dockerfile(dockerfile.path()).unwrap();
        assert_eq!(stages.len(), 3);
        assert_eq!(stages[0].name, "build");
        assert_eq!(stages[1].name, "test");
        assert_eq!(stages[2].name, "production");
    }

    #[test]
    fn test_parse_case_insensitive() {
        let dockerfile =
            create_test_dockerfile("from node:18 as build\nFROM python:3.11 As test\n");

        let stages = parse_dockerfile(dockerfile.path()).unwrap();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].name, "build");
        assert_eq!(stages[1].name, "test");
    }

    #[test]
    fn test_parse_with_comments() {
        let dockerfile = create_test_dockerfile(
            r#"
# Build stage
FROM node:18 AS build
RUN npm install
# This is a comment
FROM nginx:alpine AS production
"#,
        );

        let stages = parse_dockerfile(dockerfile.path()).unwrap();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].name, "build");
        assert_eq!(stages[1].name, "production");
    }

    #[test]
    fn test_parse_no_stages() {
        let dockerfile = create_test_dockerfile("FROM node:18\nRUN npm install\n");

        let stages = parse_dockerfile(dockerfile.path()).unwrap();
        assert_eq!(stages.len(), 0);
    }

    #[test]
    fn test_has_multi_stage_build() {
        let dockerfile = create_test_dockerfile("FROM node:18 AS build\nRUN npm install\n");

        assert!(has_multi_stage_build(dockerfile.path()));
    }

    #[test]
    fn test_has_multi_stage_build_single_stage() {
        let dockerfile = create_test_dockerfile("FROM node:18\nRUN npm install\n");

        assert!(!has_multi_stage_build(dockerfile.path()));
    }
}
