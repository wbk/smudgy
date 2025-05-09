#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::pedantic)]
use std::collections::BTreeMap;
use std::ops::Deref;
use std::sync::Arc;

use futures::StreamExt;
use iced::daemon::Title;
use iced::futures::FutureExt;
use iced::widget::{center, text};
use iced::window;
use iced::window::settings::PlatformSpecific;
use iced::{Size, Subscription, Task, futures};
use theme::Element;

// Core session imports
use windows::script_editor_window::{self, Event as ScriptEditorWindowEvent, ScriptEditorWindow};
use windows::smudgy_window::SmudgyWindow;

mod assets;
mod modal;
mod theme;
mod toolbar;
mod widgets;

mod components;

mod windows {
    pub mod map_editor_window;
    pub mod script_editor_window;
    pub mod settings_window;
    pub mod smudgy_window;
}

mod helpers {
    pub mod hotkeys;
}

use windows::smudgy_window::Event as SmudgyWindowEvent;

use crate::components::session_pane;
use crate::windows::smudgy_window;

extern crate log;

pub type Theme = crate::theme::Theme;
pub type Renderer = iced::Renderer;

// Main application state
struct Smudgy {
    smudgy_windows: BTreeMap<window::Id, SmudgyWindow>,
    script_editor_windows: BTreeMap<window::Id, ScriptEditorWindow>,
}

impl Default for Smudgy {
    fn default() -> Self {
        Self {
            smudgy_windows: BTreeMap::new(),
            script_editor_windows: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    CloseWindow(window::Id),
    SmudgyWindowMessage(window::Id, windows::smudgy_window::Message),
    NewSmudgyWindow(window::Id),
    CreateSmudgyWindow,
    ScriptEditorWindowMessage(window::Id, windows::script_editor_window::Message),
    NewScriptEditorWindow {
        id: window::Id,
        server_name: Arc<String>,
    },
    CreateScriptEditorWindow {
        server_name: Arc<String>,
    },
}

fn init() -> (Smudgy, Task<Message>) {
    let (_id, open) = window::open(window::Settings {
        exit_on_close_request: true,
        ..Default::default()
    });

    (
        Smudgy {
            smudgy_windows: BTreeMap::new(),
            script_editor_windows: BTreeMap::new(),
        },
        open.map(Message::NewSmudgyWindow),
    )
}

fn main() -> anyhow::Result<()> {
    smudgy_core::init();

    iced::daemon(init, update, view)
        .theme(|smudgy, window_id| {
            if smudgy.smudgy_windows.contains_key(&window_id) {
                theme::smudgy()
            } else {
                theme::secondary()
            }
        })
        .subscription(subscription)
        .font(assets::fonts::GEIST_VF_BYTES)
        .font(assets::fonts::GEIST_MONO_VF_BYTES)
        .font(assets::fonts::BOOTSTRAP_ICONS_BYTES)
        .default_font(assets::fonts::GEIST_VF)
        .title(|smudgy: &Smudgy, window_id: window::Id| {
            match smudgy.script_editor_windows.get(&window_id) {
                Some(window) => {
                    format!("smudgy automations - {}", window.server_name())
                }
                None => {
                    "smudgy".to_string()
                }
            }
        })
        .run()?;

    log::info!("Application closing");

    smudgy_core::session::runtime::join_runtime_threads();

    Ok(())
}

fn subscription(smudgy: &Smudgy) -> Subscription<Message> {
    Subscription::batch([
        Subscription::batch(
            smudgy
                .smudgy_windows
                .iter()
                .map(|(id, window)| window.subscription().with(*id)),
        )
        .map(|(id, msg)| Message::SmudgyWindowMessage(id, msg)),
        window::close_events().map(Message::CloseWindow),
    ])
}

fn update(smudgy: &mut Smudgy, message: Message) -> Task<Message> {
    match message {
        Message::CloseWindow(id) => {
            if smudgy.smudgy_windows.get(&id).is_some() {
                smudgy.smudgy_windows.remove(&id);
                if smudgy.smudgy_windows.is_empty() {
                    iced::exit()
                } else {
                    Task::none()
                }
            } else if smudgy.script_editor_windows.get(&id).is_some() {
                smudgy.script_editor_windows.remove(&id);
                Task::none()
            } else {
                Task::none()
            }
        }
        Message::SmudgyWindowMessage(id, msg) => {
            if let Some(window) = smudgy.smudgy_windows.get_mut(&id) {
                let update = window.update(msg);

                match update.event {
                    Some(SmudgyWindowEvent::CreateNewScriptEditorWindow { server_name }) => {
                        Task::batch([
                            update
                                .task
                                .map(move |message| Message::SmudgyWindowMessage(id, message)),
                            Task::done(Message::CreateScriptEditorWindow { server_name }),
                        ])
                    }
                    _ => update
                        .task
                        .map(move |message| Message::SmudgyWindowMessage(id, message)),
                }
            } else {
                log::warn!("Received message for unknown window index: {}", id);
                Task::none()
            }
        }
        Message::CreateSmudgyWindow => {
            let (_, task) = window::open(window::Settings::default());
            task.map(Message::NewSmudgyWindow)
        }
        Message::NewSmudgyWindow(id) => {
            smudgy
                .smudgy_windows
                .insert(id, windows::smudgy_window::SmudgyWindow::new());
            Task::none()
        }
        Message::ScriptEditorWindowMessage(id, msg) => {
            if let Some(window) = smudgy.script_editor_windows.get_mut(&id) {
                let update = window
                    .update(msg)
                    .map_message(move |msg| Message::ScriptEditorWindowMessage(id, msg));

                match update.event {
                    Some(ScriptEditorWindowEvent::ScriptsChanged { server_name }) => {
                        let reload_tasks = smudgy.smudgy_windows.iter().flat_map(|(id, window)| {
                            window.session_panes().iter().filter_map(|pane| {
                                if pane.server_name == server_name {
                                    Some(Task::done(Message::SmudgyWindowMessage(
                                        *id,
                                        smudgy_window::Message::SessionPaneUserAction {
                                            session_id: pane.id,
                                            msg: session_pane::Message::Reload,
                                        },
                                    )))
                                } else {
                                    None
                                }
                            })
                        });

                        Task::batch([update.task, Task::batch(reload_tasks)])
                    }
                    None => update.task,
                }
            } else {
                log::warn!("Received message for unknown window index: {}", id);
                Task::none()
            }
        }
        Message::CreateScriptEditorWindow { server_name } => {
            let (_, task) = window::open(window::Settings {
                min_size: Some(Size::new(600.0, 400.0)),
                ..Default::default()
            });
            task.map(move |id| Message::NewScriptEditorWindow {
                id,
                server_name: server_name.clone(),
            })
        }
        Message::NewScriptEditorWindow { id, server_name } => {
            let window = ScriptEditorWindow::new(server_name.to_string());
            let task = window.init();
            smudgy.script_editor_windows.insert(id, window);

            task.map(move |message| Message::ScriptEditorWindowMessage(id, message))
        }
    }
}

fn view(smudgy: &Smudgy, id: window::Id) -> Element<Message> {
    if let Some(window) = smudgy.smudgy_windows.get(&id) {
        center(
            window
                .view()
                .map(move |message| Message::SmudgyWindowMessage(id, message)),
        )
        .into()
    } else if let Some(window) = smudgy.script_editor_windows.get(&id) {
        center(
            window
                .view()
                .map(move |message| Message::ScriptEditorWindowMessage(id, message)),
        )
        .into()
    } else {
        text("No windows open").into()
    }
}
