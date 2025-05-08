use iced::widget::{button, text, Row};
use iced::{Element, Length};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ConnectPressed,
    SettingsPressed,
    AutomationsPressed,
    ToggleExpand, 
}

/// Renders the toolbar based on the expanded state.
///
/// # Arguments
///
/// * `expanded` - Whether the toolbar should be shown in its expanded state.
///
/// # Returns
///
/// An `Element` ready to be included in the main view.
pub fn view(expanded: bool) -> Element<'static, crate::Message> {
    // Maybe wrap in a container later for background/borders
    if expanded {
        // Expanded view: Buttons
        let connect_button = button("Connect")
            .on_press(crate::Message::ToolbarAction(Action::ConnectPressed));
        let settings_button = button("Settings")
            .on_press(crate::Message::ToolbarAction(Action::SettingsPressed));
        let automations_button = button("Automations")
            .on_press(crate::Message::ToolbarAction(Action::AutomationsPressed));
        let collapse_button = button("<")
            .on_press(crate::Message::ToolbarAction(Action::ToggleExpand));

        Row::with_children(vec![
            collapse_button.into(),
            connect_button.into(),
            settings_button.into(),
            automations_button.into(),
        ])
        .spacing(10)
        .padding(5)
        // Make the expanded toolbar fill width
        .width(Length::Fill)
        .into()
    } else {
        // Collapsed view: Hamburger + Text
        let expand_button = button("☰")
            .on_press(crate::Message::ToolbarAction(Action::ToggleExpand));
        let title = text("smudgy");

        Row::with_children(vec![expand_button.into(), title.into()])
            .spacing(10)
            .padding(5)
            // Let the collapsed toolbar take minimal width
            .width(Length::Shrink)
            .into()
    }
}
