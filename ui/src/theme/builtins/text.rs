use iced::widget::text;

use crate::theme::Theme;

impl text::Catalog for Theme {
    type Class<'a> = Box<dyn Fn(&Self) -> text::Style + 'a>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|_theme: &Self| text::Style { color: None })
    }

    fn style(&self, class: &Self::Class<'_>) -> text::Style {
        class(self)
    }
}

pub fn danger(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.styles.text.error)
    }
}
