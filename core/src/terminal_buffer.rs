use iced::Background;
use iced::widget::text::Span;
use selection::Selection;
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

use crate::session::runtime::line_operation::LineOperation;
use crate::session::styled_line::{Style, StyledLine, VtSpan};
use std::collections::{HashSet, VecDeque};
use std::num::NonZeroUsize;

type Link = ();

pub mod selection;

#[inline]
fn to_spans(styled_line: &Arc<StyledLine>) -> Rc<Vec<Span<'static, Link>>> {
    Rc::new(
        styled_line
            .spans
            .iter()
            .map(|span_info| {
                let owned_text =
                    styled_line.text[span_info.begin_pos..span_info.end_pos].to_string();
                Span::<'static, Link>::new(Cow::Owned(owned_text))
                    .color(span_info.style.fg)
                    .background(Background::Color(span_info.style.bg.into()))
            })
            .collect(),
    )
}

impl AsRef<[Span<'static, ()>]> for BufferLine {
    fn as_ref(&self) -> &[Span<'static, ()>] {
        self.spans.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct BufferLine {
    pub styled_line: Arc<StyledLine>,
    pub spans: Rc<Vec<Span<'static, ()>>>,
    pub gagged: bool,
}

impl PartialEq for BufferLine {
    fn eq(&self, other: &Self) -> bool {
        self.styled_line == other.styled_line
    }
}

impl From<Arc<StyledLine>> for BufferLine {
    fn from(styled_line: Arc<StyledLine>) -> Self {
        Self {
            spans: to_spans(&styled_line),
            styled_line,
            gagged: false,
        }
    }
}

#[derive(Debug)]
pub struct TerminalBuffer {
    lines: VecDeque<BufferLine>,
    max_lines: NonZeroUsize,
    line_terminated: bool,
    last_line_number: usize,
}

impl Default for TerminalBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalBuffer {
    /// Creates a new, empty `TerminalBuffer` with a default line limit (e.g., 10,000 lines).
    /// The internal buffer is pre-allocated to this default limit.
    pub fn new() -> Self {
        const DEFAULT_MAX_LINES: usize = 10_000;
        let max_lines =
            NonZeroUsize::new(DEFAULT_MAX_LINES).expect("Default max lines is non-zero");
        Self::new_with_max_lines(max_lines)
    }

    /// Creates a new `TerminalBuffer` with a specified maximum number of lines.
    ///
    /// # Arguments
    ///
    /// * `max_lines`: The maximum number of lines the buffer can hold. Must be non-zero.
    ///                The internal `VecDeque` will be initialized with this capacity.
    pub fn new_with_max_lines(max_lines: NonZeroUsize) -> Self {
        Self {
            lines: VecDeque::with_capacity(max_lines.get()),
            max_lines,
            line_terminated: false,
            last_line_number: 0,
        }
    }

    pub fn commit_current_line(&mut self) {
        self.line_terminated = true;
    }

    pub fn extend_line(&mut self, line_in: Arc<StyledLine>) {
        if self.line_terminated {
            self.last_line_number += 1;
            self.line_terminated = false;

            while self.lines.len() > (self.max_lines.get() - 1) {
                self.lines.pop_front();
            }

            self.lines.push_back(line_in.into());
        } else {
            match self.lines.pop_back() {
                Some(line) => self
                    .lines
                    .push_back(Arc::new(line.styled_line.append(&line_in)).into()),
                None => {
                    self.last_line_number += 1;
                    self.lines.push_back(line_in.into());
                }
            }
        }
    }

    /// Adds a line to the buffer.
    /// If the buffer is at its `max_lines` capacity, the oldest line is removed.
    pub fn push_line(&mut self, line: Arc<StyledLine>) {
        self.last_line_number += 1;

        let limit = self.max_lines.get();

        // Remove oldest lines if the buffer is at or would exceed the limit.
        // We want lines.len() to be at most limit - 1 before push_back,
        // so that after push_back, lines.len() is at most limit.
        while self.lines.len() >= limit {
            self.lines.pop_front();
        }
        self.lines.push_back(line.into());
        self.line_terminated = true;
    }

    /// Returns a reverse iterator over the lines in the buffer.
    /// This allows iterating from the most recently added line to the oldest.
    pub fn iter_rev(
        &self,
    ) -> impl DoubleEndedIterator<Item = &BufferLine> + ExactSizeIterator<Item = &BufferLine> {
        self.lines.iter().rev()
    }

    pub fn iter_rev_with_line_number(
        &self,
        last_line_number: Option<usize>,
    ) -> impl Iterator<Item = (usize, &BufferLine)> {
        let buffer_last_line_number = self.last_line_number;
        let to_skip = buffer_last_line_number - last_line_number.unwrap_or(buffer_last_line_number);
        self.lines
            .iter()
            .rev()
            .skip(to_skip)
            .zip(to_skip..)
            .map(move |(line, i)| (buffer_last_line_number - i, line))
    }

    /// Returns an iterator over the lines in the buffer, starting from an offset from the end and iterating in reverse.
    ///
    /// # Arguments
    ///
    /// * `offset`: The number of lines to skip from the end before starting reverse iteration.
    ///             An offset of 0 is equivalent to `iter_rev()`.
    pub fn iter_rev_with_offset(
        &self,
        offset: usize,
    ) -> impl DoubleEndedIterator<Item = &BufferLine> + ExactSizeIterator<Item = &BufferLine> {
        self.lines.iter().rev().skip(offset)
    }

    /// Returns the current number of lines in the buffer.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Returns `true` if the buffer contains no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn last_line_number(&self) -> usize {
        self.last_line_number
    }

    pub fn selected_text(&self, selection: &Selection) -> String {
        match selection {
            Selection::None => String::new(),
            Selection::Selecting { from, to, .. } | Selection::Selected { from, to } => {
                let offset = self.last_line_number - self.lines.len();

                let start_line_index = from.line - offset - 1;
                let to_line_index = to.line - offset - 1;

                (start_line_index..=to_line_index)
                    .map(|i| {
                        let line = &self.lines[i];
                        let start_column = if i == start_line_index {
                            from.column
                        } else {
                            0
                        };
                        let end_column = if i == to_line_index {
                            to.column
                        } else {
                            line.styled_line.text.len()
                        };

                        &line.styled_line.text[start_column..end_column]
                    })
                    .collect::<Vec<&str>>()
                    .join("\n")
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
                for raw_word in line.styled_line.text.split_whitespace() {
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

    pub fn perform_line_operation(&mut self, line_number: usize, operation: LineOperation) {
        let offset = self.last_line_number - self.lines.len();
        let line_number = line_number - offset - 1;
        self.lines.get_mut(line_number).map( |line| {
            let new_line = operation.apply(line.styled_line.clone());
            if let Some(new_line) = new_line {
                line.styled_line = new_line;
                line.spans = to_spans(&line.styled_line);
            } else {
                line.gagged = true; // currently has no effect
            }    
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::styled_line::{StyledLine, VtSpan};
    use std::num::NonZeroUsize; // Assuming VtSpan is needed for StyledLine::new

    // Helper to create Arc<StyledLine> for tests
    fn sl(s: &str) -> Arc<StyledLine> {
        Arc::new(StyledLine::new(s, Vec::<VtSpan>::new()))
    }

    #[test]
    fn test_new_buffer_initial_state() {
        let buffer = TerminalBuffer::new();
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.last_line_number, 0);
        assert_eq!(buffer.max_lines.get(), 10_000); // Default max lines
        assert!(!buffer.line_terminated); // Initial state before any line commit or push
    }

    #[test]
    fn test_new_with_max_lines_initial_state() {
        let max_lines = NonZeroUsize::new(50).unwrap();
        let buffer = TerminalBuffer::new_with_max_lines(max_lines);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.last_line_number, 0);
        assert_eq!(buffer.max_lines, max_lines);
        assert!(!buffer.line_terminated);
    }

    #[test]
    fn test_push_line_increments_current_line_number() {
        let mut buffer = TerminalBuffer::new_with_max_lines(NonZeroUsize::new(3).unwrap());
        assert_eq!(buffer.last_line_number, 0);

        buffer.push_line(sl("line 1"));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.last_line_number, 1);
        assert!(buffer.line_terminated);

        buffer.push_line(sl("line 2"));
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.last_line_number, 2);
        assert!(buffer.line_terminated);
    }

    #[test]
    fn test_extend_line_increments_current_line_number() {
        let mut buffer = TerminalBuffer::new_with_max_lines(NonZeroUsize::new(3).unwrap());

        // Case 1: Extending when line_terminated is true
        buffer.commit_current_line(); // Make line_terminated true
        assert!(buffer.line_terminated);
        buffer.extend_line(sl("line 1 part 1"));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.last_line_number, 1); // Incremented
        assert!(!buffer.line_terminated); // Becomes false after extend

        // Case 2: Extending when line_terminated is false (continuing a line)
        // The current logic in extend_line when line_terminated is false and buffer not empty
        // pops and re-pushes the existing last line, ignoring the input.
        // So, current_line_number should not change.
        let previous_line_number = buffer.last_line_number;
        buffer.extend_line(sl("line 1 part 2 - ignored"));
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.last_line_number, previous_line_number); // Not incremented
        assert!(!buffer.line_terminated);

        // Reset for next test part
        let mut buffer2 = TerminalBuffer::new_with_max_lines(NonZeroUsize::new(3).unwrap());

        // Case 3: Extending when line_terminated is false but buffer is empty (first line)
        assert!(!buffer2.line_terminated);
        assert!(buffer2.is_empty());
        buffer2.extend_line(sl("first line segment"));
        assert_eq!(buffer2.len(), 1);
        assert_eq!(buffer2.last_line_number, 1); // Incremented
        assert!(!buffer2.line_terminated);
    }

    #[test]
    fn test_buffer_wrapping_and_current_line_number() {
        let mut buffer = TerminalBuffer::new_with_max_lines(NonZeroUsize::new(2).unwrap());
        buffer.push_line(sl("1"));
        buffer.push_line(sl("2"));
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.last_line_number, 2);

        buffer.push_line(sl("3")); // Wraps, "1" is popped
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.last_line_number, 3);
        assert_eq!(buffer.lines[0].styled_line.text, "2");
        assert_eq!(buffer.lines[1].styled_line.text, "3");

        buffer.push_line(sl("4")); // Wraps, "2" is popped
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.last_line_number, 4);
        assert_eq!(buffer.lines[0].styled_line.text, "3");
        assert_eq!(buffer.lines[1].styled_line.text, "4");
    }

    #[test]
    fn test_iter_rev_with_line_number_empty() {
        let buffer = TerminalBuffer::new();
        assert_eq!(buffer.iter_rev_with_line_number(None).count(), 0);
    }

    #[test]
    fn test_iter_rev_with_line_number_no_wrap() {
        let mut buffer = TerminalBuffer::new_with_max_lines(NonZeroUsize::new(5).unwrap());
        buffer.push_line(sl("L1")); // cln=1
        buffer.push_line(sl("L2")); // cln=2
        buffer.push_line(sl("L3")); // cln=3. Lines: [L1,L2,L3]

        // iter().rev(): L3, L2, L1
        // enumerate(): (0,L3), (1,L2), (2,L1)
        // map |(i,line)| (cln - i, line) where cln = 3
        // (3-0, L3) -> (3,L3)
        // (3-1, L2) -> (2,L2)
        // (3-2, L1) -> (1,L1)
        let mut iter = buffer.iter_rev_with_line_number(None);
        assert_eq!(
            iter.next().map(|(n, l)| (n, l.styled_line.text.as_str())),
            Some((2, "L3"))
        );
        assert_eq!(
            iter.next().map(|(n, l)| (n, l.styled_line.text.as_str())),
            Some((1, "L2"))
        );
        assert_eq!(
            iter.next().map(|(n, l)| (n, l.styled_line.text.as_str())),
            Some((0, "L1"))
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_iter_rev_with_line_number_with_wrap() {
        let mut buffer = TerminalBuffer::new_with_max_lines(NonZeroUsize::new(2).unwrap());
        buffer.push_line(sl("L1")); // cln=1
        buffer.push_line(sl("L2")); // cln=2. Buffer: [L1,L2]
        buffer.push_line(sl("L3")); // cln=3. Buffer: [L2,L3]

        // cln = 3. Lines in buffer (reversed): L3, L2
        // enumerate: (0, L3), (1, L2)
        // map |(i,line)| (cln - 1 - i, line)
        // (3-1-0, L3) -> (2, L3)
        // (3-1-1, L2) -> (1, L2)
        let mut iter = buffer.iter_rev_with_line_number(None);
        assert_eq!(
            iter.next().map(|(n, l)| (n, l.styled_line.text.as_str())),
            Some((2, "L3"))
        );
        assert_eq!(
            iter.next().map(|(n, l)| (n, l.styled_line.text.as_str())),
            Some((1, "L2"))
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn find_recent_word_logic() {
        let mut buffer = TerminalBuffer::new_with_max_lines(NonZeroUsize::new(10).unwrap());
        buffer.push_line(sl("hello world"));
        buffer.push_line(sl("test another one"));
        buffer.push_line(sl("prefix_found here"));
        buffer.push_line(sl("try prefix_again"));

        // Test basic prefix matching
        assert_eq!(
            buffer.find_recent_word_by_prefix("pref", None, 4),
            Some("prefix_again".to_string())
        );
        assert_eq!(
            buffer.find_recent_word_by_prefix("pref", None, 2),
            Some("prefix_again".to_string())
        ); // Only search last 2 lines
        assert_eq!(
            buffer.find_recent_word_by_prefix("anot", None, 4),
            Some("another".to_string())
        );

        // Test case-insensitivity
        assert_eq!(
            buffer.find_recent_word_by_prefix("PREFIX", None, 4),
            Some("prefix_again".to_string())
        );

        // Test not found
        assert_eq!(
            buffer.find_recent_word_by_prefix("nonexistent", None, 4),
            None
        );

        // Test with skip_words
        let mut skip_set = HashSet::new();
        skip_set.insert("prefix_again".to_string());
        assert_eq!(
            buffer.find_recent_word_by_prefix("pref", Some(&skip_set), 4),
            Some("prefix_found".to_string())
        );

        skip_set.insert("prefix_found".to_string());
        assert_eq!(
            buffer.find_recent_word_by_prefix("pref", Some(&skip_set), 4),
            None
        ); // All "pref" words skipped

        // Test n_recent_lines
        assert_eq!(buffer.find_recent_word_by_prefix("hello", None, 1), None); // "hello" is not in the last line
        assert_eq!(
            buffer.find_recent_word_by_prefix("hello", None, 4),
            Some("hello".to_string())
        ); // "hello" is in the last 4 lines
    }
}
