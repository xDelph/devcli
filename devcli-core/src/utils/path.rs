// Path expansion utilities
// Handles ~ (tilde) and environment variable expansion in file paths
//
// Example transformations:
// - "~/Projects/app" → "/Users/username/Projects/app"
// - "$HOME/Projects/app" → "/Users/username/Projects/app"
// - "~/$PROJECT/app" → "/Users/username/myproject/app"

use std::env;
use std::path::{Path, PathBuf};

// Expand tilde (~) in a path to the user's home directory
// Example: "~/Projects/app" becomes "/Users/username/Projects/app"
//
// Generic parameter <P: AsRef<Path>> means:
// - This function accepts any type P that can be converted to a Path reference
// - Works with &str, String, PathBuf, &Path, etc.
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf {
    // Convert the input to a Path reference
    // .as_ref() does the conversion
    let path = path.as_ref();

    // Check if the path starts with ~
    if path.starts_with("~") {
        // Try to get the HOME environment variable
        // env::var_os returns Option<OsString>
        // - Some(value) if HOME is set
        // - None if HOME is not set
        if let Some(home) = env::var_os("HOME") {
            // Convert the path to a string for manipulation
            // .to_string_lossy() converts Path to Cow<str>
            // "lossy" means invalid UTF-8 becomes �, but that's rare
            let path_str = path.to_string_lossy();

            // Remove the ~ and optionally the following /
            // "~/foo/bar" → "foo/bar"
            // "~" → ""
            let without_tilde = if let Some(rest) = path_str.strip_prefix("~/") {
                // Path was "~/something" - we have "something"
                rest
            } else if let Some(rest) = path_str.strip_prefix("~") {
                // Path was just "~" - we have ""
                rest
            } else {
                // Shouldn't happen since we checked starts_with above
                &path_str
            };

            // Build the full path: HOME + remaining
            // Example: /Users/username + foo/bar = /Users/username/foo/bar
            let mut expanded = PathBuf::from(home);
            if !without_tilde.is_empty() {
                expanded.push(without_tilde);
            }
            return expanded;
        }
    }

    // If path doesn't start with ~ or HOME is not set, return as-is
    // .to_path_buf() converts Path to PathBuf (owned version)
    path.to_path_buf()
}

// Expand environment variables in a string
// Example: "$HOME/Projects" becomes "/Users/username/Projects"
// Example: "$PROJECT_DIR/src" becomes "/path/to/project/src"
//
// &str = borrowed string slice (we don't take ownership)
// -> String = returns an owned String
pub fn expand_env_vars(s: &str) -> String {
    // Start with a copy of the input string
    let mut result = s.to_string();

    // Get all environment variables as (key, value) pairs
    // env::vars() returns an iterator over all environment variables
    for (key, value) in env::vars() {
        // Build the pattern to search for: $KEY
        // format! creates a new String from a template
        let pattern = format!("${}", key);

        // Replace all occurrences of $KEY with its value
        // Example: if HOME=/Users/alice, replace "$HOME" with "/Users/alice"
        result = result.replace(&pattern, &value);
    }

    result
}

// Expand both environment variables AND tilde in a path
// This is the "do everything" function - use this for config paths
//
// Example: "~/$PROJECT/app"
// 1. expand_env_vars: "~/$PROJECT/app" → "~/myproject/app"
// 2. expand_tilde: "~/myproject/app" → "/Users/username/myproject/app"
pub fn expand_path(path: &str) -> PathBuf {
    // First expand environment variables
    let expanded_env = expand_env_vars(path);

    // Then expand tilde
    // The result of expand_env_vars is a String, which implements AsRef<Path>
    expand_tilde(expanded_env)
}

// Contract absolute paths back to tilde notation for storage
// Example: "/Users/username/Projects/app" becomes "~/Projects/app"
// This makes configs more portable between machines
pub fn contract_tilde<P: AsRef<Path>>(path: P) -> String {
    let path = path.as_ref();

    // Try to get the HOME environment variable
    if let Some(home) = env::var_os("HOME") {
        let home_path = PathBuf::from(home);

        // Check if the path starts with the home directory
        if let Ok(relative) = path.strip_prefix(&home_path) {
            // Convert the relative path back to tilde notation
            if relative.as_os_str().is_empty() {
                // Path is exactly the home directory
                return "~".to_string();
            } else {
                // Path is under home directory
                return format!("~/{}", relative.display());
            }
        }
    }

    // If path is not under home directory, return as-is
    path.display().to_string()
}

// Unit tests to verify the functions work correctly
// #[cfg(test)] means this code only compiles when running tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        // Get the actual HOME value
        let home = env::var("HOME").unwrap();

        // Test that ~/test/path expands correctly
        let expanded = expand_tilde("~/test/path");

        // Assert that the result matches what we expect
        assert_eq!(expanded, PathBuf::from(format!("{}/test/path", home)));
    }

    #[test]
    fn test_expand_env_vars() {
        // Set a test environment variable
        env::set_var("TEST_VAR", "test_value");

        // Test that $TEST_VAR gets replaced
        let result = expand_env_vars("$TEST_VAR/path");

        // Verify the result
        assert_eq!(result, "test_value/path");
    }

    #[test]
    fn test_contract_tilde() {
        // Get the actual HOME value
        let home = env::var("HOME").unwrap();

        // Test that absolute paths under home get contracted
        let absolute_path = format!("{}/test/path", home);
        let contracted = contract_tilde(&absolute_path);

        // Should become ~/test/path
        assert_eq!(contracted, "~/test/path");

        // Test that paths outside home stay absolute
        let outside_path = "/usr/local/bin";
        let contracted_outside = contract_tilde(outside_path);
        assert_eq!(contracted_outside, "/usr/local/bin");
    }
}
