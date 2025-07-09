use anyhow::{Result, bail};
use rustyscript::extensions::deno_cron::local;
use smudgy_map::{AreaId, Mapper};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};
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
pub mod line_operation;
mod script_action;
mod script_engine;

use line_operation::LineOperation;

pub use script_action::ScriptAction;
use script_engine::{FunctionId, ScriptEngine, ScriptEngineParams, ScriptId};

use crate::get_smudgy_home;
use crate::models::ScriptLang;
use crate::models::aliases::AliasDefinition;
use crate::models::hotkeys::HotkeyDefinition;
use crate::models::triggers::TriggerDefinition;
use crate::session::runtime::trigger::{PushTriggerParams, line_splitter};
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
    CompleteLineTriggersProcessed(Arc<StyledLine>),
    PartialLineTriggersProcessed(Arc<StyledLine>),
    PerformLineOperation { line_number: usize, operation: LineOperation },
    Send(Arc<String>),
    SendRaw(Arc<String>),
    ProcessOutgoingLine(Arc<String>),
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
    SelectMapperArea(AreaId),
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
    Run(Vec<RuntimeAction>),
}

enum RunAction {
    None,
    Reload,
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
        mapper: Option<Mapper>,
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
            let pending_line_operations = Rc::new(RefCell::new(Vec::new()));

            // We start at 1 because the first line ("Loading session...") is already emitted
            let emitted_line_count = Rc::new(Cell::new(0));

            let script_engine = ScriptEngine::new(ScriptEngineParams {
                session_id,
                server_name: &local_server_name,
                profile_name: &local_profile_name,
                ui_tx: local_ui_tx.clone(),
                runtime_oob_tx: local_session_runtime_oob_tx.clone(),
                pending_line_operations: &pending_line_operations,
                emitted_line_count: Rc::downgrade(&emitted_line_count),
                mapper: mapper.clone(),
            });

            let trigger_manager = Manager::new(local_session_runtime_oob_tx.clone());

            let runtime = tokio::runtime::Builder::new_current_thread()
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
                mapper: mapper.clone(),
                session_runtime_rx,
                session_runtime_oob_rx,
                session_runtime_tx: local_session_runtime_tx.clone(),
                session_runtime_oob_tx: local_session_runtime_oob_tx.clone(),
                ui_tx: local_ui_tx.clone(),
                connection: None,
                pending_buffer_updates: Vec::new(),
                pending_line_operations: pending_line_operations.clone(),
                emitted_line_count: emitted_line_count.clone(),
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
                let old_session_runtime_oob_rx =
                    std::mem::replace(&mut inner.session_runtime_oob_rx, {
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
                    pending_line_operations: &pending_line_operations,
                    emitted_line_count: Rc::downgrade(&emitted_line_count),
                    mapper: mapper.clone(),
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
                    pending_line_operations: pending_line_operations.clone(), // Preserve the shared operations
                    emitted_line_count: emitted_line_count.clone(),
                    mapper: mapper.clone(),
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
    pending_line_operations: Rc<RefCell<Vec<LineOperation>>>,
    emitted_line_count: Rc<Cell<usize>>,
    mapper: Option<Mapper>,
}

impl<'a> Inner<'a> {
    /// Applies all pending line operations to the given line and clears the operations queue.
    /// Returns None if any operation gags the line, otherwise returns the processed line.
    fn apply_pending_line_operations(
        &self,
        line: Arc<StyledLine>,
    ) -> Result<Option<Arc<StyledLine>>, anyhow::Error> {
        let mut operations = self.pending_line_operations.borrow_mut();

        // If no operations are pending, return the line unchanged
        if operations.is_empty() {
            return Ok(Some(line));
        }

        // Collect all operations and clear the queue
        let operations_to_apply: Vec<LineOperation> = operations.drain(..).collect();
        drop(operations); // Release the lock early

        // Apply each operation in sequence
        let mut current_line = line;
        for operation in operations_to_apply {
            match operation.apply(current_line) {
                Some(processed_line) => current_line = processed_line,
                None => {
                    // Line was gagged, return None immediately
                    return Ok(None);
                }
            }
        }

        Ok(Some(current_line))
    }

    #[inline]
    fn echo_warn_str_sync<'s>(
        &'s mut self,
        line: &str,
    ) {
        if self.pending_buffer_updates.last().map_or(false, |update| match update {
            BufferUpdate::EnsureNewLine => false,
            _ => true,
        }) {
            self.pending_buffer_updates.push(BufferUpdate::EnsureNewLine);
            self.emitted_line_count.set(self.emitted_line_count.get() + 1);
        }

        for line in line.split('\n') {
            let styled_line = Arc::new(StyledLine::from_warn_str(line));
            self.pending_buffer_updates
                .push(BufferUpdate::Append(styled_line));
            self.pending_buffer_updates.push(BufferUpdate::EnsureNewLine);
            self.emitted_line_count.set(self.emitted_line_count.get() + 1);
        }
    }

    fn echo_warn_str<'s>(
        &'s mut self,
        line: &str,
    ) -> Result<Option<SentSessionEvent<'s>>, anyhow::Error> {
        self.echo_warn_str_sync(line); 
        Ok(self.flush_buffer_updates()?)
    }

    #[inline]
    fn echo_str_sync<'s>(
        &'s mut self,
        line: &str,
    )  {
        if self.pending_buffer_updates.last().map_or(false, |update| match update {
            BufferUpdate::EnsureNewLine => false,
            _ => true,
        }) {
            self.pending_buffer_updates.push(BufferUpdate::EnsureNewLine);
            self.emitted_line_count.set(self.emitted_line_count.get() + 1);
        }

        for line in line.split('\n') {
            let styled_line = Arc::new(StyledLine::from_echo_str(line));
            self.pending_buffer_updates
                .push(BufferUpdate::Append(styled_line));
            self.pending_buffer_updates.push(BufferUpdate::EnsureNewLine);
            self.emitted_line_count.set(self.emitted_line_count.get() + 1);
        }
    }

    fn echo_str<'s>(
        &'s mut self,
        line: &str,
    ) -> Result<Option<SentSessionEvent<'s>>, anyhow::Error> {
        self.echo_str_sync(line);
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
        self.pending_buffer_updates.push(BufferUpdate::EnsureNewLine);

        self.emitted_line_count.set(self.emitted_line_count.get() + 1);


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
                    BufferUpdate::EnsureNewLine => {
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
                    connection.on_connect(move || {
                        local_tx.send(RuntimeAction::Send(send_on_connect)).ok();
                    });
                }
                connection.connect(host.as_str(), port);

                self.connection = Some(connection);
                Ok(ActionResult::None)
            }
            RuntimeAction::HandleIncomingLine(line) => {
                self.script_engine
                    .set_current_line(Some(Arc::downgrade(&line)));
                match self.trigger_manager.process_incoming_line(line) {
                    Ok(()) => Ok(ActionResult::None),
                    Err(err) => Ok(ActionResult::Echo(format!("Error processing line {err:?}"))),
                }
            }
            RuntimeAction::HandleIncomingPartialLine(line) => {
                self.script_engine
                    .set_current_line(Some(Arc::downgrade(&line)));
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
            RuntimeAction::CompleteLineTriggersProcessed(line) => {
                // Apply pending line operations first
                self.script_engine.set_current_line(None);
                let processed_line = self.apply_pending_line_operations(line)?;
                if let Some(processed_line) = processed_line {
                    self.pending_buffer_updates
                        .push(BufferUpdate::Append(processed_line));
                    self.pending_buffer_updates.push(BufferUpdate::EnsureNewLine);

                    self.emitted_line_count.set(self.emitted_line_count.get() + 1);

                }
                Ok(ActionResult::None)
            }
            RuntimeAction::PartialLineTriggersProcessed(line) => {
                // Apply pending line operations first
                self.script_engine.set_current_line(None);
                let processed_line = self.apply_pending_line_operations(line)?;
                if let Some(processed_line) = processed_line {
                    self.pending_buffer_updates
                        .push(BufferUpdate::Append(processed_line));
                }
                Ok(ActionResult::None)
            }
            RuntimeAction::Send(line) => {
                if line.starts_with('=') {
                    match self.trigger_manager.process_outgoing_line(&line[1..]) {
                        Ok(()) => Ok(ActionResult::None),
                        Err(err) => Ok(ActionResult::Echo(format!(
                            "Error processing command {err:?}"
                        ))),
                    }
                } else {
                    Ok(ActionResult::Run(
                    line.split(trigger::line_splitter)
                        .map(|line| RuntimeAction::ProcessOutgoingLine(Arc::new(line.to_string())))
                        .collect()    ))
                }
                    
            },
            RuntimeAction::ProcessOutgoingLine(line) => {
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
                        match self
                            .script_engine
                            .add_script(hotkey.script.as_ref().map_or("", |s| s.as_str()))
                        {
                            Ok(script_id) => ScriptAction::EvalJavascript(script_id),
                            Err(err) => {
                                self.echo_warn_str(
                                    format!("Error adding script: {:?}", err).as_str(),
                                )?;
                                ScriptAction::Noop
                            }
                        }
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
                        ScriptAction::SendSimple(script) => Ok(ActionResult::Run(
                            script
                                .split(line_splitter)
                                .map(|line| {
                                    RuntimeAction::ProcessOutgoingLine(Arc::new(line.to_string()))
                                })
                                .collect(),
                        )),
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
                self.ui_tx
                    .send(TaggedSessionEvent {
                        session_id: self.session_id,
                        event: SessionEvent::Connected,
                    })
                    .await?;
                Ok(ActionResult::None)
            }
            RuntimeAction::PerformLineOperation { line_number, operation } => {

                self.ui_tx.send(TaggedSessionEvent {
                    session_id: self.session_id,
                    event: SessionEvent::PerformLineOperation { line_number, operation },
                }).await?;
                Ok(ActionResult::None)
            }
            RuntimeAction::SelectMapperArea(id) => {
                self.ui_tx.send(TaggedSessionEvent {
                    session_id: self.session_id,
                    event: SessionEvent::SelectMapperArea(id)
                }).await?;
                Ok(ActionResult::None)
            }
            RuntimeAction::Reload => Ok(ActionResult::Reload),
            RuntimeAction::Shutdown => Ok(ActionResult::CloseSession),
            RuntimeAction::Noop => Ok(ActionResult::None),
        }
    }

    pub async fn run(&mut self) -> RunAction {
        let mut script_engine_tick_interval = ScriptEngine::tick_interval();

        // Stack-based action processing
        let mut action_stack: Vec<VecDeque<RuntimeAction>> = Vec::new();
        const MAX_STACK_DEPTH: usize = 100;

        // TODO: make logging configurable
        self.start_logging().unwrap();

        info!(
            "Session [{}, {} - {}] Started",
            self.session_id, self.server_name, self.profile_name
        );

        if let Some(mapper) = self.mapper.clone() {
            self.echo_str_sync("Loading maps...");
            if let Err(e) = mapper.load_all_areas().await {
                self.echo_warn_str_sync(&format!("Failed to load all maps: {:?}", e));
            }

            let atlas = mapper.get_current_atlas();

            for area in atlas.areas() {
                self.echo_str_sync(&format!("Loaded map area: {} ({})", area.get_name(), area.get_id()));
            }
        }


        if let Err(e) = self
            .ui_tx
            .send(TaggedSessionEvent {
                session_id: self.session_id,
                event: SessionEvent::RuntimeReady(self.session_runtime_tx.clone()),
            })
            .await
        {
            error!("Failed to send runtime ready event: {:?}", e);
        }


        loop {
            // Phase 1: Always poll script engine once per iteration (non-blocking)
            

            std::future::poll_fn(|cx| {
                Poll::Ready(match self.script_engine.poll_event_loop(cx) {
                    Poll::Ready(result) => {
                        if let Err(err) = result {
                            warn!("Error in script engine event loop: {err:?}");
                             self.echo_warn_str_sync(format!("{err:?}").as_str());
                        }
                        // Event loop completed some work, continue to action processing
                    }
                    Poll::Pending => {
                        // Event loop is waiting for async operations, continue to action processing
                    }
                })
            })
            .await;

            // Phase 2: Get next action to process
            let action = if let Some(current_frame) = action_stack.last_mut() {
                // We have spawned actions in the stack
                // Check for OOB interrupts first (non-blocking)
                if let Ok(oob_action) = self.session_runtime_oob_rx.try_recv() {
                    trace!("Handling OOB interrupt: {oob_action:?}");
                    Some(oob_action)
                } else if let Some(spawned_action) = current_frame.pop_front() {
                    // Process next spawned action
                    trace!("Handling spawned action: {spawned_action:?}");
                    Some(spawned_action)
                } else {
                    // Current frame is empty, pop it and continue
                    action_stack.pop();
                    trace!(
                        "Completed action frame, stack depth: {}",
                        action_stack.len()
                    );
                    continue;
                }
            } else {
                // No spawned actions, wait for external input
                select! {
                    biased;
                    _ = script_engine_tick_interval.tick() => {
                        // Wake up periodically to check script engine
                        continue;
                    }
                    Some(oob_action) = self.session_runtime_oob_rx.recv() => {
                        trace!("Handling OOB action: {oob_action:?}");
                        Some(oob_action)
                    }
                    Some(external_action) = self.session_runtime_rx.recv() => {
                        trace!("Handling external action: {external_action:?}");
                        Some(external_action)
                    }
                }
            };

            // Phase 3: Process the action if we have one
            if let Some(action) = action {
                match self.handle_action(action).await {
                    Ok(ActionResult::None) => {}
                    Ok(ActionResult::Echo(line)) => {
                        if let Ok(Some(fut)) = self.echo_str(line.as_str()) {
                            if fut.await.is_err() {
                                warn!("Failed to echo line");
                                break;
                            }
                        } else {
                            warn!("Failed to echo line");
                            break;
                        }
                    }
                    Ok(ActionResult::CloseSession) => {
                        info!(
                            "Session [{}, {} - {}] Closing",
                            self.session_id, self.server_name, self.profile_name
                        );
                        break;
                    }
                    Ok(ActionResult::Reload) => {
                        return RunAction::Reload;
                    }
                    Ok(ActionResult::Run(spawned_actions)) => {
                        // For OOB actions that spawn more actions, add to current frame instead of creating new frame
                        if let Some(current_frame) = action_stack.last_mut() {
                            // Add spawned actions to the front of current frame for immediate processing
                            for spawned_action in spawned_actions.into_iter().rev() {
                                current_frame.push_front(spawned_action);
                            }
                            trace!("Added spawned actions to current frame (OOB handling)");
                        } else {
                            // No current frame - create new frame
                            if action_stack.len() >= MAX_STACK_DEPTH {
                                warn!("Maximum action stack depth exceeded: {}", MAX_STACK_DEPTH);
                                if let Ok(Some(fut)) =
                                    self.echo_str("Error: Maximum execution depth exceeded")
                                {
                                    let _ = fut.await;
                                }
                            } else {
                                action_stack.push(VecDeque::from(spawned_actions));
                                trace!(
                                    "Pushed new action frame, stack depth: {}",
                                    action_stack.len()
                                );
                            }
                        }
                    }
                    Err(err) => {
                        warn!("Error in runtime: {err:?}");
                        if let Ok(Some(fut)) =
                            self.echo_str(format!("Error in runtime: {err:?}").as_str())
                        {
                            if fut.await.is_err() {
                                warn!("Failed to echo line");
                                break;
                            }
                        } else {
                            warn!("Failed to echo line");
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
