use std::{
    borrow::Cow,
    cmp::{max, min},
    rc::Rc,
};

use iced::widget::text::Span;
use smudgy_core::terminal_buffer::selection::LineSelection;

#[derive(Debug, Clone)]
pub struct Spans<Link: Clone> {
    spans: Rc<Vec<Span<'static, Link>>>,
    selected: Vec<usize>,
    spans_with_selection: Option<Rc<Vec<Span<'static, Link>>>>,
}

impl<Link: Clone> Spans<Link> {
    pub fn with_selection(spans: Rc<Vec<Span<'static, Link>>>, selection: LineSelection) -> Self {
        match selection {
            None => Self {
                spans,
                selected: Vec::new(),
                spans_with_selection: None,
            },
            Some((0, usize::MAX)) => {
                let mut spans = Self {
                    spans,
                    selected: Vec::new(),
                    spans_with_selection: None,
                };
                spans.select_all();
                spans
            }
            Some((from, to)) => {
                let mut spans = Self {
                    spans,
                    selected: Vec::new(),
                    spans_with_selection: None,
                };
                spans.select_range(from, to);
                spans
            }
        }
    }

    pub fn spans(&self) -> Rc<Vec<Span<'static, Link>>> {
        self.spans_with_selection
            .as_ref()
            .map(|spans| spans.clone())
            .unwrap_or_else(|| self.spans.clone())
    }

    pub fn select_none(&mut self) {
        self.selected.clear();
        self.spans_with_selection = None;
    }

    pub fn select_all(&mut self) {
        self.selected = (0..self.spans.len()).collect();
        self.spans_with_selection = None;
    }

    pub fn select_range(&mut self, sel_start: usize, sel_end: usize) {
        let mut span_start = 0;

        self.selected.clear();

        self.spans_with_selection = Some(Rc::new(
            self.spans
                .iter()
                .flat_map(|span| {
                    let span_end = span_start + span.text.len();

                    let mut spans = Vec::with_capacity(1);

                    if sel_start > span_start {
                        // at least part of the span is before the selection
                        let unselected_end = min(sel_start, span_end) - span_start;
                        spans.push((
                            false,
                            Span {
                                text: Cow::Owned(span.text[0..unselected_end].to_string()),
                                link: span.link.clone(),
                                ..*span
                            },
                        ))
                    }

                    if sel_start < span_end && sel_end > span_start {
                        // at least part of the span is selected
                        let selected_start = max(sel_start, span_start) - span_start;
                        let selected_end = min(sel_end, span_end) - span_start;

                        spans.push((
                            true,
                            Span {
                                text: Cow::Owned(
                                    span.text[selected_start..selected_end].to_string(),
                                ),
                                link: span.link.clone(),
                                ..*span
                            },
                        ))
                    }

                    if sel_end < span_end {
                        // at least part of the span is after the selection
                        let unselected_start = max(sel_end, span_start) - span_start;

                        spans.push((
                            false,
                            Span {
                                text: Cow::Owned(
                                    span.text[unselected_start..span.text.len()].to_string(),
                                ),
                                link: span.link.clone(),
                                ..*span
                            },
                        ))
                    }

                    span_start = span_end;

                    spans
                })
                .enumerate()
                .map(|(i, (selected, span))| {
                    if selected {
                        self.selected.push(i);
                    }
                    span
                })
                .collect(),
        ));
    }

    pub fn selected(&self) -> &[usize] {
        &self.selected
    }
}
