use iced::futures::{channel::mpsc::Sender, SinkExt, Stream, StreamExt};
use runtime::RuntimeAction;
use std::{fmt::Debug, sync::Arc};
use styled_line::StyledLine;
use tokio::sync::{mpsc::UnboundedSender, oneshot};
use derive_more::{Add, From, Into, Display};

use crate::models::hotkeys::HotkeyDefinition;

pub mod connection;
pub mod registry;
pub mod runtime;
pub mod styled_line;

#[derive(From, Into, Display, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Add)]
#[repr(transparent)]
pub struct SessionId(u32);

#[derive(Debug, Clone)]
pub enum SessionEvent {
    RuntimeReady(UnboundedSender<RuntimeAction>),
    Connected,
    Disconnected,
    UpdateBuffer(Arc<Vec<BufferUpdate>>),
    ClearHotkeys,
    RegisterHotkey(HotkeyId, HotkeyDefinition),
    UnregisterHotkey(HotkeyId),
}
#[derive(Debug, Clone)]
pub struct TaggedSessionEvent {
    pub session_id: SessionId,
    pub event: SessionEvent,
}
#[derive(Debug)]
pub struct SessionParams {
    pub session_id: SessionId,
    pub server_name: Arc<String>,
    pub profile_name: Arc<String>,
    pub profile_subtext: Arc<String>,
}

#[derive(Display, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct HotkeyId(usize);

impl std::hash::Hash for SessionParams {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.session_id.hash(state);
    }
}

#[derive(Debug)]
pub enum BufferUpdate {
    Append(Arc<StyledLine>),
    NewLine,
}

pub fn spawn(params: Arc<SessionParams>) -> impl Stream<Item = TaggedSessionEvent> {
    iced::stream::channel(1024, move |mut ui_tx: Sender<TaggedSessionEvent>| async move {
        if let Err(e) = ui_tx
            .send(TaggedSessionEvent {
                session_id: params.session_id,
                event: SessionEvent::UpdateBuffer(Arc::new(vec![BufferUpdate::Append(Arc::new(
                    StyledLine::from_echo_str("Loading session..."),
                ))])),
            })
            .await
        {
            error!("Failed to send runtime ready event: {:?}", e);
        }

        let runtime = runtime::Runtime::new(
            params.session_id,
            params.server_name.clone(),
            params.profile_name.clone(),
            params.profile_subtext.clone(),
            ui_tx.clone(),
        );

        // Register the runtime in the global registry
        registry::register_session(params.session_id, runtime.into());

    })
}
