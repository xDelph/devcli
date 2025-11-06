// Unit tests for validation module
// Tests app name validation logic and rules

#[cfg(test)]
mod tests {
    use crate::commands::auto_add::validation::validate_app_name;

    #[test]
    fn test_validate_app_name_valid_names() {
        // Test that valid names are accepted
        assert!(validate_app_name("valid-app-name").is_ok());
        assert!(validate_app_name("redis.local").is_ok());
        assert!(validate_app_name("traefik-dynamic").is_ok());
        assert!(validate_app_name("my_app").is_ok());
        assert!(validate_app_name("app123").is_ok());
        assert!(validate_app_name("simple").is_ok());
    }

    #[test]
    fn test_validate_app_name_empty_name() {
        // Test that empty names are rejected
        let result = validate_app_name("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_app_name_spaces() {
        // Test that names with spaces are rejected
        let result = validate_app_name("invalid app name");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot contain spaces"));
    }

    #[test]
    fn test_validate_app_name_path_separators() {
        // Test that names with path separators are rejected
        let result = validate_app_name("invalid/app");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot contain path separators"));

        let result = validate_app_name("invalid\\app");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot contain path separators"));
    }

    #[test]
    fn test_validate_app_name_edge_cases() {
        // Test edge cases
        assert!(validate_app_name("a").is_ok()); // Single character
        assert!(validate_app_name("app-with-many-dashes").is_ok());
        assert!(validate_app_name("app_with_underscores").is_ok());
        assert!(validate_app_name("app.with.dots").is_ok());
        
        // Multiple spaces should be rejected
        let result = validate_app_name("app  with  spaces");
        assert!(result.is_err());
    }
}