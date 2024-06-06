use std::collections::VecDeque;

const MAX_COMMAND_HISTORY: usize = 100;

/// Used to manage the history of commands entered into each session, and
/// assists in manipulating the text in the command area when the up/down arrows
/// are pressed.

#[derive(Default)]
pub struct CommandHistory {
    history: VecDeque<String>,
    current_offset: Option<usize>,
    draft_line: Option<String>,
}

impl CommandHistory {
    pub fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(MAX_COMMAND_HISTORY),
            ..Self::default()
        }
    }

    /// Notify the CommandHistory that a command was accepted in the input area
    pub fn push(&mut self, line: &str) {
        while self.history.len() + 1 > MAX_COMMAND_HISTORY {
            self.history.pop_front();
        }

        self.current_offset = None;

        if line.len() > 0 {
            self.history.retain(|f| f.ne(line));
            self.history.push_back(line.into());
        }
    }

    /// Notify the CommandHistory that the up arrow has been pressed, along with the text currently in the input area,
    /// responds with an Option<&str> indended to replace the entire input when some
    pub fn next(&mut self, current_line: &str) -> Option<&str> {
        match self.current_offset {
            Some(i) if i < self.history.len() => {
                if i > 0 {
                    self.get_tracked(i - 1)
                } else {
                    None
                }
            }
            _ => {
                let len = self.history.len();
                if len > 0 {
                    if self
                        .history
                        .back()
                        .is_some_and(|hist| hist.ne(current_line))
                    {
                        self.draft_line = Some(current_line.into());
                        self.get_tracked(len - 1)
                    } else {
                        if len >= 2 {
                            self.get_tracked(len - 2)
                        } else {
                            None
                        }
                    }
                } else {
                    None
                }
            }
        }
    }

    /// Notify the CommandHistory that the down arrow has been pressed
    /// responds with an Option<&str> indended to replace the entire input when some
    pub fn prev(&mut self) -> Option<&str> {
        match self.current_offset {
            Some(i) if (i + 1) < self.history.len() => self.get_tracked(i + 1),
            Some(_) => {
                self.current_offset = None;
                match self.draft_line {
                    Some(ref s) => Some(s.as_str()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn get_tracked(&mut self, index: usize) -> Option<&str> {
        self.current_offset = Some(index);
        self.history.get(index).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unedited_buffer() {
        let mut history = CommandHistory::new();

        assert_eq!(history.next("anything"), None);
        assert_eq!(history.prev(), None);

        history.push("hello");
        history.push("world");
        history.push("!");

        assert_eq!(history.next("!"), Some("world"));
        assert_eq!(history.next("world"), Some("hello"));
        assert_eq!(history.next("hello"), None);
        assert_eq!(history.next("hello"), None);
        assert_eq!(history.next("hello"), None);
        assert_eq!(history.prev(), Some("world"));
        assert_eq!(history.prev(), Some("!"));
        assert_eq!(history.prev(), None);
    }

    #[test]
    fn test_edited_buffer() {
        let mut history = CommandHistory::new();

        assert_eq!(history.next("anything"), None);
        assert_eq!(history.prev(), None);

        history.push("hello");
        history.push("world");
        history.push("!");

        assert_eq!(history.next("scratch"), Some("!"));
        assert_eq!(history.next("!"), Some("world"));
        assert_eq!(history.next("world"), Some("hello"));
        assert_eq!(history.next("hello"), None);
        assert_eq!(history.next("hello"), None);
        assert_eq!(history.next("hello"), None);
        assert_eq!(history.prev(), Some("world"));
        assert_eq!(history.prev(), Some("!"));
        assert_eq!(history.prev(), Some("scratch"));
        assert_eq!(history.prev(), None);
    }
}
