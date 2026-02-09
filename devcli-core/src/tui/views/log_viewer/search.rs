use super::file_loader::LogLine;

/// Represents a match within a line for highlighting purposes
#[derive(Debug, Clone)]
pub struct MatchRange {
    /// Start position of the match in the line
    pub start: usize,
    /// End position of the match in the line
    pub end: usize,
}

#[derive(Default)]
pub struct SearchState {
    /// Whether search mode is active
    pub active: bool,
    /// Current search query string
    pub query: String,
    /// Line indices that match the search query
    pub results: Vec<usize>,
    /// Index of the currently highlighted search result
    pub current_idx: usize,
    /// Match ranges for each result line (for highlighting)
    pub match_ranges: std::collections::HashMap<usize, Vec<MatchRange>>,
}

impl SearchState {
    /// Performs a search for the current query
    /// Updates results with matching line indices and match ranges for highlighting
    /// Uses smart matching: word boundaries for alphanumeric queries, substring for others
    pub fn perform_search(&mut self, content: &[LogLine]) -> Option<usize> {
        self.results.clear();
        self.match_ranges.clear();
        self.current_idx = 0;

        if self.query.is_empty() {
            return None;
        }

        // Search for the query in each line (case-insensitive)
        let query_lower = self.query.to_lowercase();

        self.results.reserve(content.len() / 20);

        for (idx, log_line) in content.iter().enumerate() {
            // Reconstruct the text from formatted spans to ensure consistency with highlighting
            let formatted_text: String = log_line
                .formatted
                .iter()
                .map(|s| s.content.as_ref())
                .collect();
            let line_lower = formatted_text.to_lowercase();

            // Find all matches in this line
            let matches = self.find_matches_in_line(&line_lower, &query_lower);

            if !matches.is_empty() {
                self.results.push(idx);
                self.match_ranges.insert(idx, matches);
            }
        }

        // Return the first result index if any
        if !self.results.is_empty() {
            Some(self.results[0])
        } else {
            None
        }
    }

    /// Finds all matches of the query in a line using substring matching
    ///
    /// # Arguments
    /// * `line_lower` - The line text in lowercase for case-insensitive matching
    /// * `query_lower` - The search query in lowercase
    ///
    /// # Returns
    /// Vector of MatchRange structs indicating character positions of matches
    fn find_matches_in_line(&self, line_lower: &str, query_lower: &str) -> Vec<MatchRange> {
        let mut matches = Vec::new();
        let mut start_pos = 0;

        // Find all substring matches in the line
        while let Some(match_pos) = line_lower[start_pos..].find(query_lower) {
            let absolute_pos = start_pos + match_pos;
            matches.push(MatchRange {
                start: absolute_pos,
                end: absolute_pos + query_lower.len(),
            });
            start_pos = absolute_pos + 1; // Move past this match to find overlapping matches
        }

        matches
    }

    /// Jumps to the next search result
    pub fn next_result(&mut self) -> Option<usize> {
        if self.results.is_empty() {
            return None;
        }

        self.current_idx = (self.current_idx + 1) % self.results.len();
        Some(self.results[self.current_idx])
    }

    /// Jumps to the previous search result
    pub fn previous_result(&mut self) -> Option<usize> {
        if self.results.is_empty() {
            return None;
        }

        if self.current_idx == 0 {
            self.current_idx = self.results.len() - 1;
        } else {
            self.current_idx -= 1;
        }
        Some(self.results[self.current_idx])
    }

    /// Checks if a line index is in the search results
    pub fn is_match(&self, line_idx: usize) -> bool {
        self.results.contains(&line_idx)
    }

    /// Gets the match ranges for a specific line (for highlighting)
    pub fn get_match_ranges(&self, line_idx: usize) -> Option<&Vec<MatchRange>> {
        self.match_ranges.get(&line_idx)
    }

    /// Checks if a line is the currently selected search result
    pub fn is_current_result(&self, line_idx: usize) -> bool {
        if self.results.is_empty() {
            return false;
        }
        self.results.get(self.current_idx) == Some(&line_idx)
    }

    /// Clears all search results and highlighting
    /// Called when exiting search mode to remove visual highlights
    pub fn clear_results(&mut self) {
        self.results.clear();
        self.match_ranges.clear();
        self.current_idx = 0;
    }
}
