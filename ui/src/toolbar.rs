use iced::Length;
use iced::widget::{Row, button, svg, text};

use crate::assets;
use crate::theme::Element;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Message {
    ConnectPressed,
    SettingsPressed,
    AutomationsPressed,
    MapEditorPressed,
    ToggleExpand,
    // Future: DisconnectPressed, ReconnectPressed
}

/// Context information about the active session for the toolbar
#[derive(Debug, Clone)]
pub struct SessionContext {
    pub has_active_session: bool,
    pub is_connected: bool,
    pub server_name: String,
}

impl Default for SessionContext {
    fn default() -> Self {
        Self {
            has_active_session: false,
            is_connected: false,
            server_name: String::new(),
        }
    }
}

pub fn view(expanded: bool, session_context: &SessionContext) -> Element<'static, Message> {
    // Maybe wrap in a container later for background/borders
    if expanded {
        // Expanded view: Buttons
        let connect_button = button("Connect").on_press(Message::ConnectPressed);
        let collapse_button = button("<").on_press(Message::ToggleExpand);
        
        let mut buttons = vec![
            collapse_button.into(),
            connect_button.into(),
        ];
        
        // Only show automations button if there's an active session
        if session_context.has_active_session {
            let automations_button = button("Automations").on_press(Message::AutomationsPressed);
            buttons.push(automations_button.into());
            let map_editor_button = button("Map Editor").on_press(Message::MapEditorPressed);
            buttons.push(map_editor_button.into());
            
            // Future: Add disconnect/reconnect buttons based on connection state
            // if session_context.is_connected {
            //     let disconnect_button = button("Disconnect").on_press(Message::DisconnectPressed);
            //     buttons.push(disconnect_button.into());
            // } else {
            //     let reconnect_button = button("Reconnect").on_press(Message::ReconnectPressed);
            //     buttons.push(reconnect_button.into());
            // }
        }

        Row::with_children(buttons)
            .spacing(10)
            .padding(5)
            // Make the expanded toolbar fill width
            .width(Length::Fill)
            .into()
    } else {
        // Collapsed view: Hamburger + Text
        let expand_button = button(svg(assets::hero_icons::BARS_3.clone()).width(16).height(16))
            .on_press(Message::ToggleExpand);
        let title = text("smudgy");

        Row::with_children(vec![expand_button.into(), title.into()])
            .spacing(10)
            .padding(5)
            // Let the collapsed toolbar take minimal width
            .width(Length::Shrink)
            .into()
    }
}
