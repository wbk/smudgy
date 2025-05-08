use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;
use vtparse::{CsiParam, VTActor};

use crate::session::{
        runtime::RuntimeAction,
        styled_line::{VtSpan, Style},
        styled_line::StyledLine,
    };

mod sgr;
pub use sgr::{AnsiColor, Color};

#[derive(Debug)]
pub struct VtProcessor {
    cursor_style: Style,
    buf: String,
    buf_raw: Vec<u8>,
    span_info: Vec<VtSpan>,
    session_runtime_tx: UnboundedSender<RuntimeAction>,
}

const INPUT_BUFFER_CAPACITY: usize = 1024;

impl VtProcessor {
    pub fn new(session_runtime_tx: UnboundedSender<RuntimeAction>) -> Self {
        VtProcessor {
            cursor_style: Style {
                fg: Color::Ansi {
                    color: AnsiColor::White,
                    bold: false,
                },
                bg: Color::DefaultBackground
            },
            buf: String::with_capacity(INPUT_BUFFER_CAPACITY),
            buf_raw: Vec::with_capacity(INPUT_BUFFER_CAPACITY),
            span_info: Vec::new(),
            session_runtime_tx,
        }
    }

    fn change_style(&mut self, new_style: Style) {
        self.span_info.push(VtSpan {
            begin_pos: match self.span_info.last() {
                Some(span_info) => span_info.end_pos,
                None => 0,
            },
            end_pos: self.buf.len(),
            style: self.cursor_style,
        });

        self.cursor_style = new_style;
    }

    pub fn consume_into_pending_line(&mut self) -> StyledLine {
        self.change_style(self.cursor_style);
        StyledLine::new_with_raw(&self.buf, self.span_info.drain(..).collect(), &self.buf_raw)
    }

    pub fn notify_end_of_buffer(&mut self) {
        let pending_line = Arc::new(self.consume_into_pending_line());
        if !self.buf.is_empty() {
            self.session_runtime_tx
                .send(RuntimeAction::HandleIncomingPartialLine(pending_line))
                .unwrap();
            self.buf.clear();
            self.buf_raw.clear();
            self.buf.shrink_to(INPUT_BUFFER_CAPACITY);
            self.buf_raw.shrink_to(INPUT_BUFFER_CAPACITY);
        }
        self.session_runtime_tx
            .send(RuntimeAction::RequestRepaint)
            .unwrap();
    }

    fn commit_line(&mut self) {
        let pending_line = Arc::new(self.consume_into_pending_line());
        self.session_runtime_tx
            .send(RuntimeAction::HandleIncomingLine(pending_line))
            .unwrap();
        self.buf.clear();
        self.buf_raw.clear();
    }

    fn push_incoming_char(&mut self, ch: char) {
        self.buf.push(ch);
    }

    pub fn push_raw_incoming_byte(&mut self, byte: u8) {
        self.buf_raw.push(byte);
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
            let new_style = sgr::process(self.cursor_style, params);
            self.change_style(new_style);
        }
    }

    fn osc_dispatch(&mut self, _params: &[&[u8]]) {}

    fn apc_dispatch(&mut self, _data: Vec<u8>) {}
}
