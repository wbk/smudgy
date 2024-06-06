use std::sync::Arc;

use vtparse::{CsiParam, VTActor};

use crate::{
    session::{
        styled_line::{SpanInfo, Style},
        StyledLine,
    },
    trigger::TriggerManager,
};

mod sgr;
pub use sgr::{AnsiColor, Color};

#[derive(Debug)]
pub struct VtProcessor {
    cursor_style: Style,
    buf: String,
    span_info: Vec<SpanInfo>,
    trigger_manager: Arc<TriggerManager>,
}

const INPUT_BUFFER_CAPACITY: usize = 1024;

impl VtProcessor {
    pub fn new(trigger_manager: Arc<TriggerManager>) -> Self {
        VtProcessor {
            cursor_style: Style {
                fg: Color::AnsiColor {
                    color: AnsiColor::White,
                    bold: false,
                },
            },
            buf: String::with_capacity(INPUT_BUFFER_CAPACITY),
            span_info: Vec::new(),
            trigger_manager,
        }
    }

    fn change_style(&mut self, new_style: Style) {
        self.span_info.push(SpanInfo {
            begin_pos: match self.span_info.last() {
                Some(span_info) => span_info.end_pos,
                None => 0,
            },
            end_pos: self.buf.len(),
            style: self.cursor_style,
        });

        self.cursor_style = new_style;
    }

    pub fn get_remaining_current_line(&mut self) -> StyledLine {
        self.change_style(self.cursor_style);
        let mut forwarded_spans = self.span_info.clone();
        forwarded_spans.retain(|info| (info.end_pos - info.begin_pos) > 0);
        StyledLine::new(&self.buf, forwarded_spans)
    }

    pub fn notify_end_of_buffer(&mut self) {
        let current_partial_line = Arc::new(self.get_remaining_current_line());
        if !self.buf.is_empty() {
            self.trigger_manager
                .process_partial_line(current_partial_line);

            self.span_info.clear();
            self.span_info.push(SpanInfo {
                begin_pos: self.buf.len(),
                end_pos: self.buf.len(),
                style: self.cursor_style,
            });
        }
        self.trigger_manager.request_repaint();
    }

    fn commit_line(&mut self) {
        let current_partial_line = Arc::new(self.get_remaining_current_line());
        self.trigger_manager
            .process_incoming_line(current_partial_line);
        self.buf.clear();
        self.buf.shrink_to(INPUT_BUFFER_CAPACITY);
        self.span_info.clear();
    }

    fn push_incoming_char(&mut self, ch: char) {
        self.buf.push(ch);
    }
}

impl VTActor for VtProcessor {
    fn print(&mut self, b: char) {
        self.push_incoming_char(b);
    }

    fn execute_c0_or_c1(&mut self, control: u8) {
        if control == b'\n' {
            self.commit_line();
        }
    }

    fn dcs_hook(
        &mut self,
        _byte: u8,
        _params: &[i64],
        _intermediates: &[u8],
        _ignored_excess_intermediates: bool,
    ) {
    }

    fn dcs_put(&mut self, _byte: u8) {}

    fn dcs_unhook(&mut self) {}

    fn esc_dispatch(
        &mut self,
        _params: &[i64],
        _intermediates: &[u8],
        _ignored_excess_intermediates: bool,
        _byte: u8,
    ) {
    }

    fn csi_dispatch(&mut self, params: &[CsiParam], _parameters_truncated: bool, byte: u8) {
        if byte == b'm' {
            let new_style = sgr::process_sgr(self.cursor_style, params);
            self.change_style(new_style)
        }
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]]) {}

    fn apc_dispatch(&mut self, _data: Vec<u8>) {}
}
