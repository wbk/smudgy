use iced::{Element, Length};
use iced::widget::{text_input, container, row, button};

/// A component for inputting text in a session
#[derive(Debug, Default, Clone)]
pub struct SessionInput {
    /// The current input value
    value: String,
    /// Whether the input is focused
    is_focused: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Input value changed
    InputChanged(String),
    /// Submit the current input
    Submit,
    /// Input focus changed
    Focused(bool),
}

impl SessionInput {
    /// Create a new session input component
    pub fn new() -> Self {
        Self {
            value: String::new(),
            is_focused: false,
        }
    }
    
    /// Get the current input value
    pub fn value(&self) -> &str {
        &self.value
    }
    
    /// Clear the input value
    pub fn clear(&mut self) {
        self.value.clear();
    }
    
    /// Update the component state based on messages
    pub fn update(&mut self, message: Message) -> Option<String> {
        match message {
            Message::InputChanged(value) => {
                self.value = value;
                None
            }
            Message::Submit => {
                if self.value.is_empty() {
                    None
                } else {
                    // Return the value to be sent
                    let value = self.value.clone();
                    self.value.clear();
                    Some(value)
                }
            }
            Message::Focused(is_focused) => {
                self.is_focused = is_focused;
                None
            }
        }
    }
    
    /// Render the component
    pub fn view(&self) -> Element<Message> {
        let input = text_input("Type a command...", &self.value)
            .on_input(Message::InputChanged)
            .on_submit(Message::Submit)
            .padding(8)
            .width(Length::Fill);
            
        let send_button = button("Send")
            .on_press(Message::Submit)
            .padding([5, 10]);
            
        container(
            row![
                input,
                send_button
            ]
            .spacing(5)
            .width(Length::Fill)
        )
        .width(Length::Fill)
        .into()
    }
} 