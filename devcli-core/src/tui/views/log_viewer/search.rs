use super::file_loader::LogLine;

/// Case-fold one character, keeping a 1:1 mapping with the displayed text.
fn fold_case(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

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
    /// Whether the query is being typed (every printable key goes into it).
    /// Once confirmed (Enter) the search stays, with its results, until
    /// cleared (Esc) or replaced (`/`).
    pub active: bool,
    /// Current search query string (empty: no search)
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
        self.current_idx = 0;
        self.rescan_from(content, 0);
        self.results.first().copied()
    }

    /// Whether there is a search (typed or confirmed).
    pub fn has_query(&self) -> bool {
        !self.query.is_empty()
    }

    /// Match lines `start..` of `content` again (they were appended or
    /// re-parsed), keeping the results before `start` and the current
    /// result. The matches used to be computed once, when the query was
    /// typed: new matching lines were neither counted nor highlighted, and a
    /// reloaded file kept highlights at stale positions.
    pub fn rescan_from(&mut self, content: &[LogLine], start: usize) {
        let current = self.results.get(self.current_idx).copied();
        self.results.retain(|&idx| idx < start);
        self.match_ranges.retain(|&idx, _| idx < start);
        if self.query.is_empty() {
            self.current_idx = 0;
            return;
        }

        // Search for the query in each line (case-insensitive)
        let query_lower: Vec<char> = self.query.chars().map(fold_case).collect();
        for (idx, log_line) in content.iter().enumerate().skip(start) {
            // Reconstruct the text from formatted spans to ensure consistency with highlighting
            let formatted_text: String = log_line
                .formatted
                .iter()
                .map(|s| s.content.as_ref())
                .collect();
            let line_lower: Vec<char> = formatted_text.chars().map(fold_case).collect();

            // Find all matches in this line
            let matches = self.find_matches_in_line(&line_lower, &query_lower);

            if !matches.is_empty() {
                self.results.push(idx);
                self.match_ranges.insert(idx, matches);
            }
        }

        // The current result stays the same line (or the next one after it).
        self.current_idx = current
            .and_then(|line| self.results.iter().position(|&idx| idx >= line))
            .unwrap_or(0);
    }

    /// Finds all matches of the query in a line using substring matching
    ///
    /// Both sides are case-folded one character at a time, so positions are
    /// *character* indices into the displayed line - what the highlighter
    /// uses. They used to be byte offsets into `to_lowercase()`'s output,
    /// which drifted after any multi-byte character (and lowercasing can even
    /// change byte lengths). Matches do not overlap: overlapping ranges made
    /// the highlighter emit the shared text twice ("aa" in "aaaa" rendered
    /// six characters).
    fn find_matches_in_line(&self, line_lower: &[char], query_lower: &[char]) -> Vec<MatchRange> {
        let mut matches = Vec::new();
        if query_lower.is_empty() || query_lower.len() > line_lower.len() {
            return matches;
        }

        let mut start = 0;
        while start + query_lower.len() <= line_lower.len() {
            if line_lower[start..start + query_lower.len()] == *query_lower {
                matches.push(MatchRange {
                    start,
                    end: start + query_lower.len(),
                });
                start += query_lower.len();
            } else {
                start += 1;
            }
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
        // O(1): called for every rendered line, `results.contains` made each
        // frame O(lines x matches).
        self.match_ranges.contains_key(&line_idx)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ranges(line: &str, query: &str) -> Vec<(usize, usize)> {
        let state = SearchState::default();
        let line: Vec<char> = line.chars().map(fold_case).collect();
        let query: Vec<char> = query.chars().map(fold_case).collect();
        state
            .find_matches_in_line(&line, &query)
            .into_iter()
            .map(|m| (m.start, m.end))
            .collect()
    }

    #[test]
    fn match_positions_are_character_indices() {
        // "é" is two bytes: byte offsets would put the match at 7, not 6.
        assert_eq!(ranges("café: ERROR", "error"), vec![(6, 11)]);
    }

    #[test]
    fn matches_do_not_overlap() {
        assert_eq!(ranges("aaaa", "aa"), vec![(0, 2), (2, 4)]);
    }
}
