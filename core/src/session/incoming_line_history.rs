use std::{
    collections::{HashSet, VecDeque},
    ops::Deref,
    sync::Arc,
};

use regex::Regex;

use super::styled_line::StyledLine;

pub struct IncomingLineHistory {
    max_len: usize,
    lines: VecDeque<Arc<StyledLine>>,
    line_terminated: bool,
}

impl IncomingLineHistory {
    pub fn new() -> Self {
        IncomingLineHistory {
            max_len: 10000,
            lines: VecDeque::new(),
            line_terminated: false,
        }
    }

    pub fn commit_current_line(&mut self) {
        self.line_terminated = true;
    }

    pub fn extend_line(&mut self, line_in: Arc<StyledLine>) {
        if self.line_terminated {
            self.line_terminated = false;

            while self.lines.len() > (self.max_len - 1) {
                self.lines.pop_front();
            }
            self.lines.push_back(line_in);
        } else {
            match self.lines.pop_back() {
                Some(line) => {
                    self.lines.push_back(line);
                }
                None => {
                    self.lines.push_back(line_in);
                }
            }
        }
    }

    /// Finds the most recent word matching the given prefix.
    ///
    /// # Arguments
    /// * `prefix` - The prefix to match against (case-insensitive)
    /// * `skip_words_in` - Optional set of words to ignore in the search
    /// * `n_recent_lines` - Number of recent lines to search through
    ///
    /// # Returns
    /// * `Option<String>` - The matching word if found, or None otherwise
    pub fn find_recent_word_by_prefix(
        &self,
        prefix: &str,
        skip_words_in: Option<&HashSet<String>>,
        n_recent_lines: usize,
    ) -> Option<String> {
        let lowercase_prefix = prefix.to_lowercase();
        
        self.lines
            .iter()
            .rev()
            .take(n_recent_lines)
            .find_map(|line| {
                // Split line by whitespace to get words
                for raw_word in line.text.split_whitespace() {
                    // Clean the word by trimming non-alphanumeric chars from start/end
                    let word = raw_word.trim_matches(|c: char| !c.is_alphanumeric());
                    
                    // Skip empty words
                    if word.is_empty() {
                        continue;
                    }
                    
                    // Skip if word is in the exclusion set
                    if let Some(history) = skip_words_in {
                        if history.contains(word) {
                            continue;
                        }
                    }
                    
                    // Return the word if it starts with the prefix (case-insensitive)
                    if word.to_lowercase().starts_with(&lowercase_prefix) {
                        return Some(word.to_string());
                    }
                }
                None
            })
    }
}
