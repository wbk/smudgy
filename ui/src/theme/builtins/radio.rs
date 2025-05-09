
use iced::{border::{radius}, widget::radio::{self, Catalog}};

use crate::theme::{self, Theme};

pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, radio::Status) -> radio::Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Theme>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }


    fn style(&self, class: &Self::Class<'_>, status: radio::Status) -> radio::Style {
        class(self, status)
    }
}

pub fn default(theme: &Theme, status: radio::Status) -> radio::Style {
    radio::Style { background: iced::Background::Color(iced::Color::WHITE), dot_color: theme.styles.general.accent, border_width: 0.0, border_color: theme.styles.general.accent, text_color: None }
}
