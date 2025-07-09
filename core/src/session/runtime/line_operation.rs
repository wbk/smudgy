use std::sync::Arc;

use crate::session::styled_line::{Style, StyledLine};

#[derive(Debug, Clone)]
pub enum LineOperation {
    Insert { str: Arc<String>, begin: usize, end: usize, style: Style},
    Replace { str: Arc<String>, begin: usize, end: usize },
    Highlight { begin: usize, end: usize, style: Style },
    Remove { begin: usize, end: usize },
    Gag,
}

impl LineOperation {
    pub fn apply(&self, line: Arc<StyledLine>) -> Option<Arc<StyledLine>> {
        match self {
            LineOperation::Gag => None,
            LineOperation::Insert { str, begin, end, style } => {
                Some(Arc::new(line.insert(str.as_str(), *begin, *end, *style)))
            }
            LineOperation::Replace { str, begin, end } => {
                // For replace, we need a default style - using the first span's style or a default
                let default_style = line.spans.first().map(|s| s.style).unwrap_or(Style {
                    fg: crate::session::connection::vt_processor::Color::Ansi {
                        color: crate::session::connection::vt_processor::AnsiColor::White,
                        bold: false,
                    },
                    bg: crate::session::connection::vt_processor::Color::DefaultBackground,
                });
                Some(Arc::new(line.insert(str.as_str(), *begin, *end, default_style)))
            }
            LineOperation::Highlight { begin, end, style } => {
                Some(Arc::new(line.highlight(*begin, *end, *style)))
            }
            LineOperation::Remove { begin, end } => {
                Some(Arc::new(line.remove(*begin, *end)))
            }
        }
    }
}