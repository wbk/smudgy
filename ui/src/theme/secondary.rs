use iced::{Background, Border, Color, Gradient, Shadow, Vector, border::Radius, gradient::Linear};

use super::{Button, Buttons, General, Modal, Styles, Text, Theme};

pub fn secondary() -> Theme {
    let mut base = super::smudgy::smudgy();
    base.styles.general.background = Color::from_rgb8(25, 25, 25);
    base.styles.general.border = Color::from_rgb8(35, 35, 35);
    base.styles.general.container_background = Color::from_rgb8(15, 15, 14);
    base
}
