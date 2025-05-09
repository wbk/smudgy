pub mod fonts {
    use iced::Font;

    pub const GEIST_VF_BYTES: &[u8] = include_bytes!("../../assets/fonts/GeistVF.ttf");
    pub const GEIST_VF: Font = Font::with_name("Geist");
    pub const GEIST_MONO_VF_BYTES: &[u8] = include_bytes!("../../assets/fonts/GeistMonoVF.ttf");
    pub const GEIST_MONO_VF: Font = Font::with_name("Geist Mono");
    pub const BOOTSTRAP_ICONS_BYTES: &[u8] =
        include_bytes!("../../assets/fonts/bootstrap-icons.ttf");
    pub const BOOTSTRAP_ICONS: Font = Font::with_name("bootstrap-icons");
}

pub mod bootstrap_icons {
    pub const ARROW_REPEAT: &str = "\u{F130}";
    pub const ASTERISK: &str = "\u{F151}";
    pub const AT: &str = "\u{F152}";
    pub const DPAD: &str = "\u{F687}";
    pub const FOLDER_PLUS: &str = "\u{F3D3}";
    pub const LIGHTNING: &str = "\u{F46F}";
    pub const TRASH_3: &str = "\u{F78B}";
}

pub mod hero_icons {
    use std::sync::LazyLock;

    use iced::widget::svg;
    pub const BARS_3_BYTES: &[u8] =
        include_bytes!("../../assets/heroicons/optimized/16/solid/bars-3.svg");

    pub static BARS_3: LazyLock<svg::Handle> =
        LazyLock::new(|| svg::Handle::from_memory(BARS_3_BYTES));
}
