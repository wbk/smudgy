use super::connection::vt_processor;

pub use vt_processor::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fg: vt_processor::Color,
    pub bg: vt_processor::Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VtSpan {
    pub style: Style,
    pub begin_pos: usize,
    pub end_pos: usize,
}

#[derive(Debug, Clone, Eq)]
pub struct StyledLine {
    pub text: String,
    pub spans: Vec<VtSpan>,
    raw: Option<String>,
}

impl PartialEq for StyledLine {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl StyledLine {
    #[must_use]
    pub fn new(text: &str, span_info: Vec<VtSpan>) -> Self {
        Self {
            text: String::from(text),
            spans: span_info,
            raw: None,
        }
    }

    #[must_use]
    pub fn new_with_raw(text: &str, span_info: Vec<VtSpan>, raw: &[u8]) -> Self {
        Self {
            text: String::from(text),
            spans: span_info,
            raw: Some(String::from_utf8_lossy(raw).into_owned()),
        }
    }

    #[must_use]
    pub fn append(&self, other_line: &StyledLine) -> Self {
        Self {
            text: format!("{}{}", self.text, other_line.text),
            spans: self
                .spans
                .clone()
                .into_iter()
                .chain(other_line.spans.iter().map(|span| VtSpan {
                    style: span.style,
                    begin_pos: span.begin_pos + self.text.len(),
                    end_pos: span.end_pos + self.text.len(),
                }))
                .collect(),
            raw: match self.raw {
                Some(ref raw) => {
                    let mut combined = raw.clone();
                    match other_line.raw {
                        Some(ref other_raw) => {
                            combined.push_str(other_raw);
                            Some(combined)
                        }
                        None => Some(combined),
                    }
                }
                None => other_line.raw.clone(),
            },
        }
    }

    pub fn insert(&self, str: &str, begin: usize, end: usize, style: Style) -> Self {
        // Clamp bounds to text length
        let begin = begin.min(self.text.len());
        let end = end.min(self.text.len().max(begin));
        
        // Create new text by inserting the string
        let mut new_text = String::new();
        new_text.push_str(&self.text[..begin]);
        new_text.push_str(str);
        new_text.push_str(&self.text[end..]);
        
        let insert_len = str.len();
        let removed_len = end - begin;
        let shift = insert_len as i32 - removed_len as i32;
        
        let mut new_spans = Vec::new();
        
        // Adjust existing spans based on the insertion
        for span in self.spans.iter() {
            if span.end_pos <= begin {
                // Span is completely before insertion point
                new_spans.push(*span);
            } else if span.begin_pos >= end {
                // Span is completely after removal range - shift by the difference
                new_spans.push(VtSpan {
                    style: span.style,
                    begin_pos: ((span.begin_pos as i32) + shift).max(0) as usize,
                    end_pos: ((span.end_pos as i32) + shift).max(0) as usize,
                });
            } else if span.begin_pos < begin && span.end_pos > end {
                // Span encompasses the replacement range
                new_spans.push(VtSpan {
                    style: span.style,
                    begin_pos: span.begin_pos,
                    end_pos: end,
                });
                new_spans.push(VtSpan {
                    style: span.style,
                    begin_pos: begin + shift.max(0) as usize,
                    end_pos: span.end_pos + shift.max(0) as usize,
                });
            } else if span.begin_pos < begin && span.end_pos > begin {
                // Span starts before and ends within removal range
                new_spans.push(VtSpan {
                    style: span.style,
                    begin_pos: span.begin_pos,
                    end_pos: begin,
                });
            }
            // Spans that start within [begin, end) are removed by the replacement
        }
        
        // Add span for the inserted text if it's not empty
        if !str.is_empty() {
            new_spans.push(VtSpan {
                style,
                begin_pos: begin,
                end_pos: begin + insert_len,
            });
        }
        
        // Sort spans by begin position
        new_spans.sort_by_key(|span| span.begin_pos);
        
        Self {
            text: new_text,
            spans: new_spans,
            raw: self.raw.clone(),
        }
    }

    pub fn highlight(&self, begin: usize, end: usize, style: Style) -> Self {
        // Clamp bounds to text length
        let begin = begin.min(self.text.len());
        let end = end.min(self.text.len().max(begin));
        
        // If range is empty, return unchanged
        if begin >= end {
            return self.clone();
        }

        let mut new_spans = Vec::new();

        // We want to keep spans that are completely outside the range of the new style,
        // and shrink any spans that have partial overlap with the new style.
        // Any spans that are completely inside the new style are replaced with a single span.
        for span in self.spans.iter() {
            if span.end_pos <= begin {
                // Span is completely before highlight range
                new_spans.push(*span);
            } else if span.begin_pos >= end {
                // Span is completely after highlight range  
                new_spans.push(*span);
            } else if span.begin_pos < begin && span.end_pos > begin && span.end_pos <= end {
                // Span starts before and ends within highlight range - keep the part before
                new_spans.push(VtSpan {
                    style: span.style,
                    begin_pos: span.begin_pos,
                    end_pos: begin,
                });
            } else if span.begin_pos >= begin && span.begin_pos < end && span.end_pos > end {
                // Span starts within and ends after highlight range - keep the part after
                new_spans.push(VtSpan {
                    style: span.style,
                    begin_pos: end,
                    end_pos: span.end_pos,
                });
            } else if span.begin_pos < begin && span.end_pos > end {
                // Span completely encompasses highlight range - split into before and after
                new_spans.push(VtSpan {
                    style: span.style,
                    begin_pos: span.begin_pos,
                    end_pos: begin,
                });
                new_spans.push(VtSpan {
                    style: span.style,
                    begin_pos: end,
                    end_pos: span.end_pos,
                });
            }
            // Case where span is completely within highlight range: 
            // do nothing (gets replaced by highlight span)
        }

        // Add the highlight span
        new_spans.push(VtSpan {
            style,
            begin_pos: begin,
            end_pos: end,
        });

        // Sort spans by begin position to maintain order
        new_spans.sort_by_key(|span| span.begin_pos);

        Self {
            text: self.text.clone(),
            spans: new_spans,
            raw: self.raw.clone(),
        }
    }

    pub fn remove(&self, begin: usize, end: usize) -> Self {
        let text = self.text.as_str();
        let begin = begin.min(text.len());
        let end = end.min(text.len().max(begin));

        let shift = end - begin;

        let new_spans = self.spans.iter().filter_map(|span| {
            if span.begin_pos >= begin && span.end_pos <= end {
                // Span is completely within removal range - remove it
                None
            } else if span.begin_pos >= end {
                // Span is completely after removal range - shift it left
                Some(VtSpan {
                    begin_pos: span.begin_pos - shift,
                    end_pos: span.end_pos - shift,
                    style: span.style,
                })
            } else if span.end_pos <= begin {
                // Span is completely before removal range - keep it unchanged
                Some(*span)
            } else if span.begin_pos < begin && span.end_pos > end {
                // Span encompasses removal range - shrink it
                Some(VtSpan {
                    begin_pos: span.begin_pos,
                    end_pos: span.end_pos - shift,
                    style: span.style,
                })
            } else if span.begin_pos < begin && span.end_pos > begin {
                // Span starts before and ends within removal range - truncate to before part
                Some(VtSpan {
                    begin_pos: span.begin_pos,
                    end_pos: begin,
                    style: span.style,
                })
            } else if span.begin_pos < end && span.end_pos > end {
                // Span starts within and ends after removal range - keep the after part, shifted
                Some(VtSpan {
                    begin_pos: begin,
                    end_pos: span.end_pos - shift,
                    style: span.style,
                })
            } else {
                // Should not reach here, but keep the span as fallback
                Some(*span)
            }
        }).collect();

        Self {
            text: text[..begin].to_string() + &text[end..],
            spans: new_spans,
            raw: self.raw.clone(),
        }
    }
        

    #[must_use]
    pub fn from_echo_str(text: &str) -> Self {
        Self {
            spans: vec![VtSpan {
                begin_pos: 0,
                end_pos: text.len(),
                style: Style {
                    fg: { Color::Echo },
                    bg: { Color::DefaultBackground },
                },
            }],
            text: String::from(text),
            raw: None,
        }
    }

    #[must_use]
    pub fn from_warn_str(text: &str) -> Self {
        Self {
            spans: vec![VtSpan {
                begin_pos: 0,
                end_pos: text.len(),
                style: Style {
                    fg: { Color::Warn },
                    bg: { Color::DefaultBackground },
                },
            }],
            text: String::from(text),
            raw: None,
        }
    }

    #[must_use]
    pub fn from_output_str(text: &str) -> Self {
        Self {
            spans: vec![VtSpan {
                begin_pos: 0,
                end_pos: text.len(),
                style: Style {
                    fg: { Color::Output },
                    bg: { Color::DefaultBackground },
                },
            }],
            text: String::from(text),
            raw: None,
        }
    }

    #[must_use]
    pub fn raw(&self) -> Option<&str> {
        self.raw.as_deref()
    }
}

impl std::ops::Deref for StyledLine {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.text.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::connection::vt_processor::AnsiColor;

    fn create_test_style(fg_color: AnsiColor, bold: bool) -> Style {
        Style {
            fg: Color::Ansi { color: fg_color, bold },
            bg: Color::DefaultBackground,
        }
    }

    fn create_test_line() -> StyledLine {
        StyledLine::new(
            "Hello World Test",
            vec![
                VtSpan {
                    style: create_test_style(AnsiColor::Red, false),
                    begin_pos: 0,
                    end_pos: 5, // "Hello"
                },
                VtSpan {
                    style: create_test_style(AnsiColor::Green, false),
                    begin_pos: 6,
                    end_pos: 11, // "World"
                },
                VtSpan {
                    style: create_test_style(AnsiColor::Blue, false),
                    begin_pos: 12,
                    end_pos: 16, // "Test"
                },
            ],
        )
    }

    #[test]
    fn test_insert_at_beginning() {
        let line = create_test_line();
        let new_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.insert("START ", 0, 0, new_style);

        assert_eq!(result.text, "START Hello World Test");
        assert_eq!(result.spans.len(), 4);
        
        // Check that the new span is at the beginning
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 6);
        assert_eq!(result.spans[0].style, new_style);
        
        // Check that existing spans are shifted
        assert_eq!(result.spans[1].begin_pos, 6);
        assert_eq!(result.spans[1].end_pos, 11);
    }

    #[test]
    fn test_insert_at_end() {
        let line = create_test_line();
        let new_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.insert(" END", 16, 16, new_style);

        assert_eq!(result.text, "Hello World Test END");
        assert_eq!(result.spans.len(), 4);
        
        // Check that existing spans are unchanged
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 5);
        
        // Check that the new span is at the end
        assert_eq!(result.spans[3].begin_pos, 16);
        assert_eq!(result.spans[3].end_pos, 20);
        assert_eq!(result.spans[3].style, new_style);
    }

    #[test]
    fn test_insert_in_middle() {
        let line = create_test_line();
        let new_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.insert(" MIDDLE", 6, 6, new_style);

        assert_eq!(result.text, "Hello  MIDDLEWorld Test");
        assert_eq!(result.spans.len(), 4);
        
        // Check spans before insertion point
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 5);
        
        // Check inserted span
        assert_eq!(result.spans[1].begin_pos, 6);
        assert_eq!(result.spans[1].end_pos, 13);
        assert_eq!(result.spans[1].style, new_style);
        
        // Check spans after insertion point are shifted
        assert_eq!(result.spans[2].begin_pos, 13);
        assert_eq!(result.spans[2].end_pos, 18);
    }

    #[test]
    fn test_insert_with_replacement() {
        let line = create_test_line();
        let new_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.insert("REPLACEMENT", 6, 11, new_style); // Replace "World"

        assert_eq!(result.text, "Hello REPLACEMENT Test");
        assert_eq!(result.spans.len(), 3);
        
        // Check that the replaced span is gone and new span is there
        assert_eq!(result.spans[1].begin_pos, 6);
        assert_eq!(result.spans[1].end_pos, 17);
        assert_eq!(result.spans[1].style, new_style);
    }

    #[test]
    fn test_insert_empty_string() {
        let line = create_test_line();
        let new_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.insert("", 6, 6, new_style);

        assert_eq!(result.text, "Hello World Test");
        assert_eq!(result.spans.len(), 3); // No new span added for empty string
        
        // Check that spans are unchanged
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 5);
    }

    #[test]
    fn test_insert_bounds_checking() {
        let line = create_test_line();
        let new_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.insert("OVERFLOW", 100, 100, new_style);

        assert_eq!(result.text, "Hello World TestOVERFLOW");
        assert_eq!(result.spans.len(), 4);
        
        // Check that the new span is at the actual end
        assert_eq!(result.spans[3].begin_pos, 16);
        assert_eq!(result.spans[3].end_pos, 24);
    }

    #[test]
    fn test_highlight_at_beginning() {
        let line = create_test_line();
        let highlight_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.highlight(0, 3, highlight_style);

        assert_eq!(result.text, "Hello World Test");
        assert_eq!(result.spans.len(), 4);
        
        // Check that the highlight span is first
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 3);
        assert_eq!(result.spans[0].style, highlight_style);
        
        // Check that the original span is truncated
        assert_eq!(result.spans[1].begin_pos, 3);
        assert_eq!(result.spans[1].end_pos, 5);
    }

    #[test]
    fn test_highlight_at_end() {
        let line = create_test_line();
        let highlight_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.highlight(14, 16, highlight_style);

        assert_eq!(result.text, "Hello World Test");
        assert_eq!(result.spans.len(), 4);
        
        // Check that the original span is truncated
        assert_eq!(result.spans[2].begin_pos, 12);
        assert_eq!(result.spans[2].end_pos, 14);
        
        // Check that the highlight span is last
        assert_eq!(result.spans[3].begin_pos, 14);
        assert_eq!(result.spans[3].end_pos, 16);
        assert_eq!(result.spans[3].style, highlight_style);
    }

    #[test]
    fn test_highlight_spanning_multiple_spans() {
        let line = create_test_line();
        let highlight_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.highlight(3, 9, highlight_style); // Spans across "Hello" and "World"

        assert_eq!(result.text, "Hello World Test");
        assert_eq!(result.spans.len(), 4);
        
        // Check that the first span is truncated
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 3);
        
        // Check that the highlight span is in the middle
        assert_eq!(result.spans[1].begin_pos, 3);
        assert_eq!(result.spans[1].end_pos, 9);
        assert_eq!(result.spans[1].style, highlight_style);
        
        // Check that the second span is truncated
        assert_eq!(result.spans[2].begin_pos, 9);
        assert_eq!(result.spans[2].end_pos, 11);
    }

    #[test]
    fn test_highlight_encompassing_span() {
        let line = create_test_line();
        let highlight_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.highlight(4, 8, highlight_style); // Encompasses part of "Hello" and space

        assert_eq!(result.text, "Hello World Test");
        assert_eq!(result.spans.len(), 4);
        
        // Check that the original span is split
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 4);
        
        // Check that the highlight span is in the middle
        assert_eq!(result.spans[1].begin_pos, 4);
        assert_eq!(result.spans[1].end_pos, 8);
        assert_eq!(result.spans[1].style, highlight_style);
        
        // Check that the original span continues after
        assert_eq!(result.spans[2].begin_pos, 8);
        assert_eq!(result.spans[2].end_pos, 11);
    }

    #[test]
    fn test_highlight_empty_range() {
        let line = create_test_line();
        let highlight_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.highlight(5, 5, highlight_style);

        assert_eq!(result.text, "Hello World Test");
        assert_eq!(result.spans.len(), 3); // No change in spans
        
        // Check that spans are unchanged
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 5);
    }

    #[test]
    fn test_highlight_bounds_checking() {
        let line = create_test_line();
        let highlight_style = create_test_style(AnsiColor::Yellow, true);
        let result = line.highlight(10, 100, highlight_style);

        assert_eq!(result.text, "Hello World Test");
        assert_eq!(result.spans.len(), 3);
        
        // Check that the highlight goes to the end of the text
        assert_eq!(result.spans[2].begin_pos, 10);
        assert_eq!(result.spans[2].end_pos, 16);
        assert_eq!(result.spans[2].style, highlight_style);
    }

    #[test]
    fn test_remove_at_beginning() {
        let line = create_test_line();
        let result = line.remove(0, 6); // Remove "Hello "

        assert_eq!(result.text, "World Test");
        assert_eq!(result.spans.len(), 2);
        
        // Check that the first span is removed and others are shifted
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 5);
        assert_eq!(result.spans[1].begin_pos, 6);
        assert_eq!(result.spans[1].end_pos, 10);
    }

    #[test]
    fn test_remove_at_end() {
        let line = create_test_line();
        let result = line.remove(12, 16); // Remove "Test"

        assert_eq!(result.text, "Hello World ");
        assert_eq!(result.spans.len(), 2);
        
        // Check that the last span is removed
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 5);
        assert_eq!(result.spans[1].begin_pos, 6);
        assert_eq!(result.spans[1].end_pos, 11);
    }

    #[test]
    fn test_remove_in_middle() {
        let line = create_test_line();
        let result = line.remove(6, 12); // Remove "World "

        assert_eq!(result.text, "Hello Test");
        assert_eq!(result.spans.len(), 2);
        
        // Check that the middle span is removed and others are shifted
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 5);
        assert_eq!(result.spans[1].begin_pos, 6);
        assert_eq!(result.spans[1].end_pos, 10);
    }

    #[test]
    fn test_remove_partial_span() {
        let line = create_test_line();
        let result = line.remove(2, 8); // Remove "llo Wo"

        assert_eq!(result.text, "Herld Test");
        assert_eq!(result.spans.len(), 3);
        
        // Check that the first span is truncated
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 2);
        
        // Check that the second span (from "World") is truncated and shifted
        assert_eq!(result.spans[1].begin_pos, 2);
        assert_eq!(result.spans[1].end_pos, 5);
        
        // Check that the third span (from "Test") is shifted
        assert_eq!(result.spans[2].begin_pos, 6);
        assert_eq!(result.spans[2].end_pos, 10);
    }

    #[test]
    fn test_remove_empty_range() {
        let line = create_test_line();
        let result = line.remove(5, 5);

        assert_eq!(result.text, "Hello World Test");
        assert_eq!(result.spans.len(), 3);
        
        // Check that nothing changes
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 5);
    }

    #[test]
    fn test_remove_bounds_checking() {
        let line = create_test_line();
        let result = line.remove(10, 100);

        assert_eq!(result.text, "Hello Worl");
        assert_eq!(result.spans.len(), 2);
        
        // Check that removal goes to the end of the text
        assert_eq!(result.spans[0].begin_pos, 0);
        assert_eq!(result.spans[0].end_pos, 5);
        assert_eq!(result.spans[1].begin_pos, 6);
        assert_eq!(result.spans[1].end_pos, 10);
    }

    #[test]
    fn test_remove_entire_text() {
        let line = create_test_line();
        let result = line.remove(0, 100);

        assert_eq!(result.text, "");
        assert_eq!(result.spans.len(), 0);
    }
}
