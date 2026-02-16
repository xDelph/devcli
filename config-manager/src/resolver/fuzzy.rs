//! Fuzzy matching utilities for resolvers.

use crate::utils::string::find_closest_match;

/// Find a fuzzy match for a target string from a list of candidates.
///
/// Returns the closest match if one exists within the max_distance threshold.
///
/// # Arguments
///
/// * `target` - The string to match
/// * `candidates` - List of candidate strings
/// * `max_distance` - Maximum Levenshtein distance to consider a match
///
/// # Examples
///
/// ```rust
/// use config_manager::resolver::fuzzy_match;
///
/// let candidates = vec!["redis".to_string(), "postgres".to_string(), "mongodb".to_string()];
/// let result = fuzzy_match("redi", &candidates, 2);
/// assert_eq!(result, Some("redis".to_string()));
/// ```
pub fn fuzzy_match(target: &str, candidates: &[String], max_distance: usize) -> Option<String> {
    let candidate_refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
    find_closest_match(target, &candidate_refs, max_distance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match_found() {
        let candidates = vec![
            "redis".to_string(),
            "postgres".to_string(),
            "mongodb".to_string(),
        ];

        assert_eq!(
            fuzzy_match("redi", &candidates, 2),
            Some("redis".to_string())
        );

        assert_eq!(
            fuzzy_match("postgre", &candidates, 2),
            Some("postgres".to_string())
        );
    }

    #[test]
    fn test_fuzzy_match_not_found() {
        let candidates = vec!["redis".to_string(), "postgres".to_string()];

        assert_eq!(fuzzy_match("mysql", &candidates, 2), None);
    }

    #[test]
    fn test_fuzzy_match_exact() {
        let candidates = vec!["redis".to_string(), "postgres".to_string()];

        assert_eq!(
            fuzzy_match("redis", &candidates, 2),
            Some("redis".to_string())
        );
    }
}
