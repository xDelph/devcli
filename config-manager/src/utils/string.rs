//! String manipulation utilities.

/// Calculate the Levenshtein distance between two strings.
///
/// The Levenshtein distance is the minimum number of single-character edits
/// (insertions, deletions, or substitutions) required to transform one string
/// into another.
///
/// # Examples
///
/// ```rust
/// use config_manager::utils::levenshtein_distance;
///
/// assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
/// assert_eq!(levenshtein_distance("saturday", "sunday"), 3);
/// assert_eq!(levenshtein_distance("api", "api"), 0);
/// ```
pub fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();

    // Early returns for edge cases
    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    // Create a matrix to store distances
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    // Initialize first column (distance from empty string)
    #[allow(clippy::needless_range_loop)]
    for i in 0..=len1 {
        matrix[i][0] = i;
    }

    // Initialize first row (distance from empty string)
    #[allow(clippy::needless_range_loop)]
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    // Fill in the rest of the matrix
    for (i, c1) in s1.chars().enumerate() {
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };

            matrix[i + 1][j + 1] = std::cmp::min(
                std::cmp::min(
                    matrix[i][j + 1] + 1, // deletion
                    matrix[i + 1][j] + 1, // insertion
                ),
                matrix[i][j] + cost, // substitution or match
            );
        }
    }

    matrix[len1][len2]
}

/// Find the closest match from a list of candidates.
///
/// Returns the candidate with the smallest Levenshtein distance
/// if it's within the max_distance threshold.
///
/// # Examples
///
/// ```rust
/// use config_manager::utils::string::find_closest_match;
///
/// let candidates = vec!["apple", "banana", "cherry"];
/// assert_eq!(find_closest_match("aple", &candidates, 2), Some("apple".to_string()));
/// assert_eq!(find_closest_match("xyz", &candidates, 2), None);
/// ```
pub fn find_closest_match(
    target: &str,
    candidates: &[&str],
    max_distance: usize,
) -> Option<String> {
    candidates
        .iter()
        .map(|&candidate| (candidate, levenshtein_distance(target, candidate)))
        .filter(|(_, distance)| *distance <= max_distance)
        .min_by_key(|(_, distance)| *distance)
        .map(|(candidate, _)| candidate.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_identical() {
        assert_eq!(levenshtein_distance("test", "test"), 0);
    }

    #[test]
    fn test_levenshtein_empty() {
        assert_eq!(levenshtein_distance("", "test"), 4);
        assert_eq!(levenshtein_distance("test", ""), 4);
        assert_eq!(levenshtein_distance("", ""), 0);
    }

    #[test]
    fn test_levenshtein_single_edit() {
        assert_eq!(levenshtein_distance("cat", "hat"), 1); // substitution
        assert_eq!(levenshtein_distance("cat", "cats"), 1); // insertion
        assert_eq!(levenshtein_distance("cats", "cat"), 1); // deletion
    }

    #[test]
    fn test_levenshtein_multiple_edits() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("saturday", "sunday"), 3);
    }

    #[test]
    fn test_find_closest_match() {
        let candidates = vec!["apple", "banana", "cherry", "date"];

        assert_eq!(
            find_closest_match("aple", &candidates, 2),
            Some("apple".to_string())
        );

        assert_eq!(
            find_closest_match("banan", &candidates, 2),
            Some("banana".to_string())
        );

        // No match within distance
        assert_eq!(find_closest_match("xyz", &candidates, 2), None);

        // Exact match
        assert_eq!(
            find_closest_match("date", &candidates, 2),
            Some("date".to_string())
        );
    }

    #[test]
    fn test_find_closest_match_tie() {
        // When there's a tie, should return the first one
        let candidates = vec!["cat", "hat", "bat"];
        let result = find_closest_match("rat", &candidates, 2);
        assert!(result.is_some());
        // Could be any of them, all have distance 1
    }
}
