use super::connection::vt_processor;

pub use vt_processor::Color;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub fg: vt_processor::Color,
}

#[derive(Debug, Clone, Copy)]
pub struct SpanInfo {
    pub style: Style,
    pub begin_pos: usize,
    pub end_pos: usize,
}

#[derive(Debug, Clone)]
pub struct StyledLine {
    pub text: String,
    pub spans: Vec<SpanInfo>,
}

impl StyledLine {
    pub fn new(text: &str, span_info: Vec<SpanInfo>) -> Self {
        Self {
            text: String::from(text),
            spans: span_info,
        }
    }

    pub fn append(&self, other_line: &StyledLine) -> Self {
        Self {
            text: format!("{}{}", self.text, other_line.text),
            spans: self
                .spans
                .clone()
                .into_iter()
                .chain(other_line.spans.iter().map(|span| SpanInfo {
                    style: span.style,
                    begin_pos: span.begin_pos + self.text.len(),
                    end_pos: span.end_pos + self.text.len(),
                }))
                .collect(),
        }
    }

    pub fn from_echo_str(text: &str) -> Self {
        Self {
            spans: vec![SpanInfo {
                begin_pos: 0,
                end_pos: text.len(),
                style: Style {
                    fg: { Color::Echo },
                },
            }],
            text: String::from(text),
        }
    }

    pub fn from_output_str(text: &str) -> Self {
        Self {
            spans: vec![SpanInfo {
                begin_pos: 0,
                end_pos: text.len(),
                style: Style {
                    fg: { Color::Output },
                },
            }],
            text: String::from(text),
        }
    }

    #[inline(always)]
    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}
