//! Path manipulation utilities.

use std::env;
use std::path::{Path, PathBuf};

/// Expand tilde (~) in a path to the user's home directory.
///
/// # Examples
///
/// ```rust
/// use config_manager::utils::expand_tilde;
/// use std::path::PathBuf;
///
/// let expanded = expand_tilde("~/config.json");
/// // On Unix: /home/username/config.json
/// // On Windows: C:\Users\username\config.json
/// ```
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();

    if path.starts_with("~") {
        if let Some(home) = env::var_os("HOME") {
            let path_str = path.to_string_lossy();

            let without_tilde = if let Some(rest) = path_str.strip_prefix("~/") {
                rest
            } else if let Some(rest) = path_str.strip_prefix('~') {
                rest
            } else {
                &path_str
            };

            let mut result = PathBuf::from(home);
            if !without_tilde.is_empty() {
                result.push(without_tilde);
            }
            return result;
        }
    }

    path.to_path_buf()
}

/// Expand environment variables in a path.
///
/// Supports both `$VAR` and `${VAR}` syntax.
///
/// # Examples
///
/// ```rust
/// use config_manager::utils::expand_env_vars;
/// use std::env;
///
/// env::set_var("CONFIG_DIR", "/etc/myapp");
/// let expanded = expand_env_vars("$CONFIG_DIR/config.json");
/// assert_eq!(expanded.to_str().unwrap(), "/etc/myapp/config.json");
/// ```
pub fn expand_env_vars<P: AsRef<Path>>(path: P) -> PathBuf {
    let path_str = path.as_ref().to_string_lossy();
    let mut result = String::new();
    let chars: Vec<char> = path_str.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '$' {
            i += 1; // Move past '$'

            // Check for ${VAR} syntax
            if i < chars.len() && chars[i] == '{' {
                i += 1; // Move past '{'
                let start = i;

                // Find closing '}'
                while i < chars.len() && chars[i] != '}' {
                    i += 1;
                }

                if i < chars.len() && chars[i] == '}' {
                    let var_name: String = chars[start..i].iter().collect();
                    i += 1; // Move past '}'

                    if let Ok(value) = env::var(&var_name) {
                        result.push_str(&value);
                    } else {
                        // If var doesn't exist, keep original syntax
                        result.push_str(&format!("${{{}}}", var_name));
                    }
                } else {
                    // Unclosed ${, keep as-is
                    result.push_str("${");
                    result.extend(chars[start..i].iter());
                }
            } else {
                // $VAR syntax
                let start = i;

                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }

                let var_name: String = chars[start..i].iter().collect();

                if !var_name.is_empty() {
                    if let Ok(value) = env::var(&var_name) {
                        result.push_str(&value);
                    } else {
                        // If var doesn't exist, keep original syntax
                        result.push_str(&format!("${}", var_name));
                    }
                } else {
                    // Just a '$' followed by non-var char
                    result.push('$');
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    PathBuf::from(result)
}

/// Expand both tilde and environment variables in a path.
///
/// First expands environment variables, then expands tilde.
///
/// # Examples
///
/// ```rust
/// use config_manager::utils::path;
/// use std::env;
///
/// env::set_var("APP", "myapp");
/// let expanded = path::expand_all("~/$APP/config.json");
/// // Result: /home/username/myapp/config.json
/// ```
pub fn expand_all<P: AsRef<Path>>(path: P) -> PathBuf {
    expand_tilde(expand_env_vars(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        // Only test on Unix-like systems where HOME is reliable
        if cfg!(unix) {
            let home = env::var("HOME").unwrap();
            let expanded = expand_tilde("~/test.json");
            assert_eq!(expanded, PathBuf::from(format!("{}/test.json", home)));
        }
    }

    #[test]
    fn test_expand_env_vars() {
        env::set_var("TEST_VAR", "test_value");

        let expanded = expand_env_vars("$TEST_VAR/config.json");
        assert_eq!(expanded, PathBuf::from("test_value/config.json"));

        let expanded = expand_env_vars("${TEST_VAR}/config.json");
        assert_eq!(expanded, PathBuf::from("test_value/config.json"));
    }

    #[test]
    fn test_expand_env_vars_missing() {
        let expanded = expand_env_vars("$MISSING_VAR/config.json");
        assert_eq!(expanded, PathBuf::from("$MISSING_VAR/config.json"));
    }

    #[test]
    fn test_no_expansion_needed() {
        let path = "/absolute/path/config.json";
        let expanded = expand_tilde(path);
        assert_eq!(expanded, PathBuf::from(path));
    }
}
