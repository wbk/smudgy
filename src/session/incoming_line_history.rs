use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

use regex::Regex;

use super::StyledLine;

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

    pub fn find_recent_word_by_prefix(
        &self,
        prefix: &str,
        skip_words_in: Option<&HashSet<String>>,
        n_recent_lines: usize,
    ) -> Option<String> {
        let lowercase_prefix = prefix.to_lowercase();
        let found = self
            .lines
            .iter()
            .rev()
            .take(n_recent_lines)
            .flat_map(|line| line.as_str().split_whitespace())
            .find_map(|str| {
                if let Some(history) = skip_words_in {
                    if history.contains(str) {
                        return None;
                    }
                }

                str.to_lowercase()
                    .starts_with(lowercase_prefix.as_str())
                    .then_some(str.to_string())
            });
        found
    }
}
