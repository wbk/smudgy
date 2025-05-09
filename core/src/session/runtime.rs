use anyhow::{Result, bail};
use rustyscript::extensions::deno_cron::local;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::ops::Add;
use std::rc::Rc;
use std::thread::JoinHandle;
use std::time::SystemTime;
use std::{
    sync::{Arc, Mutex},
    task::Poll,
    thread::{self},
    time::Instant,
};

use deno_core::v8::{self, Global, Handle, script_compiler::Source};

use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
};

mod trigger;
use trigger::Manager;
mod script_action;
mod script_engine;

pub use script_action::ScriptAction;
use script_engine::{FunctionId, ScriptEngine, ScriptEngineParams, ScriptId};

use crate::get_smudgy_home;
use crate::models::ScriptLang;
use crate::models::aliases::AliasDefinition;
use crate::models::hotkeys::HotkeyDefinition;
use crate::models::triggers::TriggerDefinition;
use crate::session::runtime::trigger::PushTriggerParams;
use crate::session::{HotkeyId, registry};

use super::{SessionId, TaggedSessionEvent, connection::Connection, styled_line::StyledLine};

use super::{BufferUpdate, SessionEvent};
use iced::futures::{SinkExt, channel::mpsc::Sender};
#[derive(Clone, Debug)]
pub enum RuntimeAction {
    Connect {
        host: Arc<String>,
        port: u16,
        send_on_connect: Option<Arc<String>>,
    },
    HandleIncomingLine(Arc<StyledLine>),
    HandleIncomingPartialLine(Arc<StyledLine>),
    AddCompleteLineToBuffer(Arc<StyledLine>),
    AddPartialLineToBuffer(Arc<StyledLine>),
    Send(Arc<String>),
    SendRaw(Arc<String>),
    Echo(Arc<String>),
    EvalJavascript {
        id: ScriptId,
        matches: Arc<Vec<(String, String)>>,
        depth: u32,
    },
    CallJavascriptFunction {
        id: FunctionId,
        matches: Arc<Vec<(String, String)>>,
        depth: u32,
    },
    AddHotkey {
        name: Arc<String>,
        hotkey: HotkeyDefinition,
    },
    AddAlias {
        name: Arc<String>,
        alias: AliasDefinition,
    },
    AddJavascriptFunctionAlias {
        name: Arc<String>,
        patterns: Arc<Vec<String>>,
        function_id: FunctionId,
    },
    AddTrigger {
        name: Arc<String>,
        trigger: TriggerDefinition,
    },
    AddJavascriptFunctionTrigger {
        name: Arc<String>,
        patterns: Arc<Vec<String>>,
        raw_patterns: Arc<Vec<String>>,
        anti_patterns: Arc<Vec<String>>,
        function_id: FunctionId,
        prompt: bool,
        enabled: bool,
    },
    EnableAlias(Arc<String>, bool),
    EnableTrigger(Arc<String>, bool),
    ExecHotkey {
        id: HotkeyId,
    },
    RequestRepaint,
    Connected,
    Reload,
    Shutdown,
    Noop,
}

pub struct Runtime {
    pub session_id: SessionId,
    pub server_name: Arc<String>,
    pub profile_name: Arc<String>,
    pub profile_subtext: Arc<String>,
    pub ui_tx: Sender<TaggedSessionEvent>,
    pub tx: UnboundedSender<RuntimeAction>,
    pub oob_tx: UnboundedSender<RuntimeAction>,
}

enum ActionResult {
    None,
    Echo(String),
    Reload,
    CloseSession,
}

enum RunAction {
    None,
    Reload
}

static RUNTIME_THREADS: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

pub fn join_runtime_threads() {
    let mut runtime_threads = RUNTIME_THREADS.lock().unwrap();
    while let Some(join_handle) = runtime_threads.pop() {
        join_handle.join().unwrap();
    }
}

type SentSessionEvent<'a> =
    iced::futures::sink::Send<'a, Sender<TaggedSessionEvent>, TaggedSessionEvent>;

impl Runtime {
    pub fn new(
        session_id: SessionId,
        server_name: Arc<String>,
        profile_name: Arc<String>,
        profile_subtext: Arc<String>,
        ui_tx: Sender<TaggedSessionEvent>,
    ) -> Self {
        let (session_runtime_tx, session_runtime_rx) =
            tokio::sync::mpsc::unbounded_channel::<RuntimeAction>();

        let (session_runtime_oob_tx, session_runtime_oob_rx) =
            tokio::sync::mpsc::unbounded_channel::<RuntimeAction>();

        let local_session_runtime_tx = session_runtime_tx.clone();
        let local_session_runtime_oob_tx = session_runtime_oob_tx.clone();

        let local_server_name = server_name.clone();
        let local_profile_name = profile_name.clone();
        let local_ui_tx = ui_tx.clone();

        let thread = thread::spawn(move || {
            let script_engine = ScriptEngine::new(ScriptEngineParams {
                session_id,
                server_name: &local_server_name,
                profile_name: &local_profile_name,
                ui_tx: local_ui_tx.clone(),
                runtime_oob_tx: local_session_runtime_oob_tx.clone(),
            });

            let trigger_manager = Manager::new(local_session_runtime_oob_tx.clone());

            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            let mut inner = Inner {
                log_file: None,
                session_id: session_id,
                trigger_manager,
                hotkeys: BTreeMap::new(),
                next_hotkey_id: HotkeyId(0),
                script_engine,
                server_name: &local_server_name,
                profile_name: &local_profile_name,
                session_runtime_rx,
                session_runtime_oob_rx,
                session_runtime_tx: local_session_runtime_tx.clone(),
                session_runtime_oob_tx: local_session_runtime_oob_tx.clone(),
                ui_tx: local_ui_tx.clone(),
                connection: None,
                pending_buffer_updates: Vec::new(),
            };

            while let RunAction::Reload = runtime.block_on(inner.run()) {
                info!("Reloading session runtime...");
                
                // Extract the receivers and connection from the old inner before dropping it
                let old_connection = inner.connection.take();
                let old_session_runtime_rx = std::mem::replace(&mut inner.session_runtime_rx, {
                    // Create a dummy receiver that will be immediately replaced
                    let (_, rx) = tokio::sync::mpsc::unbounded_channel();
                    rx
                });
                let old_session_runtime_oob_rx = std::mem::replace(&mut inner.session_runtime_oob_rx, {
                    // Create a dummy receiver that will be immediately replaced  
                    let (_, rx) = tokio::sync::mpsc::unbounded_channel();
                    rx
                });
                
                
                drop(inner);
                
                // Create completely new Inner struct with fresh ScriptEngine and TriggerManager
                // This avoids any V8 isolate replacement issues
                let new_script_engine = ScriptEngine::new(ScriptEngineParams {
                    session_id,
                    server_name: &local_server_name,
                    profile_name: &local_profile_name,
                    ui_tx: local_ui_tx.clone(),
                    runtime_oob_tx: local_session_runtime_oob_tx.clone(),
                });

                let new_trigger_manager = Manager::new(local_session_runtime_oob_tx.clone());

                // Replace with the new inner struct
                inner = Inner {
                    log_file: None, // Will restart logging
                    session_id: session_id,
                    trigger_manager: new_trigger_manager,
                    hotkeys: BTreeMap::new(), // Reset hotkeys - they'll be re-registered by modules
                    next_hotkey_id: HotkeyId(0),
                    script_engine: new_script_engine,
                    server_name: &local_server_name,
                    profile_name: &local_profile_name,
                    session_runtime_rx: old_session_runtime_rx,
                    session_runtime_oob_rx: old_session_runtime_oob_rx,
                    session_runtime_tx: local_session_runtime_tx.clone(),
                    session_runtime_oob_tx: local_session_runtime_oob_tx.clone(),
                    ui_tx: local_ui_tx.clone(),
                    connection: old_connection, // Preserve the connection
                    pending_buffer_updates: Vec::new(),
                };
                
                info!("Session runtime reloaded successfully");
            }

            registry::unregister_session(session_id);

            drop(inner);
        });

        RUNTIME_THREADS.lock().unwrap().push(thread);

        Self {
            session_id,
            server_name,
            profile_name,
            profile_subtext,
            ui_tx,
            tx: session_runtime_tx,
            oob_tx: session_runtime_oob_tx,
        }
    }

    pub fn tx(&self) -> UnboundedSender<RuntimeAction> {
        self.tx.clone()
    }
}

struct Inner<'a> {
    session_id: SessionId,
    trigger_manager: trigger::Manager,
    script_engine: ScriptEngine<'a>,
    server_name: &'a Arc<String>,
    profile_name: &'a Arc<String>,
    session_runtime_rx: UnboundedReceiver<RuntimeAction>,
    session_runtime_oob_rx: UnboundedReceiver<RuntimeAction>,
    session_runtime_tx: UnboundedSender<RuntimeAction>,
    session_runtime_oob_tx: UnboundedSender<RuntimeAction>,
    ui_tx: Sender<TaggedSessionEvent>,
    connection: Option<Connection>,
    pending_buffer_updates: Vec<BufferUpdate>,
    hotkeys: BTreeMap<HotkeyId, ScriptAction>,
    next_hotkey_id: HotkeyId,
    log_file: Option<BufWriter<File>>,
}

impl<'a> Inner<'a> {
    fn echo_warn_str<'s>(
        &'s mut self,
        line: &str,
    ) -> Result<Option<SentSessionEvent<'s>>, anyhow::Error> {
        let styled_line = Arc::new(StyledLine::from_warn_str(line));

        self.pending_buffer_updates.push(BufferUpdate::NewLine);
        self.pending_buffer_updates
            .push(BufferUpdate::Append(styled_line));
        self.pending_buffer_updates.push(BufferUpdate::NewLine);

        Ok(self.flush_buffer_updates()?)
    }

    fn echo_str<'s>(
        &'s mut self,
        line: &str,
    ) -> Result<Option<SentSessionEvent<'s>>, anyhow::Error> {
        let styled_line = Arc::new(StyledLine::from_echo_str(line));

        self.pending_buffer_updates.push(BufferUpdate::NewLine);
        self.pending_buffer_updates
            .push(BufferUpdate::Append(styled_line));
        self.pending_buffer_updates.push(BufferUpdate::NewLine);

        Ok(self.flush_buffer_updates()?)
    }

    fn send<'s>(&'s mut self, line: &str) -> Result<Option<SentSessionEvent<'s>>, anyhow::Error> {
        let mut socket_str = String::with_capacity(line.len() + 2);
        socket_str.push_str(line);
        socket_str.push_str("\r\n");
        let arc_socket_str = Arc::new(socket_str);

        let styled_line = Arc::new(StyledLine::from_output_str(line));

        self.pending_buffer_updates
            .push(BufferUpdate::Append(styled_line));
        self.pending_buffer_updates.push(BufferUpdate::NewLine);

        if let Some(ref connection) = self.connection {
            if let Err(e) = connection.write(arc_socket_str) {
                warn!("Error writing to connection: {e:?}");
                self.echo_warn_str(format!("Send error: {e:?}").as_str())?;
            }
        }

        Ok(self.flush_buffer_updates()?)
    }

    fn flush_buffer_updates<'s>(
        &'s mut self,
    ) -> Result<Option<SentSessionEvent<'s>>, anyhow::Error> {
        if self.pending_buffer_updates.is_empty() {
            return Ok(None);
        }

        if let Some(log_file) = self.log_file.as_mut() {
            for update in self.pending_buffer_updates.iter() {
                match update {
                    BufferUpdate::NewLine => {
                        log_file.write_all(b"\n")?;
                    }
                    BufferUpdate::Append(line) => {
                        log_file.write_all(line.as_bytes())?;
                    }
                }
            }
            log_file.flush()?;
        }

        Ok(Some(self.ui_tx.send(TaggedSessionEvent {
            session_id: self.session_id,
            event: SessionEvent::UpdateBuffer(Arc::new(
                self.pending_buffer_updates.drain(..).collect(),
            )),
        })))
    }

    #[inline]
    #[allow(clippy::unused_async)]
    async fn handle_action(
        &mut self,
        action: RuntimeAction,
    ) -> Result<ActionResult, anyhow::Error> {
        debug!("Handling action: {:?}", action);
        match action {
            RuntimeAction::Connect {
                host,
                port,
                send_on_connect,
            } => {
                let mut connection =
                    Connection::new(self.session_runtime_tx.clone(), self.ui_tx.clone());

                if let Some(send_on_connect) = send_on_connect {
                    let local_tx = self.session_runtime_tx.clone();
                    let mut local_ui_tx = self.ui_tx.clone();
                    let session_id = self.session_id;
                    connection.on_connect(move || {                        
                        local_tx.send(RuntimeAction::Send(send_on_connect)).ok();                        
                    });
                }
                connection.connect(host.as_str(), port);

                self.connection = Some(connection);
                Ok(ActionResult::None)
            }
            RuntimeAction::HandleIncomingLine(line) => {
                match self.trigger_manager.process_incoming_line(line) {
                    Ok(()) => Ok(ActionResult::None),
                    Err(err) => Ok(ActionResult::Echo(format!("Error processing line {err:?}"))),
                }
            }
            RuntimeAction::HandleIncomingPartialLine(line) => {
                match self.trigger_manager.process_partial_line(line) {
                    Ok(()) => Ok(ActionResult::None),
                    Err(err) => Ok(ActionResult::Echo(format!(
                        "Error processing partial line {err:?}"
                    ))),
                }
            }
            RuntimeAction::RequestRepaint => {
                if let Some(fut) = self.flush_buffer_updates()? {
                    fut.await?;
                }
                Ok(ActionResult::None)
            }
            RuntimeAction::Echo(line) => {
                if let Some(fut) = self.echo_str(line.as_str())? {
                    fut.await?;
                }
                Ok(ActionResult::None)
            }
            RuntimeAction::AddCompleteLineToBuffer(line) => {
                self.pending_buffer_updates.push(BufferUpdate::Append(line));
                self.pending_buffer_updates.push(BufferUpdate::NewLine);
                Ok(ActionResult::None)
            }
            RuntimeAction::AddPartialLineToBuffer(line) => {
                self.pending_buffer_updates.push(BufferUpdate::Append(line));
                Ok(ActionResult::None)
            }
            RuntimeAction::Send(line) => {
                match self.trigger_manager.process_outgoing_line(line.as_str()) {
                    Ok(()) => Ok(ActionResult::None),
                    Err(err) => Ok(ActionResult::Echo(format!(
                        "Error processing command {err:?}"
                    ))),
                }
            }
            RuntimeAction::SendRaw(str) => {
                for line in str.split('\n') {
                    if let Some(fut) = self.send(line)? {
                        fut.await?;
                    }
                }
                if let Some(fut) = self.flush_buffer_updates()? {
                    fut.await?;
                }
                Ok(ActionResult::None)
            }
            RuntimeAction::EvalJavascript { id, matches, depth } => Ok(self
                .script_engine
                .run_script(&self.trigger_manager, id, &matches, depth)
                .unwrap_or_else(|err| ActionResult::Echo(format!("JavaScript Error: {:?}", err)))),
            RuntimeAction::CallJavascriptFunction { id, matches, depth } => Ok(self
                .script_engine
                .call_javascript_function(&self.trigger_manager, id, &matches, depth)
                .unwrap_or_else(|err| {
                    ActionResult::Echo(format!("Error in Javascript Function: {:?}", err))
                })),
            RuntimeAction::AddHotkey {
                name: _name,
                hotkey,
            } => {
                let hotkey_id = self.next_hotkey_id;
                self.next_hotkey_id.0 = self.next_hotkey_id.0.add(1);
                let action = match hotkey.language {
                    ScriptLang::Plaintext => {
                        ScriptAction::SendSimple(hotkey.script.clone().unwrap_or_default().into())
                    }
                    ScriptLang::JS | ScriptLang::TS => {
                        let script_id = self
                            .script_engine
                            .add_script(hotkey.script.as_ref().map_or("", |s| s.as_str()))?;
                        ScriptAction::EvalJavascript(script_id)
                    }
                };
                self.hotkeys.insert(hotkey_id, action);
                self.ui_tx
                    .send(TaggedSessionEvent {
                        session_id: self.session_id,
                        event: SessionEvent::RegisterHotkey(hotkey_id, hotkey),
                    })
                    .await?;

                Ok(ActionResult::None)
            }
            RuntimeAction::ExecHotkey { id } => {
                if let Some(action) = self.hotkeys.get(&id) {
                    match action {
                        ScriptAction::SendRaw(script) => {
                            if let Some(fut) = self.send(script.clone().as_str())? {
                                fut.await?;
                            }
                            Ok(ActionResult::None)
                        }
                        ScriptAction::SendSimple(script) => {
                            match self.trigger_manager.process_outgoing_line(script.as_str()) {
                                Ok(()) => Ok(ActionResult::None),
                                Err(err) => Ok(ActionResult::Echo(format!(
                                    "Error processing command {err:?}"
                                ))),
                            }
                        }
                        ScriptAction::EvalJavascript(script_id) => {
                            self.script_engine
                                .run_script(&self.trigger_manager, *script_id, &Arc::new(vec![]), 0)
                                .unwrap_or_else(|err| {
                                    ActionResult::Echo(format!(
                                        "Error in Javascript Function: {:?}",
                                        err
                                    ))
                                });

                            Ok(ActionResult::None)
                        }
                        ScriptAction::CallJavascriptFunction(function_id) => {
                            self.script_engine
                                .call_javascript_function(
                                    &self.trigger_manager,
                                    *function_id,
                                    &Arc::new(vec![]),
                                    0,
                                )
                                .unwrap_or_else(|err| {
                                    ActionResult::Echo(format!(
                                        "Error calling Javascript Function: {:?}",
                                        err
                                    ))
                                });

                            Ok(ActionResult::None)
                        }
                        ScriptAction::Noop => Ok(ActionResult::None),
                    }
                } else {
                    bail!("Hotkey {id} not found")
                }
            }
            RuntimeAction::AddAlias { name, alias } => {
                match alias.language {
                    ScriptLang::Plaintext => {
                        self.trigger_manager.push_simple_alias(
                            name,
                            Arc::new(vec![alias.pattern]),
                            alias.script.unwrap_or_default().into(),
                        )?;
                    }
                    ScriptLang::JS | ScriptLang::TS => {
                        let script_id = self
                            .script_engine
                            .add_script(alias.script.unwrap_or_default().as_str())?;
                        self.trigger_manager.push_javascript_alias(
                            &name,
                            &Arc::new(vec![alias.pattern]),
                            script_id,
                        )?;
                    }
                };

                Ok(ActionResult::None)
            }
            RuntimeAction::AddJavascriptFunctionAlias {
                name,
                patterns,
                function_id,
            } => {
                self.trigger_manager
                    .push_javascript_function_alias(name, patterns, function_id)?;
                Ok(ActionResult::None)
            }
            RuntimeAction::AddTrigger { name, trigger } => {
                let action = match trigger.language {
                    ScriptLang::Plaintext => {
                        ScriptAction::SendSimple(trigger.script.unwrap_or_default().into())
                    }
                    ScriptLang::JS | ScriptLang::TS => {
                        let script_id = self
                            .script_engine
                            .add_script(trigger.script.unwrap_or_default().as_str())?;
                        ScriptAction::EvalJavascript(script_id)
                    }
                };

                self.trigger_manager.push_trigger(PushTriggerParams {
                    name: &name,
                    patterns: &Arc::new(trigger.patterns.unwrap_or_default()),
                    raw_patterns: &Arc::new(trigger.raw_patterns.unwrap_or_default()),
                    anti_patterns: &Arc::new(trigger.anti_patterns.unwrap_or_default()),
                    action: action,
                    enabled: trigger.enabled,
                    prompt: trigger.prompt,
                })?;
                Ok(ActionResult::None)
            }
            RuntimeAction::AddJavascriptFunctionTrigger {
                name,
                patterns,
                raw_patterns,
                anti_patterns,
                function_id,
                prompt,
                enabled,
            } => {
                self.trigger_manager.push_trigger(PushTriggerParams {
                    name: &name,
                    patterns: &patterns,
                    raw_patterns: &raw_patterns,
                    anti_patterns: &anti_patterns,
                    action: ScriptAction::CallJavascriptFunction(function_id),
                    enabled,
                    prompt,
                })?;
                Ok(ActionResult::None)
            }
            RuntimeAction::EnableAlias(name, enabled) => {
                self.trigger_manager.enable_alias(&name, enabled);
                Ok(ActionResult::None)
            }
            RuntimeAction::EnableTrigger(name, enabled) => {
                self.trigger_manager.enable_trigger(&name, enabled);
                Ok(ActionResult::None)
            }
            RuntimeAction::Connected => {
                self.ui_tx.send(TaggedSessionEvent {
                    session_id: self.session_id,
                    event: SessionEvent::Connected,
                }).await?;
                Ok(ActionResult::None)
            }
            RuntimeAction::Reload => {
                Ok(ActionResult::Reload)
            },
            RuntimeAction::Shutdown => Ok(ActionResult::CloseSession),
            RuntimeAction::Noop => Ok(ActionResult::None),
        }
    }

    pub async fn run(&mut self) -> RunAction {
        let mut script_engine_tick_interval = ScriptEngine::tick_interval();

        // TODO: make logging configurable
        self.start_logging().unwrap();

        info!(
            "Session [{}, {} - {}] Started",
            self.session_id, self.server_name, self.profile_name
        );

        
        if let Err(e) = self.ui_tx
            .send(TaggedSessionEvent {
                session_id: self.session_id,
                event: SessionEvent::RuntimeReady(self.session_runtime_tx.clone()),
            })
            .await
        {
            error!("Failed to send runtime ready event: {:?}", e);
        }

        loop {
            std::future::poll_fn(|cx| {
                Poll::Ready(match self.script_engine.poll_event_loop(cx) {
                    Poll::Ready(t) => t.map(|()| false),
                    Poll::Pending => Ok(true),
                })
            })
            .await
            .map_err(|err| {
                warn!("Error in script engine event loop: {err:?}");
                self.echo_warn_str(format!("{err:?}").as_str())
            })
            .ok();

            select! {
                biased;
                _ = script_engine_tick_interval.tick() => {
                    // this serves to trigger a cancel on the pending receive below when it's time
                    // for the event loop above to tick
                }
                Some(action) = self.session_runtime_oob_rx.recv() => {
                    trace!("Handling OOB action: {action:?}");
                    match self.handle_action(action).await {
                        Ok(ActionResult::None) => {}
                        Ok(ActionResult::Echo(line)) => {
                            if let Ok(Some(fut)) = self.echo_str(line.as_str()) {
                                if let Err(_) = fut.await {
                                    warn!("Failed to echo line");
                                    break;
                                }
                            } else {
                                warn!("Failed to echo line");
                                break;
                            }
                        }
                        Ok(ActionResult::CloseSession) => {
                            info!("Session [{}, {} - {}] Closing", self.session_id, self.server_name, self.profile_name);
                            break;
                        }
                        Ok(ActionResult::Reload) => {
                            return RunAction::Reload;
                        }
                        Err(err) => {
                            warn!("Error in script runtime: {err:?}, ending");
                            break;
                        }
                    }
                }
                Some(action) = self.session_runtime_rx.recv() => {
                    trace!("Handling action: {action:?}");
                    match self.handle_action(action).await {
                        Ok(ActionResult::None) => {}
                        Ok(ActionResult::Echo(line)) => {
                            if let Ok(Some(fut)) = self.echo_str(line.as_str()) {
                                if let Err(_) = fut.await {
                                    warn!("Failed to echo line");
                                    break;
                                }
                            } else {
                                warn!("Failed to echo line");
                                break;
                            }
                        }
                        Ok(ActionResult::CloseSession) => {
                            info!("Session [{}, {} - {}] Closing", self.session_id, self.server_name, self.profile_name);
                            break;
                        }
                        Ok(ActionResult::Reload) => {
                            return RunAction::Reload;
                        }
                         Err(err) => {
                            warn!("Error in script runtime: {err:?}, ending");
                            break;
                        }
                    }
                }
            }
        }

        RunAction::None
    }

    fn start_logging(&mut self) -> Result<()> {
        let path = get_smudgy_home()?
            .join(self.server_name.as_str())
            .join("logs")
            .join(format!(
                "{}-{}.log",
                self.profile_name,
                chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
            ));
        self.log_file = Some(BufWriter::with_capacity(65536, File::create(path)?));
        Ok(())
    }
}
