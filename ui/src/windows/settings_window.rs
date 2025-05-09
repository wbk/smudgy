use crate::components::Update;
use crate::theme::Element as ThemedElement;
use iced::widget::{column, text};

#[derive(Debug, Clone)]
pub enum Message {
    None,
}

#[derive(Debug, Clone)]
pub enum Event {}
pub struct SettingsWindow {
}

impl SettingsWindow {
    pub fn new() -> Self {
        Self { }
    }
    pub fn update(&mut self, message: Message) -> Update<Message, Event> {
        match message {
            Message::None => Update::none(),
        }
    }

    pub fn view(&self) -> ThemedElement<Event> {
        column![text("Future settings window"),].into()
    }
}
