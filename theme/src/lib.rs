use std::ops::Deref;
use std::sync::LazyLock;

use iced::widget::{container, scrollable, svg, text, text_editor};
use iced::{Background, Border, Color, Shadow, border};

mod secondary;
mod smudgy;

pub use secondary::secondary;
pub use smudgy::smudgy;

pub mod builtins {
    pub mod button;
    pub mod container;
    pub mod radio;
    pub mod rule;
    pub mod svg;
    pub mod text;
    pub mod text_input;
}

pub type Element<'a, Message> = iced::Element<'a, Message, Theme>;
pub struct Theme {
    pub name: String,
    pub styles: Styles,
}

impl iced::theme::Base for Theme {
    fn base(&self) -> iced::theme::Style {
        iced::theme::Style {
            background_color: self.styles.general.background,
            text_color: self.styles.text.normal,
        }
    }

    fn palette(&self) -> Option<iced::theme::Palette> {
        Some(iced::theme::Palette {
            background: self.styles.general.background,
            text: self.styles.text.normal,
            primary: self.styles.buttons.primary.text,
            success: self.styles.text.success,
            warning: self.styles.text.error,
            danger: self.styles.text.error,
        })
    }
}

#[derive(Debug)]
pub struct Styles {
    pub buttons: Buttons,
    pub general: General,
    pub text: Text,
    pub modal: Modal,
}

#[derive(Debug, Clone)]
pub struct Modal {
    pub title_bar_background: Background,
    pub title_bar_border: Border,
    pub body_background: Background,
    pub body_border: Border,
    pub shadow: Shadow,
}
#[derive(Debug, Clone)]
pub struct Buttons {
    pub primary: Button,
    pub secondary: Button,
}

#[derive(Debug, Clone)]
pub struct Button {
    pub background: Background,
    pub background_hover: Background,
    pub background_pressed: Background,
    pub border: Border,
    pub text: Color,
}

#[derive(Debug, Clone)]
pub struct General {
    pub background: Color,
    pub container_background: Color,
    pub accent: Color,
    pub border: Color,
    pub rule: Color,
    pub overlay_background: Color,
}

#[derive(Debug, Clone)]
pub struct Text {
    pub normal: Color,
    pub success: Color,
    pub error: Color,
}

impl Default for Theme {
    fn default() -> Self {
        smudgy::smudgy()
    }
}

impl scrollable::Catalog for Theme {
    type Class<'a> = ();

    fn default<'a>() -> Self::Class<'a> {}

    fn style(&self, _class: &Self::Class<'_>, _status: scrollable::Status) -> scrollable::Style {
        scrollable::Style {
            container: container::Style {
                ..Default::default()
            },
            gap: None,
            horizontal_rail: scrollable::Rail {
                background: None,
                border: Border::default(),
                scroller: scrollable::Scroller {
                    color: self.styles.general.accent,
                    border: Border::default(),
                },
            },
            vertical_rail: scrollable::Rail {
                background: None,
                border: Border::default(),
                scroller: scrollable::Scroller {
                    color: self.styles.general.accent,                    
                    border: Border::default(),
                },
            },
        }
    }
}


pub enum TextEditorClass {
    Default,
}

impl text_editor::Catalog for Theme {
    type Class<'a> = TextEditorClass;

    fn default<'a>() -> Self::Class<'a> {
        TextEditorClass::Default
    }

    fn style(&self, class: &Self::Class<'_>, status: text_editor::Status) -> text_editor::Style {
        text_editor::Style {
            background: Background::Color(self.styles.general.container_background),
            border: border::color(self.styles.general.border).width(1.0).into(),
            icon: self.styles.text.normal,
            placeholder: self.styles.text.normal.scale_alpha(0.4),
            value: self.styles.text.normal,
            selection: self.styles.general.accent,
        }
    }
}
