use vtparse::CsiParam;

use crate::session::styled_line::Style;

#[derive(Copy, Clone, Debug)]
pub enum AnsiColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

#[derive(Copy, Clone, Debug)]
pub enum Color {
    Ansi { color: AnsiColor, bold: bool },
    Rgb { r: u8, g: u8, b: u8 },
    Echo,
    Output,
    Warn,
    DefaultBackground,
}

enum SgrState {
    Ready { style: Style },
    SetForegroundReceived { style: Style },
    SetForegroundAwaitMode { style: Style },
    SetForegroundMode2 { style: Style },
    SetForegroundMode2Red { style: Style },
    SetForegroundMode2ReceivedRed { style: Style, r: u8 },
    SetForegroundMode2Green { style: Style, r: u8 },
    SetForegroundMode2ReceivedGreen { style: Style, r: u8, g: u8 },
    SetForegroundMode2Blue { style: Style, r: u8, g: u8 },
    SetForegroundMode5 { style: Style },
    SetForegroundMode5Number { style: Style },
    Invalid,
}

pub fn process(initial_style: Style, params: &[CsiParam]) -> Style {
    let mut state = SgrState::Ready {
        style: initial_style,
    };
    for param in params {
        state = match state {
            SgrState::Invalid => state,
            SgrState::Ready { style } => match param {
                CsiParam::Integer(n) => match n {
                    0 => SgrState::Ready {
                        style: Style {
                            fg: Color::Ansi {
                                color: AnsiColor::White,
                                bold: false,
                            },
                            bg: Color::DefaultBackground
                        },
                    },
                    1 => SgrState::Ready {
                        style: Style {
                            fg: match style.fg {
                                Color::Ansi { color, bold: _bold } => {
                                    Color::Ansi { color, bold: true }
                                }
                                _ => style.fg,
                            },
                            ..style
                        },
                    },
                    30..=37 => SgrState::Ready {
                        style: Style {
                            fg: Color::Ansi {
                                color: match n {
                                    30 => AnsiColor::Black,
                                    31 => AnsiColor::Red,
                                    32 => AnsiColor::Green,
                                    33 => AnsiColor::Yellow,
                                    34 => AnsiColor::Blue,
                                    35 => AnsiColor::Magenta,
                                    36 => AnsiColor::Cyan,
                                    37 => AnsiColor::White,
                                    _ => unreachable!(),
                                },
                                bold: match style.fg {
                                    Color::Ansi {
                                        color: _,
                                        bold: is_bold,
                                    } => is_bold,
                                    _ => false,
                                },
                            },
                            ..style
                        },
                    },
                    90..=97 => SgrState::Ready {
                        style: Style {
                            fg: Color::Ansi {
                                color: match n {
                                    90 => AnsiColor::Black,
                                    91 => AnsiColor::Red,
                                    92 => AnsiColor::Green,
                                    93 => AnsiColor::Yellow,
                                    94 => AnsiColor::Blue,
                                    95 => AnsiColor::Magenta,
                                    96 => AnsiColor::Cyan,
                                    97 => AnsiColor::White,
                                    _ => unreachable!(),
                                },
                                bold: true,
                            },
                            ..style
                        },
                    },
                    38 => SgrState::SetForegroundReceived {style},
                    _ => SgrState::Invalid,
                },
                _ => SgrState::Ready { style },
            },
            SgrState::SetForegroundReceived {style }=> match param {
                CsiParam::P(b';') => SgrState::SetForegroundAwaitMode { style },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundAwaitMode  { style } => match param {
                CsiParam::Integer(2) => SgrState::SetForegroundMode2 { style },
                CsiParam::Integer(5) => SgrState::SetForegroundMode5 { style },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2 { style } => match param {
                CsiParam::P(b';') => SgrState::SetForegroundMode2Red { style },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2Red { style } => match param {
                CsiParam::Integer(r) => SgrState::SetForegroundMode2ReceivedRed { style, r: *r as u8 },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2ReceivedRed { style, r } => match param {
                CsiParam::P(b';') => SgrState::SetForegroundMode2Green { style, r },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2Green { style, r } => match param {
                CsiParam::Integer(g) => {
                    SgrState::SetForegroundMode2ReceivedGreen { style, r, g: *g as u8 }
                }
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2ReceivedGreen { style, r, g } => match param {
                CsiParam::P(b';') => SgrState::SetForegroundMode2Blue { style, r, g },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2Blue { style, r, g } => match param {
                CsiParam::Integer(b) => SgrState::Ready {
                    style: Style {
                        fg: Color::Rgb { r, g, b: *b as u8 },
                        ..style
                    },
                },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode5 { style }=> match param {
                CsiParam::P(b';') => SgrState::SetForegroundMode5Number { style },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode5Number { style }=> match param {
                CsiParam::Integer(n) => SgrState::Ready {
                    style: Style {
                        fg: match n {
                            16..=231 => {
                                let n = (n - 16) as f32;
                                let r = (n / 36.0).floor();
                                let g = ((n - (r * 36.0)) / 6.0).floor();
                                let b = n - (r * 36.0) - (g * 6.0);
                                let mul = 255.0 / 6.0;

                                Color::Rgb {
                                    r: (r * mul).round() as u8,
                                    g: (g * mul).round() as u8,
                                    b: (b * mul).round() as u8,
                                }
                            }
                            232..=255 => {
                                let range = 255.0 / (255.0 - 232.0);
                                let val = (range * (n - 232) as f32).round() as u8;

                                Color::Rgb {
                                    r: val,
                                    g: val,
                                    b: val,
                                }
                            }
                            0 => Color::Ansi {
                                color: AnsiColor::Black,
                                bold: false,
                            },
                            1 => Color::Ansi {
                                color: AnsiColor::Red,
                                bold: false,
                            },
                            2 => Color::Ansi {
                                color: AnsiColor::Green,
                                bold: false,
                            },
                            3 => Color::Ansi {
                                color: AnsiColor::Yellow,
                                bold: false,
                            },
                            4 => Color::Ansi {
                                color: AnsiColor::Blue,
                                bold: false,
                            },
                            5 => Color::Ansi {
                                color: AnsiColor::Magenta,
                                bold: false,
                            },
                            6 => Color::Ansi {
                                color: AnsiColor::Cyan,
                                bold: false,
                            },
                            7 => Color::Ansi {
                                color: AnsiColor::White,
                                bold: false,
                            },
                            8 => Color::Ansi {
                                color: AnsiColor::Black,
                                bold: true,
                            },
                            9 => Color::Ansi {
                                color: AnsiColor::Red,
                                bold: true,
                            },
                            10 => Color::Ansi {
                                color: AnsiColor::Green,
                                bold: true,
                            },
                            11 => Color::Ansi {
                                color: AnsiColor::Yellow,
                                bold: true,
                            },
                            12 => Color::Ansi {
                                color: AnsiColor::Blue,
                                bold: true,
                            },
                            13 => Color::Ansi {
                                color: AnsiColor::Magenta,
                                bold: true,
                            },
                            14 => Color::Ansi {
                                color: AnsiColor::Cyan,
                                bold: true,
                            },
                            15 => Color::Ansi {
                                color: AnsiColor::White,
                                bold: true,
                            },
                            _ => Color::Ansi {
                                color: AnsiColor::White,
                                bold: false,
                            },
                        },
                        ..style
                    },
                },
                _ => SgrState::Invalid,
            },
        }
    }

    match state {
        SgrState::Ready { style } => style,
        _ => initial_style,
    }
}
