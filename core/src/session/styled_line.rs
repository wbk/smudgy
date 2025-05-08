use super::connection::vt_processor;

pub use vt_processor::Color;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub fg: vt_processor::Color,
    pub bg: vt_processor::Color,
}

#[derive(Debug, Clone, Copy)]
pub struct VtSpan {
    pub style: Style,
    pub begin_pos: usize,
    pub end_pos: usize,
}

#[derive(Debug, Clone)]
pub struct StyledLine {
    pub text: String,
    pub spans: Vec<VtSpan>,
    raw: Option<String>,
}

impl StyledLine {
    pub fn new(text: &str, span_info: Vec<VtSpan>) -> Self {
        Self {
            text: String::from(text),
            spans: span_info,
            raw: None,
        }
    }

    pub fn new_with_raw(text: &str, span_info: Vec<VtSpan>, raw: &[u8]) -> Self {
        Self {
            text: String::from(text),
            spans: span_info,
            raw: Some(String::from_utf8_lossy(raw).into_owned()),
        }
    }

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
                            combined.push_str(&other_raw);
                            Some(combined)
                        }
                        None => Some(combined),
                    }
                }
                None => other_line.raw.clone(),
            },
        }
    }

    pub fn from_echo_str(text: &str) -> Self {
        Self {
            spans: vec![VtSpan {
                begin_pos: 0,
                end_pos: text.len(),
                style: Style {
                    fg: { Color::Echo },
                    bg: { Color::DefaultBackground }
                },
            }],
            text: String::from(text),
            raw: None,
        }
    }

    pub fn from_warn_str(text: &str) -> Self {
        Self {
            spans: vec![VtSpan {
                begin_pos: 0,
                end_pos: text.len(),
                style: Style {
                    fg: { Color::Warn },
                    bg: { Color::DefaultBackground }
                },
            }],
            text: String::from(text),
            raw: None,
        }
    }

    pub fn from_output_str(text: &str) -> Self {
        Self {
            spans: vec![VtSpan {
                begin_pos: 0,
                end_pos: text.len(),
                style: Style {
                    fg: { Color::Output },
                    bg: { Color::DefaultBackground }
                },
            }],
            text: String::from(text),
            raw: None,
        }
    }

    pub fn raw(&self) -> Option<&str> {
        self.raw.as_deref()
    }
}

impl std::ops::Deref for StyledLine {
    type Target = str;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.text.as_str()
    }
}
