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
    AnsiColor { color: AnsiColor, bold: bool },
    RGB { r: u8, g: u8, b: u8 },
    Echo,
    Output,
}

enum SgrState {
    Ready { style: Style },
    SetForegroundReceived,
    SetForegroundAwaitMode,
    SetForegroundMode2,
    SetForegroundMode2Red,
    SetForegroundMode2ReceivedRed { r: u8 },
    SetForegroundMode2Green { r: u8 },
    SetForegroundMode2ReceivedGreen { r: u8, g: u8 },
    SetForegroundMode2Blue { r: u8, g: u8 },
    SetForegroundMode5,
    SetForegroundMode5Number,
    Invalid,
}

pub fn process_sgr(initial_style: Style, params: &[CsiParam]) -> Style {
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
                            fg: Color::AnsiColor {
                                color: AnsiColor::White,
                                bold: false,
                            },
                        },
                    },
                    1 => SgrState::Ready {
                        style: Style {
                            fg: match style.fg {
                                Color::AnsiColor { color, bold: _bold } => {
                                    Color::AnsiColor { color, bold: true }
                                }
                                _ => style.fg,
                            },
                            ..style
                        },
                    },
                    30..=37 => SgrState::Ready {
                        style: Style {
                            fg: Color::AnsiColor {
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
                                    Color::AnsiColor {
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
                            fg: Color::AnsiColor {
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
                    38 => SgrState::SetForegroundReceived,
                    _ => SgrState::Invalid,
                },
                _ => SgrState::Ready { style },
            },
            SgrState::SetForegroundReceived => match param {
                CsiParam::P(b';') => SgrState::SetForegroundAwaitMode,
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundAwaitMode => match param {
                CsiParam::Integer(2) => SgrState::SetForegroundMode2,
                CsiParam::Integer(5) => SgrState::SetForegroundMode5,
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2 => match param {
                CsiParam::P(b';') => SgrState::SetForegroundMode2Red,
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2Red => match param {
                CsiParam::Integer(r) => SgrState::SetForegroundMode2ReceivedRed { r: *r as u8 },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2ReceivedRed { r } => match param {
                CsiParam::P(b';') => SgrState::SetForegroundMode2Green { r },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2Green { r } => match param {
                CsiParam::Integer(g) => {
                    SgrState::SetForegroundMode2ReceivedGreen { r, g: *g as u8 }
                }
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2ReceivedGreen { r, g } => match param {
                CsiParam::P(b';') => SgrState::SetForegroundMode2Blue { r, g },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode2Blue { r, g } => match param {
                CsiParam::Integer(b) => SgrState::Ready {
                    style: Style {
                        fg: Color::RGB { r, g, b: *b as u8 },
                    },
                },
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode5 => match param {
                CsiParam::P(b';') => SgrState::SetForegroundMode5Number,
                _ => SgrState::Invalid,
            },
            SgrState::SetForegroundMode5Number => match param {
                CsiParam::Integer(n) => SgrState::Ready {
                    style: Style {
                        fg: match n {
                            16..=231 => {
                                let n = (n - 16) as f32;
                                let r = (n / 36.0).floor();
                                let g = ((n - (r * 36.0)) / 6.0).floor();
                                let b = n - (r * 36.0) - (g * 6.0);
                                let mul = 255.0 / 6.0;

                                Color::RGB {
                                    r: (r * mul).round() as u8,
                                    g: (g * mul).round() as u8,
                                    b: (b * mul).round() as u8,
                                }
                            }
                            232..=255 => {
                                let range = 255.0 / (255.0 - 232.0);
                                let val = (range * (n - 232) as f32).round() as u8;

                                Color::RGB {
                                    r: val,
                                    g: val,
                                    b: val,
                                }
                            }
                            0 => Color::AnsiColor {
                                color: AnsiColor::Black,
                                bold: false,
                            },
                            1 => Color::AnsiColor {
                                color: AnsiColor::Red,
                                bold: false,
                            },
                            2 => Color::AnsiColor {
                                color: AnsiColor::Green,
                                bold: false,
                            },
                            3 => Color::AnsiColor {
                                color: AnsiColor::Yellow,
                                bold: false,
                            },
                            4 => Color::AnsiColor {
                                color: AnsiColor::Blue,
                                bold: false,
                            },
                            5 => Color::AnsiColor {
                                color: AnsiColor::Magenta,
                                bold: false,
                            },
                            6 => Color::AnsiColor {
                                color: AnsiColor::Cyan,
                                bold: false,
                            },
                            7 => Color::AnsiColor {
                                color: AnsiColor::White,
                                bold: false,
                            },
                            8 => Color::AnsiColor {
                                color: AnsiColor::Black,
                                bold: true,
                            },
                            9 => Color::AnsiColor {
                                color: AnsiColor::Red,
                                bold: true,
                            },
                            10 => Color::AnsiColor {
                                color: AnsiColor::Green,
                                bold: true,
                            },
                            11 => Color::AnsiColor {
                                color: AnsiColor::Yellow,
                                bold: true,
                            },
                            12 => Color::AnsiColor {
                                color: AnsiColor::Blue,
                                bold: true,
                            },
                            13 => Color::AnsiColor {
                                color: AnsiColor::Magenta,
                                bold: true,
                            },
                            14 => Color::AnsiColor {
                                color: AnsiColor::Cyan,
                                bold: true,
                            },
                            15 => Color::AnsiColor {
                                color: AnsiColor::White,
                                bold: true,
                            },
                            _ => Color::AnsiColor {
                                color: AnsiColor::White,
                                bold: false,
                            },
                        },
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
