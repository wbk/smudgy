use crate::Theme;
use iced::{border::Radius, widget::container};

pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> container::Style + 'a>;

impl container::Catalog for Theme {
    type Class<'a> = StyleFn<'a, Theme>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> container::Style {
        class(self)
    }
}

pub fn default(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgb8(0, 0, 0))),
        ..Default::default()
    }
}

pub fn plain(_theme: &Theme) -> container::Style {
    container::Style {
        ..Default::default()
    }
}

pub fn overlay(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(
            theme.styles.general.overlay_background,
        )),
        ..Default::default()
    }
}

pub fn modal_container(theme: &Theme) -> container::Style {
    container::Style {
        shadow: theme.styles.modal.shadow,
        ..Default::default()
    }
}

pub fn modal_title_bar(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(theme.styles.modal.title_bar_background),
        border: theme.styles.modal.title_bar_border.rounded(Radius {
            top_left: 5.0,
            top_right: 5.0,
            bottom_right: 0.0,
            bottom_left: 0.0,
        }),
        ..Default::default()
    }
}

pub fn modal_body(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(theme.styles.modal.body_background),
        border: theme.styles.modal.body_border.rounded(Radius {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: 5.0,
            bottom_left: 5.0,
        }),        
        ..Default::default()
    }
}
