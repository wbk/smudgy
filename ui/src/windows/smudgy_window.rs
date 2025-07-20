use std::{rc::Rc, sync::Arc};

use iced::{
    alignment::{Horizontal, Vertical}, widget::{center, column, container, mouse_area, opaque, row, stack, text, text_input}, Color, Event as IcedEvent, Length, Subscription, Task
};
use smudgy_core::session::{SessionId, TaggedSessionEvent};
use smudgy_map::{AreaId, Mapper};

use crate::{
    assets,
    components::{Update, session_pane},
    modal,
    theme::{self, Element as ThemedElement},
    toolbar,
};

#[derive(Debug, Clone)]
pub enum Message {
    ToolbarAction(toolbar::Message),
    ModalMessage(modal::Message),
    ModalEvent(modal::Event),
    CloseModal,
    SetActiveSession(SessionId),
    SessionPaneUserAction {
        session_id: SessionId,
        msg: session_pane::Message,
    },
    SessionEvent(TaggedSessionEvent),
}

#[derive(Debug, Clone)]
pub enum Event {
    CreateNewScriptEditorWindow { server_name: Arc<String> },
    CreateNewMapEditorWindow { mapper: Mapper },
    SetMapperCurrentLocation(AreaId, Option<i32>),
}

pub struct SmudgyWindow {
    toolbar_expanded: bool,
    modal: Option<modal::Modal>,
    session_panes: Vec<session_pane::SessionPane>,
    active_session_id: Option<SessionId>,
    next_session_id: SessionId,
}

impl SmudgyWindow {
    pub fn new() -> Self {
        Self {
            toolbar_expanded: true,
            modal: None,
            session_panes: Vec::new(),
            active_session_id: None,
            next_session_id: 0.into(),
        }
    }

    pub fn session_panes(&self) -> &[session_pane::SessionPane] {
        &self.session_panes
    }

    /// Create session context information for the toolbar
    fn create_session_context(&self) -> toolbar::SessionContext {
        if let Some(active_id) = self.active_session_id {
            if let Some(active_session) = self.session_panes.iter().find(|s| s.id == active_id) {
                toolbar::SessionContext {
                    has_active_session: true,
                    is_connected: active_session.is_connected(),
                    server_name: active_session.server_name.clone(),
                }
            } else {
                toolbar::SessionContext::default()
            }
        } else {
            toolbar::SessionContext::default()
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let session_subscriptions = Subscription::batch(
            self.session_panes
                .iter()
                .map(|session| session.session_subscription().map(Message::SessionEvent)),
        );

        session_subscriptions
    }

    /// Set the active session, deactivating all others
    pub fn set_active_session(&mut self, session_id: SessionId) -> Task<Message> {
        self.active_session_id = Some(session_id);

        // Find the session and get its unique input ID
        if let Some(session) = self.session_panes.iter().find(|s| s.id == session_id) {
            let input_id = session.input.input_id();
            text_input::focus(input_id)
        } else {
            Task::none()
        }
    }

    pub fn update(&mut self, message: Message) -> Update<Message, Event> {
        match message {
            Message::ToolbarAction(action) => match action {
                toolbar::Message::ToggleExpand => {
                    self.toolbar_expanded = !self.toolbar_expanded;
                    Update::none()
                }
                toolbar::Message::ConnectPressed => {
                    let connect_state = modal::connect::State::default();
                    let new_modal = modal::Modal::Connect(connect_state);
                    let modal_init_task: Task<modal::Message> = new_modal.initial_task();
                    self.modal = Some(new_modal);
                    Update::with_task(modal_init_task.map(Message::ModalMessage))
                }
                toolbar::Message::SettingsPressed => {
                    log::info!("Settings button pressed!");
                    Update::none()
                }
                toolbar::Message::AutomationsPressed => {
                    // Only allow automation actions when there's an active session
                    if let Some(active_id) = self.active_session_id {
                        if let Some(active_session) =
                            self.session_panes.iter().find(|s| s.id == active_id)
                        {
                            Update::with_event(Event::CreateNewScriptEditorWindow {
                                server_name: Arc::new(active_session.server_name.clone()),
                            })
                        } else {
                            log::warn!(
                                "Active session ID {} not found in session panes",
                                active_id
                            );
                            Update::none()
                        }
                    } else {
                        log::info!("AutomationsPressed ignored - no active session");
                        Update::none()
                    }
                }
                toolbar::Message::MapEditorPressed => {
                    if let Some(active_id) = self.active_session_id {
                        if let Some(active_session) =
                            self.session_panes.iter().find(|s| s.id == active_id)
                        {
                            active_session
                                .mapper
                                .as_ref()
                                .map(|mapper| {
                                    Update::with_event(Event::CreateNewMapEditorWindow { mapper: mapper.clone() })
                                })
                                .unwrap_or_else(|| Update::none())
                        } else {
                            log::warn!(
                                "Active session ID {} not found in session panes",
                                active_id
                            );
                            Update::none()
                        }
                    } else {
                        log::info!("AutomationsPressed ignored - no active session");
                        Update::none()
                    }
                }
            },
            Message::ModalMessage(msg) => {
                if let Some(m) = self.modal.as_mut() {
                    let (task, event) = m.update(msg);
                    if let Some(evt) = event {
                        return self.update(Message::ModalEvent(evt));
                    }
                    Update::with_task(task.map(Message::ModalMessage))
                } else {
                    Update::none()
                }
            }
            Message::ModalEvent(event) => match event {
                modal::Event::Connect(connect_event) => match connect_event {
                    modal::ConnectEvent::CloseModalRequested => {
                        self.modal = None;
                        Update::none()
                    }
                    modal::ConnectEvent::Connect(server_name, profile_name) => {
                        log::info!("Connect requested for {profile_name} on {server_name}");

                        let session_id = self.next_session_id;
                        self.next_session_id = self.next_session_id + 1.into();

                        let new_session =
                            session_pane::SessionPane::new(session_id, server_name, profile_name);

                        self.session_panes.push(new_session);

                        // Set this as the active session (will deactivate others)
                        let focus_task = self.set_active_session(session_id);

                        self.toolbar_expanded = false;

                        self.modal = None;

                        Update::with_task(focus_task)
                    }
                },
            },
            Message::CloseModal => {
                self.modal = None;
                Update::none()
            }
            Message::SetActiveSession(session_id) => {
                // TODO: I'm not sure this is still used...
                let focus_task = self.set_active_session(session_id);
                Update::with_task(focus_task)
            }
            Message::SessionPaneUserAction { session_id, msg } => {
                if let Some(managed_session) =
                    self.session_panes.iter_mut().find(|s| s.id == session_id)
                {
                    let session_task = managed_session.update(msg.clone());

                    // Handle special cases from pane messages, e.g., Close
                    if let session_pane::Message::Close = msg {
                        log::info!("Closing session with id: {}", session_id);

                        // If we're closing the active session, clear the active session
                        if self.active_session_id == Some(session_id) {
                            self.active_session_id = None;
                        }

                        self.session_panes.retain(|s| s.id != session_id);

                        // If there are remaining sessions and no active session, activate the first one
                        if self.active_session_id.is_none() && !self.session_panes.is_empty() {
                            let first_session_id = self.session_panes[0].id;
                            let focus_task = self.set_active_session(first_session_id);
                            Update::with_task(focus_task)
                        } else {
                            Update::none()
                        }
                    } else if let session_pane::Message::SetMapperCurrentLocation(area_id, room_number) = msg {
                        // Handle the SetMapperCurrentLocation message that bubbles up from session inputs
                        Update::with_event(Event::SetMapperCurrentLocation(area_id, room_number))
                    } else {
                        // Handle the Activate message that bubbles up from session inputs
                        if let session_pane::Message::Activate = msg {
                            self.toolbar_expanded = false;

                            let focus_task = self.set_active_session(session_id);
                            Update::with_task(focus_task)
                        } else {
                            Update::with_task(session_task.map(move |pane_msg| {
                                Message::SessionPaneUserAction {
                                    session_id,
                                    msg: pane_msg,
                                }
                            }))
                        }
                    }
                } else {
                    log::warn!(
                        "Received SessionPaneUserAction for unknown session_id: {}",
                        session_id
                    );
                    Update::none()
                }
            }
            Message::SessionEvent(TaggedSessionEvent { session_id, event }) => {
                log::debug!("Received SessionEvent for session_id: {}", session_id);
                if let Some(managed_session) =
                    self.session_panes.iter_mut().find(|s| s.id == session_id)
                {
                    Update::with_task(
                        managed_session
                            .update(session_pane::Message::SessionEvent(event))
                            .map(move |msg| Message::SessionPaneUserAction { session_id, msg }),
                    )
                } else {
                    log::warn!(
                        "Received SessionEvent for unknown session_id: {}",
                        session_id
                    );
                    Update::none()
                }
            }
        }
    }

    pub fn view(&self) -> ThemedElement<Message> {
        let session_context = self.create_session_context();
        let toolbar_element = toolbar::view(self.toolbar_expanded, &session_context);

        let main_content_area: ThemedElement<Message> = if self.session_panes.is_empty() {
            container(text("no active sessions").font(assets::fonts::GEIST_VF))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
                .into()
        } else {
            row(self
                .session_panes
                .iter()
                .map(|managed_session| {
                    let session_view_id = managed_session.id;
                    let is_active = self.active_session_id == Some(session_view_id);
                    managed_session
                        .view(self.toolbar_expanded, is_active)
                        .map(move |pane_msg| Message::SessionPaneUserAction {
                            session_id: session_view_id,
                            msg: pane_msg,
                        })
                })
                .collect::<Vec<_>>())
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        };

        let main_layout: ThemedElement<_> = column![
            toolbar_element.map(Message::ToolbarAction),
            main_content_area
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

        if let Some(modal) = &self.modal {
            let modal_view = modal.view().map(Message::ModalMessage);
            stack(vec![
                main_layout,
                opaque(
                    mouse_area(
                        center(opaque(modal_view)).style(theme::builtins::container::overlay),
                    )
                    .on_press(Message::CloseModal),
                ),
            ])
            .into()
        } else {
            main_layout
        }
    }
}
