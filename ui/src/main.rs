#![allow(clippy::pedantic)] // TODO: Remove this later
use iced::{Element, Theme, Task, Color, Length, Font};
use iced::widget::{column, container, text, stack, opaque, mouse_area, center, row};
use iced::{Size, window}; // Import Settings, Size, and window
use iced::alignment::{Horizontal, Vertical};

// Load custom fonts
const GEIST_VF_BYTES: &[u8] = include_bytes!("../../assets/fonts/GeistVF.ttf");
const GEIST_VF: Font = Font::with_name("Geist");
const GEIST_MONO_VF_BYTES: &[u8] = include_bytes!("../../assets/fonts/GeistMonoVF.ttf");

mod toolbar;
mod modal;
mod session_pane;
mod session_input;

extern crate log;

#[derive(Default)]
struct Smudgy {
    toolbar_expanded: bool,
    modal: Option<modal::Modal>, // Store the active modal state
    sessions: Vec<session_pane::SessionPane>, // Store active sessions
}

#[derive(Debug, Clone)]
enum Message {
    ToolbarAction(toolbar::Action),
    ModalMessage(modal::Message), // Wrap modal-specific messages
    ModalEvent(modal::Event),     // Wrap modal-specific events
    CloseModal,                   // Explicit message to close the modal
    SessionMessage(usize, session_pane::Message), // Session index and message
}

// Application starts with no modal and no initial task
fn init() -> (Smudgy, Task<Message>) { // Accept flags
    (Smudgy {
        toolbar_expanded: true,
        modal: None,
        sessions: Vec::new(),
    }, Task::none())
}

 fn main() -> anyhow::Result<()> {

    smudgy_core::init();

    iced::application("smudgy", update, view)
        .theme(|_s| theme())
        .font(GEIST_VF_BYTES) // Set the default font bytes
        .font(GEIST_MONO_VF_BYTES) // Load the mono font bytes
        .default_font(GEIST_VF) // Set the default font family
        .window(window::Settings {
            min_size: Some(Size::new(600.0, 400.0)), // Set minimum size
            ..window::Settings::default()
        })
        .run_with(init)?;
    
    log::info!("exiting");
    Ok(())
}

fn update(smudgy: &mut Smudgy, message: Message) -> Task<Message> {
    match message {
        Message::ToolbarAction(action) => match action {
            toolbar::Action::ToggleExpand => {
                smudgy.toolbar_expanded = !smudgy.toolbar_expanded;
                Task::none()
            }
            toolbar::Action::ConnectPressed => {
                // Create and store the connect modal state
                let connect_state = modal::connect::State::default();
                let new_modal = modal::Modal::Connect(connect_state);
                // Get the initial task from the modal logic
                let modal_init_task: Task<modal::Message> = new_modal.initial_task(); 
                smudgy.modal = Some(new_modal);
                // Map the modal's task message to the main Message type
                modal_init_task.map(Message::ModalMessage)
            }
            toolbar::Action::SettingsPressed => {
                println!("Settings button pressed!");
                // TODO: Open settings modal
                Task::none()
            }
            toolbar::Action::AutomationsPressed => {
                println!("Automations button pressed!");
                // TODO: Open automations modal
                Task::none()
            }
        },
        // Route modal messages to the active modal's update function
        Message::ModalMessage(msg) => {
            if let Some(m) = smudgy.modal.as_mut() {
                let (task, event) = m.update(msg);
                
                // If we have an event, handle it immediately
                if let Some(evt) = event {
                    return update(smudgy, Message::ModalEvent(evt));
                }
                
                // Otherwise return the mapped task
                task.map(Message::ModalMessage)
            } else {
                Task::none()
            }
        }
        // Handle events emitted by the modal
        Message::ModalEvent(event) => match event {
            modal::Event::Connect(connect_event) => match connect_event {
                modal::ConnectEvent::CloseModalRequested => {
                    smudgy.modal = None;
                    Task::none()
                }
                modal::ConnectEvent::Connect(server_name, profile_name) => {
                    println!("Connect requested for {profile_name} on {server_name}");
                    
                    // Create a new session and add it to the sessions list
                    let new_session = session_pane::SessionPane::new(server_name, profile_name);
                    smudgy.sessions.push(new_session);
                    
                    smudgy.modal = None; // Close modal on connect
                    // TODO: Implement actual connection logic
                    Task::none()
                }
            }
        },
        Message::CloseModal => {
             smudgy.modal = None;
             Task::none()
        },
        Message::SessionMessage(session_idx, session_msg) => {
            if let Some(session) = smudgy.sessions.get_mut(session_idx) {
                let session_task = session.update(session_msg.clone());
                
                // Handle special cases that require app-level changes
                match session_msg {
                    session_pane::Message::Close => {
                        // Remove the session when closed
                        if session_idx < smudgy.sessions.len() {
                            smudgy.sessions.remove(session_idx);
                        }
                        Task::none()
                    }
                    _ => {
                        // Map the session task to main Message type
                        session_task.map(move |msg| Message::SessionMessage(session_idx, msg))
                    }
                }
            } else {
                Task::none()
            }
        }
    }
}

fn view(smudgy: &Smudgy) -> Element<Message> {
    let toolbar_element = toolbar::view(smudgy.toolbar_expanded);

    // Create the session view
    let main_content_area: Element<Message> = if smudgy.sessions.is_empty() {
        // If no sessions, show placeholder
        container(text("no active sessions").font(GEIST_VF))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
    } else {
        // Map session views to our main message type
        row(
            smudgy.sessions
                .iter()
                .enumerate()
                .map(|(idx, session)| {
                    session.view().map(move |msg| Message::SessionMessage(idx, msg))
                })
                .collect::<Vec<_>>()
        )
        .spacing(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    };

    let main_layout: Element<_> = column![toolbar_element, main_content_area] // Toolbar first, then content
        .width(Length::Fill)
        .height(Length::Fill)
        // Remove centering from the column itself
        .into();

    // If a modal is active, overlay its view
    if let Some(modal) = &smudgy.modal {
        let modal_view = modal.view().map(Message::ModalMessage); // Map modal messages

        stack(vec![
            main_layout, 
            opaque(mouse_area(center(opaque(modal_view)).style(|_theme| {
                container::Style {
                    background: Some(Color { a: 0.8, ..Color::BLACK }.into()),
                    ..container::Style::default()
                }
            }))
            // Pressing the background sends the explicit CloseModal message
            .on_press(Message::CloseModal)),
        ])
        .into()
    } else {
        main_layout
    }
}

fn theme() -> Theme {
    Theme::Oxocarbon
}