use crate::components::session_input;
use crate::theme::Element;
use crate::widgets::split_terminal_pane;
use iced::futures::StreamExt;
use iced::widget::{button, column, container, mouse_area, row, stack, text};
use iced::{Length, Padding, Subscription, Task};
use iced_jsx::IcedJsxRoot;
use log::info;
use smudgy_core::models::aliases::load_aliases;
use smudgy_core::models::hotkeys::load_hotkeys;
use smudgy_core::models::profile::load_profile;
use smudgy_core::models::server::load_server;
use smudgy_core::models::settings::load_settings;
use smudgy_core::models::triggers::load_triggers;
use smudgy_core::session::runtime::RuntimeAction;
use smudgy_core::session::{self, SessionEvent, SessionId};
use smudgy_core::session::{BufferUpdate, TaggedSessionEvent};
use smudgy_core::session::{HotkeyId, SessionParams};
use smudgy_core::terminal_buffer::TerminalBuffer;
use smudgy_core::terminal_buffer::selection::Selection;
use smudgy_map::{AreaId, CloudMapper, Mapper};
use smudgy_theme::builtins::container::{opaque};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::mpsc::{self};
use tokio::sync::oneshot;

/// Represents a connection session to a server with a specific profile
pub struct SessionPane {
    pub id: SessionId,
    /// The name of the server this session is connected to
    pub server_name: String,
    /// The name of the profile used for this connection
    pub profile_name: String,
    /// Input component for sending commands
    pub input: session_input::SessionInput,

    pub session_params: Arc<SessionParams>,

    pub mapper: Option<Mapper>,

    terminal_buffer: Rc<RefCell<TerminalBuffer>>,
    terminal_pane_selection: Rc<RefCell<Selection>>,

    jsx_widgets: IcedJsxRoot<'static, (), crate::Theme, crate::Renderer>,

    runtime_tx: Option<mpsc::UnboundedSender<RuntimeAction>>,

    connected: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    Close,
    Activate,
    Input(session_input::Message),
    Send(Arc<String>),
    SessionEvent(SessionEvent),
    HotkeyTriggered(HotkeyId),
    SetMapperCurrentLocation(AreaId, Option<i32>),
    Reload,
    Reconnect,
    WidgetMessage,
}

impl SessionPane {
    /// Creates a new session for the given server and profile
    pub fn new(id: SessionId, server_name: String, profile_name: String) -> Self {
        // Create a single shared terminal buffer
        let terminal_buffer = Rc::new(RefCell::new(TerminalBuffer::new()));

        let settings = load_settings();

        info!("Settings: {:?}", settings);

        // Load profile to get the subtext (caption) once
        let profile_subtext = match load_profile(&server_name, &profile_name) {
            Ok(profile) => Arc::new(profile.config.caption),
            Err(_) => Arc::new(String::new()), // Default to empty string on error
        };

        let mapper = settings.api_key.map(|api_key| {
            Mapper::new(Arc::new(CloudMapper::new(
                "https://api.dev.smudgy.org".to_string(),
                api_key,
            )))
        });

        let jsx_widgets = IcedJsxRoot::new();

        Self {
            id,
            session_params: Arc::new(SessionParams {
                session_id: id,
                server_name: Arc::new(server_name.clone()),
                profile_name: Arc::new(profile_name.clone()),
                profile_subtext,
                mapper: mapper.clone(),
                jsx_widgets: IcedJsxRoot::clone(&jsx_widgets),
            }),
            server_name,
            profile_name,
            input: session_input::SessionInput::new().with_terminal_buffer(terminal_buffer.clone()),
            terminal_buffer: terminal_buffer.clone(),
            terminal_pane_selection: Rc::new(RefCell::new(Selection::default())),
            runtime_tx: None,
            connected: false,
            mapper,
            jsx_widgets,
        }
    }

    /// Returns whether this session is currently connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn jsx_subscription(&self) -> Subscription<SessionId> {
        self.jsx_widgets.subscription(self.id)
    }

    pub fn session_subscription(&self) -> Subscription<TaggedSessionEvent> {
        Subscription::run_with(self.session_params.clone(), |params| {
            session::spawn(params.clone())
        })
    }

    /// Handle session-specific messages
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Close => {
                // Handle session closing - for now, we'll let the parent handle it
                Task::none()
            }
            Message::Activate => {
                // This message is handled by the parent (SmudgyWindow)
                Task::none()
            }
            Message::SetMapperCurrentLocation(_, _) => {
                // This message is handled by the parent (SmudgyWindow)
                Task::none()
            }
            Message::Input(input_msg) => self.input.update(input_msg),
            Message::Send(command) => {
                self.runtime_tx.as_ref().map(|tx| {
                    tx.send(RuntimeAction::Send(command))
                        .expect("Failed to write command to session runtime")
                });
                Task::none()
            }
            Message::HotkeyTriggered(id) => {
                self.runtime_tx.as_ref().map(|tx| {
                    tx.send(RuntimeAction::ExecHotkey { id })
                        .expect("Failed to write command to session runtime")
                });

                Task::none()
            }
            Message::SessionEvent(event) => {
                match event {
                    SessionEvent::RuntimeReady(tx) => {
                        // Load server configuration
                        let server_config = load_server(&self.server_name.as_str())
                            .expect("Failed to load server config");

                        info!("Loading Hotkeys");
                        // Load hotkeys, triggers, and aliases for this server
                        if let Ok(hotkeys) = load_hotkeys(&self.server_name) {
                            for (name, hotkey) in hotkeys {
                                if let Err(e) = tx.send(RuntimeAction::AddHotkey {
                                    name: Arc::new(name),
                                    hotkey,
                                }) {
                                    log::error!("Failed to send hotkey to runtime: {}", e);
                                }
                            }
                        } else {
                            log::warn!("Failed to load hotkeys for server: {}", self.server_name);
                        }

                        info!("Loading Triggers");
                        if let Ok(triggers) = load_triggers(&self.server_name) {
                            for (name, trigger) in triggers {
                                if let Err(e) = tx.send(RuntimeAction::AddTrigger {
                                    name: Arc::new(name),
                                    trigger,
                                }) {
                                    log::error!("Failed to send trigger to runtime: {}", e);
                                }
                            }
                        } else {
                            log::warn!("Failed to load triggers for server: {}", self.server_name);
                        }

                        info!("Loading Aliases");
                        if let Ok(aliases) = load_aliases(&self.server_name) {
                            for (name, alias) in aliases {
                                if let Err(e) = tx.send(RuntimeAction::AddAlias {
                                    name: Arc::new(name),
                                    alias,
                                }) {
                                    log::error!("Failed to send alias to runtime: {}", e);
                                }
                            }
                        } else {
                            log::warn!("Failed to load aliases for server: {}", self.server_name);
                        }

                        self.runtime_tx = Some(tx);

                        if self.connected {
                            return Task::none();
                        } else {
                            return Task::done(Message::Reconnect);
                        }
                    }

                    SessionEvent::UpdateBuffer(buffer_updates) => {
                        for update in buffer_updates.iter() {
                            match update {
                                BufferUpdate::EnsureNewLine => {
                                    self.terminal_buffer.borrow_mut().commit_current_line();
                                }
                                BufferUpdate::Append(line) => {
                                    self.terminal_buffer.borrow_mut().extend_line(line.clone());
                                }
                            }
                        }
                        return Task::none();
                    }
                    SessionEvent::ClearHotkeys => {
                        self.input.clear_hotkeys();
                        return Task::none();
                    }
                    SessionEvent::RegisterHotkey(name, hotkey) => {
                        self.input.register_hotkey(name, hotkey);
                        return Task::none();
                    }
                    SessionEvent::UnregisterHotkey(name) => {
                        self.input.unregister_hotkey(&name);
                        return Task::none();
                    }
                    SessionEvent::PerformLineOperation {
                        line_number,
                        operation,
                    } => {
                        self.terminal_buffer
                            .borrow_mut()
                            .perform_line_operation(line_number, operation);
                        return Task::none();
                    }
                    SessionEvent::SetCurrentLocation(area_id, room_number) => {
                        return Task::done(Message::SetMapperCurrentLocation(area_id, room_number));
                    }
                    SessionEvent::Connected => {
                        self.connected = true;
                        return Task::none();
                    }
                    SessionEvent::Disconnected => {
                        self.connected = false;
                        return Task::done(Message::Reconnect);
                    }
                };
            }
            Message::Reload => {
                self.input.clear_hotkeys();
                self.runtime_tx.as_ref().map(|tx| {
                    tx.send(RuntimeAction::Reload).ok();
                });
                return Task::none();
            }
            Message::Reconnect => {
                info!("Connecting to server");
                self.runtime_tx.as_ref().map(|tx| {
                    let profile_config = load_profile(&self.server_name, &self.profile_name)
                        .expect("Failed to load profile config");


                    let send_on_connect = if profile_config.config.send_on_connect.is_empty() {
                        None
                    } else {
                        Some(Arc::new(profile_config.config.send_on_connect))
                    };

                    let server_config = load_server(&self.server_name.as_str())
                        .expect("Failed to load server config");

                    // Connect to the server
                    tx.send(RuntimeAction::Connect {
                        host: server_config.config.host.into(),
                        port: server_config.config.port,
                        send_on_connect,
                    })
                    .expect("Failed to send connect command to session runtime");
                });

                return Task::none();
            }
            Message::WidgetMessage => {
                return Task::none();
            }
        }
    }

    /// Render the session
    pub fn view(&self, expanded: bool, is_active: bool) -> Element<Message> {
        // Session header with title and close button
        let header = row![
            text(format!("{} ({})", self.profile_name, self.server_name)).size(16),
            button("×").on_press(Message::Close).padding(2)
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

        // Wrap header in mouse_area to handle clicks for activation
        let clickable_header = mouse_area(header).on_press(Message::Activate);

        let terminal = split_terminal_pane::split_terminal_pane(
            self.terminal_buffer.borrow(),
            self.terminal_pane_selection.clone(),
        );

        // Wrap terminal in mouse_area to handle clicks for activation
        let widgets = self.jsx_widgets.view(|| Box::new(opaque)).map(|_| Message::WidgetMessage);
        let clickable_terminal = mouse_area(terminal).on_press(Message::Activate);

        let terminal_area = stack![
            clickable_terminal,
            widgets
        ];

        // Map input messages to session messages
        let input = self.input.view().map(Message::Input);


        // Combine all elements in a column
        let content = if expanded {
            column![clickable_header, terminal_area, input]
                .spacing(10)
                .width(Length::Fill)
                .height(Length::Fill)
        } else {
            column![terminal_area, input]
                .spacing(10)
                .width(Length::Fill)
                .height(Length::Fill)
        };


            container(content)
            .padding(Padding {top: 0.0, right: 10.0, bottom: 10.0, left: 10.0})
            .width(Length::Fill)
            .height(Length::Fill).into()
    }
}

impl Drop for SessionPane {
    fn drop(&mut self) {
        self.runtime_tx.as_ref().map(|tx| {
            tx.send(RuntimeAction::Shutdown).ok();
        });
    }
}
