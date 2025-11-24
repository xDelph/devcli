use super::file_loader::LogLine;

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
}

impl SearchState {
    /// Performs a search for the current query
    /// Updates results with matching line indices
    pub fn perform_search(&mut self, content: &[LogLine]) -> Option<usize> {
        self.results.clear();
        self.current_idx = 0;

        if self.query.is_empty() {
            return None;
        }

        // Search for the query in each line (case-insensitive)
        let query_lower = self.query.to_lowercase();

        self.results.reserve(content.len() / 20);

        for (idx, log_line) in content.iter().enumerate() {
            if log_line.raw.to_lowercase().contains(&query_lower) {
                self.results.push(idx);
            }
        }

        // Return the first result index if any
        if !self.results.is_empty() {
            Some(self.results[0])
        } else {
            None
        }
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
}
