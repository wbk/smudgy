use crate::theme::{self, Theme};
use iced::{Border, widget::button};

pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, button::Status) -> button::Style + 'a>;

impl button::Catalog for Theme {
    type Class<'a> = StyleFn<'a, Theme>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(primary)
    }

    fn style(&self, class: &Self::Class<'_>, status: button::Status) -> button::Style {
        class(self, status)
    }
}

#[inline]
fn style(button_theme: &theme::Button, status: button::Status) -> button::Style {
    match status {
        button::Status::Active => button::Style {
            background: Some(button_theme.background),
            border: button_theme.border,
            text_color: button_theme.text,
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(button_theme.background_hover),
            border: button_theme.border,
            text_color: button_theme.text,
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(button_theme.background_pressed),
            border: button_theme.border,
            text_color: button_theme.text,
            ..Default::default()
        },
        button::Status::Disabled => button::Style {
            background: Some(button_theme.background.scale_alpha(0.4)),
            border: button_theme
                .border
                .color(button_theme.border.color.scale_alpha(0.4)),
            text_color: button_theme.text.scale_alpha(0.4),
            ..Default::default()
        },
    }
}

pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    style(&theme.styles.buttons.primary, status)
}

pub fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    style(&theme.styles.buttons.secondary, status)
}

pub fn link(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        background: None,
        border: Border::default(),
        text_color: match status {
            button::Status::Active => theme.styles.text.normal,
            button::Status::Hovered => theme.styles.text.normal.scale_alpha(0.8),
            button::Status::Pressed => theme.styles.text.normal.scale_alpha(0.6),
            button::Status::Disabled => theme.styles.text.normal.scale_alpha(0.2),
        },
        ..Default::default()
    }
}
