
use iced::{widget::progress_bar::{self, Catalog, Style, StyleFn}, Border, Color};

use crate::Theme;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Theme>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> progress_bar::Style {
        class(self)
    }
}

pub fn default(theme: &Theme) -> progress_bar::Style {
    Style { 
        background: theme.styles.general.background.into(),
        bar: Color::WHITE.into(),
        border: Default::default(),
    }
}
