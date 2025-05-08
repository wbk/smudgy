use iced::{Element, Length, Task, Font};
use iced::widget::{column, container, text, row, button, scrollable};
use iced::alignment::{Horizontal, Vertical};

use crate::session_input;

/// Represents a connection session to a server with a specific profile
#[derive(Debug, Clone)]
pub struct SessionPane {
    /// The name of the server this session is connected to
    pub server_name: String,
    /// The name of the profile used for this connection
    pub profile_name: String,
    /// Whether the session is currently active
    pub active: bool,
    /// The content/output of the session
    pub content: String,
    /// Input component for sending commands
    pub input: session_input::SessionInput,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Close this session
    Close,
    /// Activate this session
    Activate,
    /// New content received from server
    ContentReceived(String),
    /// Message from the input component
    Input(session_input::Message),
    /// Send a command to the server
    SendCommand(String),
}

impl SessionPane {
    /// Creates a new session for the given server and profile
    pub fn new(server_name: String, profile_name: String) -> Self {
        Self {
            server_name,
            profile_name,
            active: false,
            content: String::new(),
            input: session_input::SessionInput::new(),
        }
    }

    /// Handle session-specific messages
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Close => {
                // Handle session closing - for now, we'll let the parent handle it
                Task::none()
            }
            Message::Activate => {
                self.active = true;
                Task::none()
            }
            Message::ContentReceived(content) => {
                self.content.push_str(&content);
                // Add a newline if not already present
                if !content.ends_with('\n') {
                    self.content.push('\n');
                }
                Task::none()
            }
            Message::Input(input_msg) => {
                if let Some(command) = self.input.update(input_msg) {
                    // If a command was submitted, echo it to the content and send it
                    let echo_command = format!("> {}\n", command);
                    self.content.push_str(&echo_command);
                    
                    // Return a task to send the command
                    return Task::perform(
                        async move { command },
                        Message::SendCommand
                    );
                }
                Task::none()
            }
            Message::SendCommand(command) => {
                // In a real implementation, this would send the command to the server
                // For now, we'll just echo a fake response
                let response = format!("Server would process: {}\n", command);
                self.content.push_str(&response);
                Task::none()
            }
        }
    }

    /// Render the session
    pub fn view(&self) -> Element<Message> {
        // Session header with title and close button
        let header = row![
            text(format!("{} ({})", self.profile_name, self.server_name))
                .size(16),
            button("×")
                .on_press(Message::Close)
                .padding(2)
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        // Terminal-like output with monospace font and scrolling
        let content = scrollable(
            text(&self.content)
                .font(Font::with_name("Geist Mono"))
                .width(Length::Fill)
        )
        .height(Length::Fill)
        .width(Length::Fill);
            
        // Map input messages to session messages
        let input = self.input.view().map(Message::Input);

        // Combine all elements in a column
        container(
            column![
                header,
                content,
                input
            ]
            .spacing(10)
            .width(Length::Fill)
            .height(Length::Fill)
        )
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

/// View function to render a list of sessions
pub fn view_sessions(sessions: &[SessionPane]) -> Element<Message> {
    if sessions.is_empty() {
        let no_sessions_text: Element<Message> = text("No active sessions")
            .width(Length::Fill)
            .into();
            
        container(no_sessions_text)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
    } else {
        row(
            sessions.iter().map(|session| session.view()).collect::<Vec<_>>()
        )
        .spacing(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
} 