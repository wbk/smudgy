use std::{
    cell::RefCell, collections::{HashMap, HashSet}, fs::{self, File}, io::BufReader, path::{Path, PathBuf}, rc::Rc, str::FromStr, sync::{self, Arc, Mutex, RwLock}, task::Poll, thread, time::Instant
};

use anyhow::{bail, Context, Result};

use deno_core::{
    ascii_str_include, url::Url, v8::{self, script_compiler::Source, CreateParams, Global, Handle}, JsRuntime, PollEventLoopOptions
};

use rustyscript::{extensions::deno_fs::FileSystem, ExtensionOptions, Runtime as RustyRuntime, RustyResolver};

use serde::Deserialize;

use tokio::{
    select,
    sync::mpsc::{UnboundedReceiver, UnboundedSender},
};

use crate::{
    get_smudgy_home, session::{incoming_line_history::IncomingLineHistory}
};

mod trigger;
use trigger::{Manager, PushTriggerParams};

use super::styled_line::StyledLine;

pub enum UiAction {
    AppendCompleteLine(Arc<StyledLine>),
    AppendPartialLine(Arc<StyledLine>),
}

#[derive(Clone, Debug)]
pub enum RuntimeAction {
    HandleIncomingLine(Arc<StyledLine>),
    HandleIncomingPartialLine(Arc<StyledLine>),
    PassthroughCompleteLine(Arc<StyledLine>),
    PassthroughPartialLine(Arc<StyledLine>),
    Send(Arc<String>),
    SendRaw(Arc<String>),
    Echo(Arc<String>),
    RequestRepaint,
    UpdateWriteToSocketTx(Option<UnboundedSender<Arc<String>>>),
    Reload,
}

pub struct Runtime {
    tx: UnboundedSender<RuntimeAction>,
}

enum ActionResult {
    RequestRepaint,
    SkipRepaint,
    Echo(String),
    CloseSession,
}

macro_rules! trigger_patterns {
    ($name:ident, $singular:literal, $plural:literal) => {
        #[derive(Deserialize)]
        enum $name {
            #[serde(rename = $singular)]
            Single(String),

            #[serde(rename = $plural)]
            Multiple(Vec<String>),
        }

        impl From<$name> for Vec<String> {
            fn from(patterns: $name) -> Self {
                match patterns {
                    $name::Single(pattern) => vec![pattern],
                    $name::Multiple(patterns) => patterns,
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                $name::Multiple(vec![])
            }
        }
    };
}

trigger_patterns!(TriggerPatterns, "pattern", "patterns");
trigger_patterns!(TriggerRawPatterns, "rawPattern", "rawPatterns");
trigger_patterns!(TriggerAntiPatterns, "antiPattern", "antiPatterns");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AliasManifest {
    #[serde(flatten)]
    patterns: TriggerPatterns,
    script: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TriggerManifest {
    #[serde(flatten)]
    patterns: Option<TriggerPatterns>,
    #[serde(flatten)]
    raw_patterns: Option<TriggerRawPatterns>,
    #[serde(flatten)]
    anti_patterns: Option<TriggerAntiPatterns>,
    script: Option<String>,
    prompt: Option<bool>,
    enabled: Option<bool>,
}

impl Runtime {
    pub fn new(
        id: Arc<Mutex<i32>>,
        view_line_action_tx: UnboundedSender<UiAction>,
        server_name: Arc<String>,
        incoming_line_history: Arc<Mutex<IncomingLineHistory>>,
    ) -> Arc<Self> {
        let (session_runtime_tx, session_runtime_rx) =
            tokio::sync::mpsc::unbounded_channel::<RuntimeAction>();

        let (session_runtime_oob_tx, session_runtime_oob_rx) =
            tokio::sync::mpsc::unbounded_channel::<RuntimeAction>();

        let local_session_runtime_tx = session_runtime_tx.clone();
        let local_server_name = server_name.clone();
        thread::spawn(move || {
           
            let script_functions: Rc<RefCell<Vec<v8::Global<v8::Function>>>> =
                Rc::new(RefCell::new(Vec::new()));
            let mut compiled_scripts: Vec<v8::Global<v8::Script>> = Vec::new();

            let (rustyscript, trigger_manager) = Runtime::load_scripts(
                &id,
                &script_functions,
                &mut compiled_scripts,
                &session_runtime_oob_tx,
                server_name
            );

            let runtime = tokio::runtime::Builder::new_current_thread()
            .build().expect("Failed to create tokio runtime");

            runtime.block_on(Runtime::run_event_loop(
                id,
                local_server_name,
                rustyscript,
                trigger_manager,
                session_runtime_rx,
                session_runtime_oob_rx,
                local_session_runtime_tx,
                session_runtime_oob_tx,
                view_line_action_tx,
                incoming_line_history,
                compiled_scripts,
                script_functions,
            ));
        });

        Arc::new(Self { tx: session_runtime_tx })
    }

    pub fn load_scripts(
        id: &Arc<Mutex<i32>>,
        script_functions: &Rc<RefCell<Vec<v8::Global<v8::Function>>>>,
        compiled_scripts: &mut Vec<v8::Global<v8::Script>>,
        session_runtime_oob_tx: &UnboundedSender<RuntimeAction>,
        server_name: Arc<String>,
    ) -> (RustyRuntime, Manager) {
        script_functions.borrow_mut().clear();
        compiled_scripts.clear();

        
    let smudgy_dir = get_smudgy_home().unwrap();
    let server_path = smudgy_dir.join(server_name.as_str());

        let mut rustyscript = rustyscript::Runtime::with_tokio_runtime_handle(rustyscript::RuntimeOptions {
            // extensions: vec![
            //     ops::smudgy_session_ops::init_ops(
            //         id.clone(),
            //         weak_sessions.clone(),
            //         script_functions.clone(),
            //         session_runtime_oob_tx.clone(),
            //     )
            // ],
            extension_options: ExtensionOptions {                 
                webstorage_origin_storage_dir: Some(server_path.join("localstorage")),
                node_resolver: Arc::new(RustyResolver::new(Some(server_path.to_path_buf()), Arc::new(rustyscript::extensions::deno_fs::RealFs))),
                
                ..Default::default()
            },            
            schema_whlist: HashSet::from(["smudgy".to_string()]),
            ..Default::default()
        }, tokio::runtime::Handle::current()).expect("Failed to create JS runtime");

        rustyscript.deno_runtime().execute_script("<smudgy>", ascii_str_include!("runtime/js/smudgy.js"))
            .unwrap();

        let trigger_manager = Manager::new(session_runtime_oob_tx.clone());
        (rustyscript, trigger_manager)
    }

    #[inline(always)]
    pub fn send(&self, action: RuntimeAction) -> Result<()> {
        self.tx
            .send(action)
            .context("Failed to send action to session runtime")
    }

    pub fn tx(&self) -> UnboundedSender<RuntimeAction> {
        self.tx.clone()
    }

    #[inline(always)]
    fn send_line_as_command_input(
        line: &str,
        view_line_action_tx: &UnboundedSender<UiAction>,
        write_to_socket_tx: &Option<UnboundedSender<Arc<String>>>,
    ) {
        let styled_line = Arc::new(StyledLine::from_output_str(line));

        // Copy the line into a string with \r\n appended
        let mut socket_str = String::with_capacity(styled_line.len() + 2);
        socket_str.push_str(&styled_line);
        socket_str.push_str("\r\n");
        let arc_socket_str = Arc::new(socket_str);

        if let Some(tx) = write_to_socket_tx.as_ref() {
            tx.send(arc_socket_str).unwrap();
        }

        view_line_action_tx
            .send(UiAction::AppendCompleteLine(styled_line))
            .unwrap();
    }

    #[inline(always)]
    fn echo_line(
        line: &str,
        view_line_action_tx: &UnboundedSender<UiAction>,
    ) -> Result<(), anyhow::Error> {
        let styled_line = Arc::new(StyledLine::from_echo_str(line));
        view_line_action_tx
            .send(UiAction::AppendCompleteLine(styled_line))
            .context("Failed to send echo line to view")
    }

    #[inline(always)]
    fn warn_line(
        line: &str,
        view_line_action_tx: &UnboundedSender<UiAction>,
    ) -> Result<(), anyhow::Error> {
        let styled_line = Arc::new(StyledLine::from_warn_str(line));
        view_line_action_tx
            .send(UiAction::AppendCompleteLine(styled_line))
            .context("Failed to send echo line to view")
    }

    fn compile_javascript(scope: &mut v8::HandleScope, source: &str) -> v8::Global<v8::Script> {
        let v8_script_source =
            v8::String::new_from_utf8(scope, source.as_bytes(), v8::NewStringType::Normal).unwrap();

        let unbound_script = v8::script_compiler::compile_unbound_script(
            scope,
            &mut Source::new(v8_script_source, None),
            v8::script_compiler::CompileOptions::NoCompileOptions,
            v8::script_compiler::NoCacheReason::BecauseV8Extension,
        )
        .unwrap();

        let bound_script = unbound_script.open(scope).bind_to_current_context(scope);

        Global::new(scope, bound_script)
    }

    #[inline(always)]
    async fn handle_incoming_action(
        id: &Arc<Mutex<i32>>,
        server_name: Arc<String>,
        session_runtime_tx: &UnboundedSender<RuntimeAction>,
        session_runtime_oob_tx: &UnboundedSender<RuntimeAction>,
        rustyscript: &mut Option<RustyRuntime>,
        trigger_manager: &mut Manager,
        view_line_action_tx: &UnboundedSender<UiAction>,
        incoming_line_history_arc: &Arc<Mutex<IncomingLineHistory>>,
        write_to_socket_tx: &mut Option<UnboundedSender<Arc<String>>>,
        compiled_scripts: &mut Vec<v8::Global<v8::Script>>,
        script_functions: &mut Rc<RefCell<Vec<v8::Global<v8::Function>>>>,
        action: RuntimeAction,
    ) -> Result<ActionResult, anyhow::Error> {
        match action {
            RuntimeAction::HandleIncomingLine(line) => {
                match trigger_manager.process_incoming_line(line) {
                    Ok(_) => Ok(ActionResult::SkipRepaint),
                    Err(err) => Ok(ActionResult::Echo(format!(
                        "Error processing line {:?}",
                        err
                    ))),
                }
            }
            RuntimeAction::HandleIncomingPartialLine(line) => {
                match trigger_manager.process_partial_line(line) {
                    Ok(_) => Ok(ActionResult::SkipRepaint),
                    Err(err) => Ok(ActionResult::Echo(format!(
                        "Error processing partial line {:?}",
                        err
                    ))),
                }
            }
            RuntimeAction::RequestRepaint => Ok(ActionResult::RequestRepaint),
            RuntimeAction::Echo(line) => {
                Runtime::echo_line(line.as_str(), &view_line_action_tx)?;
                Ok(ActionResult::RequestRepaint)
            }
            RuntimeAction::PassthroughCompleteLine(line) => {
                view_line_action_tx
                    .send(UiAction::AppendCompleteLine(line.clone()))
                    .unwrap();
                let mut incoming_line_history = incoming_line_history_arc.lock().unwrap();
                incoming_line_history.extend_line(line);
                incoming_line_history.commit_current_line();
                Ok(ActionResult::SkipRepaint)
            }
            RuntimeAction::PassthroughPartialLine(line) => {
                view_line_action_tx
                    .send(UiAction::AppendPartialLine(line.clone()))
                    .unwrap();
                let mut incoming_line_history = incoming_line_history_arc.lock().unwrap();
                incoming_line_history.extend_line(line);
                Ok(ActionResult::SkipRepaint)
            }
            // RuntimeAction::EvalJavascriptTrigger(_line, script_id, matches) => {
            //     let deno = rustyscript.as_mut().unwrap().deno_runtime();
            //     if let Some(script) = compiled_scripts.get(script_id) {
            //         Ok(Runtime::run_script(
            //             trigger_manager,
            //             &mut deno.handle_scope(),
            //             script,
            //             matches,
            //             0,
            //         )
            //         .unwrap_or_else(|err| {
            //             ActionResult::Echo(format!("Error in Javascript Trigger: {:?}", err))
            //         }))
            //     } else {
            //         bail!("Failed to load alias by script id {script_id}");
            //     }
            // }
            // RuntimeAction::EvalJavascriptAlias(_line, script_id, matches, depth) => {
            //     let deno = rustyscript.as_mut().unwrap().deno_runtime();
            //     if let Some(script) = compiled_scripts.get(script_id) {
            //         Ok(Runtime::run_script(
            //             trigger_manager,
            //             &mut deno.handle_scope(),
            //             script,
            //             matches,
            //             depth,
            //         )
            //         .unwrap_or_else(|err| {
            //             ActionResult::Echo(format!("Error in Javascript Alias: {:?}", err))
            //         }))
            //     } else {
            //         bail!("Failed to load alias by script id {script_id}");
            //     }
            // }
            // RuntimeAction::EvalJavascriptFunctionAlias(_line, function_id, matches, depth) => {
            //     let deno = rustyscript.as_mut().unwrap().deno_runtime();
            //     if let Some(f) = script_functions.borrow().get(function_id) {
            //         Ok(Runtime::call_javascript_function(
            //             trigger_manager,
            //             &mut deno.handle_scope(),
            //             f,
            //             matches,
            //             depth,
            //         )
            //         .unwrap_or_else(|err| {
            //             ActionResult::Echo(format!("Error in Javascript Alias: {:?}", err))
            //         }))
            //     } else {
            //         bail!("Failed to load function by id {function_id}");
            //     }
            // }

            RuntimeAction::Send(line) => {
                match trigger_manager.process_outgoing_line(line.as_str()) {
                    Ok(_) => Ok(ActionResult::SkipRepaint),
                    Err(err) => Ok(ActionResult::Echo(format!(
                        "Error processing command {:?}",
                        err
                    ))),
                }
            }
            RuntimeAction::SendRaw(str) => {
                for line in str.split(|ch| ch == '\n') {
                    Runtime::send_line_as_command_input(
                        line,
                        &view_line_action_tx,
                        &write_to_socket_tx,
                    );
                }
                Ok(ActionResult::RequestRepaint)
            }
            RuntimeAction::UpdateWriteToSocketTx(option_tx) => {
                *write_to_socket_tx = option_tx;
                Ok(ActionResult::SkipRepaint)
            }
            // RuntimeAction::LoadJavascriptModules(paths) => {
            //     let deno = rustyscript.as_mut().unwrap().deno_runtime();

            //     let code = paths.iter().map(|path| format!("import '{path}';")).collect::<Vec<String>>().join("\n");

            //     match deno
            //         .load_main_es_module_from_code(&Url::from_str("smudgy://main").unwrap(), code)
            //         .await
            //     {
            //         Ok(module_id) => {
            //             let evaluation_result = {
            //                 let mut receiver = deno.mod_evaluate(module_id);

            //                 tokio::select! {
            //                     biased;

            //                     maybe_result = &mut receiver => {
            //                         maybe_result
            //                     }

            //                     event_loop_result = deno.run_event_loop(PollEventLoopOptions::default()) => {
            //                         event_loop_result?;
            //                         receiver.await
            //                     }
            //                 }
            //             };

            //             if let Err(e) = evaluation_result {
            //             Runtime::warn_line(
            //                 format!("Failed to evaluate modules: {:?}", e).as_str(),
            //                 view_line_action_tx,
            //                 )?;
            //             }
            //         }
            //         Err(e) => {
            //             Runtime::warn_line(
            //                 format!("Failed to load modules: {:?}", e).as_str(),
            //                 view_line_action_tx,
            //             )?;
            //         }
            //     }
            //     Ok(ActionResult::SkipRepaint)
            // }
            // RuntimeAction::AddJavascriptAlias(name, pattern, source) => {
            //     let deno = rustyscript.as_mut().unwrap().deno_runtime();
            //     let f =
            //         Runtime::compile_javascript(&mut deno.handle_scope(), source.as_str());

            //     let module_id = compiled_scripts.len();
            //     compiled_scripts.push(f);

            //     match trigger_manager.push_javascript_alias(&name, &pattern, module_id) {
            //         Ok(_) => Ok(ActionResult::SkipRepaint),
            //         Err(err) => Ok(ActionResult::Echo(format!(
            //             "Error adding javascript alias: {:?}",
            //             err
            //         ))),
            //     }
            // }
            // RuntimeAction::AddJavascriptFunctionAlias(name, pattern, f) => {
            //     match trigger_manager.push_javascript_function_alias(name, pattern, f) {
            //         Ok(_) => Ok(ActionResult::SkipRepaint),
            //         Err(err) => Ok(ActionResult::Echo(format!(
            //             "Error adding javascript function alias: {:?}",
            //             err
            //         ))),
            //     }
            // }
            // RuntimeAction::AddSimpleAlias(name, pattern, source) => {
            //     match trigger_manager.push_simple_alias(name, pattern, source) {
            //         Ok(_) => Ok(ActionResult::SkipRepaint),
            //         Err(err) => Ok(ActionResult::Echo(format!("Error adding alias: {:?}", err))),
            //     }
            // }
            // RuntimeAction::AddJavascriptTrigger(
            //     name,
            //     patterns,
            //     raw_patterns,
            //     anti_patterns,
            //     source,
            //     prompt,
            //     enabled,
            // ) => {
            //     let deno = rustyscript.as_mut().unwrap().deno_runtime();
            //     let f =
            //         Runtime::compile_javascript(&mut deno.handle_scope(), source.as_str());

            //     let module_id = compiled_scripts.len();
            //     compiled_scripts.push(f);

            //     match trigger_manager.push_trigger(
            //         PushTriggerParams {
            //             name: &name,
            //             patterns: &patterns,
            //             raw_patterns: &raw_patterns,
            //             anti_patterns: &anti_patterns,
            //             action: trigger::Action::EvalJavascript(module_id),
            //             prompt,
            //             enabled
            //         }
            //     ) {
            //         Ok(_) => Ok(ActionResult::SkipRepaint),
            //         Err(err) => Ok(ActionResult::Echo(format!(
            //             "Error adding javascript trigger: {:?}",
            //             err
            //         ))),
            //     }
            // }
            // RuntimeAction::AddJavascriptFunctionTrigger(
            //     name,
            //     patterns,
            //     raw_patterns,
            //     anti_patterns,
            //     f,
            //     prompt,
            //     enabled,
            // ) => {
            //     match trigger_manager.push_trigger(
            //         PushTriggerParams {
            //             name: &name,
            //             patterns: &patterns,
            //             raw_patterns: &raw_patterns,
            //             anti_patterns: &anti_patterns,
            //             action: trigger::Action::CallJavascriptFunction(f),
            //             prompt,
            //             enabled
            //         }
            //     ) {
            //         Ok(_) => Ok(ActionResult::SkipRepaint),
            //         Err(err) => Ok(ActionResult::Echo(format!(
            //             "Error adding javascript function trigger: {:?}",
            //             err
            //         ))),
            //     }
            // }
            // RuntimeAction::AddSimpleTrigger(
            //     name,
            //     patterns,
            //     raw_patterns,
            //     anti_patterns,
            //     source,
            //     prompt,
            //     enabled,
            // ) => {
            //     match trigger_manager.push_trigger(
            //         PushTriggerParams {
            //             name: &name,
            //             patterns: &patterns,
            //             raw_patterns: &raw_patterns,
            //             anti_patterns: &anti_patterns,
            //             action: trigger::Action::ProcessScript(source),
            //             prompt,
            //             enabled
            //         }
            //     ) {
            //         Ok(_) => Ok(ActionResult::SkipRepaint),
            //         Err(err) => Ok(ActionResult::Echo(format!(
            //             "Error adding simple trigger: {:?}",
            //             err
            //         ))),
            //     }
            // }
            RuntimeAction::Reload => {
                let tokio_runtime = rustyscript.take().unwrap().tokio_runtime();
                let (new_rustyscript, new_trigger_manager) = Runtime::load_scripts(
                    id,
                    script_functions,
                    compiled_scripts,
                    session_runtime_oob_tx,
                    server_name
                );
                rustyscript.replace(new_rustyscript);
                *trigger_manager = new_trigger_manager;
                Runtime::echo_line("Reloading...", view_line_action_tx)?;
                Ok(ActionResult::SkipRepaint)
            }
        }
    }

    #[inline(always)]
    fn run_script(
        trigger_manager: &Manager,
        scope: &mut v8::HandleScope,
        script: &v8::Global<v8::Script>,
        matches: Arc<Vec<(String, String)>>,
        depth: u32,
    ) -> Result<ActionResult> {
        let started = Instant::now();
        let result = {
            let try_catch = &mut v8::TryCatch::new(scope);

            let matches_object = v8::Object::new(try_catch);
            for (k, v) in matches.iter() {
                let arg_k = v8::String::new(try_catch, k).unwrap();
                let arg_v = v8::String::new(try_catch, v).unwrap();
                matches_object.create_data_property(try_catch, arg_k.into(), arg_v.into());
            }

            let matches_name = v8::String::new(try_catch, "matches").unwrap();

            try_catch.get_current_context().global(try_catch).set(
                try_catch,
                matches_name.into(),
                matches_object.into(),
            );

            let result = script.open(try_catch).run(try_catch);

            if try_catch.has_caught() {
                let ex = try_catch.exception().unwrap();
                let exc = ex.to_string(try_catch).unwrap();
                let exc = exc.to_rust_string_lossy(try_catch);
                Ok(ActionResult::Echo(exc))
            } else {
                if let Some(value) = result {
                    if value.boolean_value(try_catch) {
                        let output = value.open(try_catch).to_rust_string_lossy(try_catch);
                        trigger_manager.process_nested_outgoing_line(output.as_str(), depth + 1)?;
                        Ok(ActionResult::SkipRepaint)
                    } else {
                        Ok(ActionResult::SkipRepaint)
                    }
                } else {
                    Ok(ActionResult::SkipRepaint)
                }
            }
        };

        let elapsed = started.elapsed();
        debug!(
            "Script execution on {} took {:?}",
            matches
                .get(0)
                .unwrap_or(&("".to_string(), "unknown".to_string()))
                .1,
            elapsed
        );
        result
    }

    #[inline(always)]
    fn call_javascript_function(
        trigger_manager: &Manager,
        scope: &mut v8::HandleScope,
        f: &v8::Global<v8::Function>,
        matches: Arc<Vec<(String, String)>>,
        depth: u32,
    ) -> Result<ActionResult> {
        let started = Instant::now();
        let result = {
            let try_catch = &mut v8::TryCatch::new(scope);

            let matches_object = v8::Object::new(try_catch);
            for (k, v) in matches.iter() {
                let arg_k = v8::String::new(try_catch, k).unwrap();
                let arg_v = v8::String::new(try_catch, v).unwrap();
                matches_object.create_data_property(try_catch, arg_k.into(), arg_v.into());
            }

            let f = f.open(try_catch);
            let f_this = v8::undefined(try_catch).into();
            let result = f.call(try_catch, f_this, &[matches_object.into()]);

            if try_catch.has_caught() {
                let ex = try_catch.exception().unwrap();
                let exc = ex.to_string(try_catch).unwrap();
                let exc = exc.to_rust_string_lossy(try_catch);
                Ok(ActionResult::Echo(exc))
            } else {
                if let Some(value) = result {
                    if value.boolean_value(try_catch) {
                        let output = value.open(try_catch).to_rust_string_lossy(try_catch);
                        trigger_manager.process_nested_outgoing_line(output.as_str(), depth + 1)?;
                        Ok(ActionResult::SkipRepaint)
                    } else {
                        Ok(ActionResult::SkipRepaint)
                    }
                } else {
                    Ok(ActionResult::SkipRepaint)
                }
            }
        };

        let elapsed = started.elapsed();
        debug!(
            "Script execution on {} took {:?}",
            matches
                .get(0)
                .unwrap_or(&("".to_string(), "unknown".to_string()))
                .1,
            elapsed
        );
        result
    }

    async fn run_event_loop(
        id: Arc<Mutex<i32>>,
        server_name: Arc<String>,
        rustyscript: RustyRuntime,
        mut trigger_manager: Manager,
        mut session_runtime_rx: UnboundedReceiver<RuntimeAction>,
        mut session_runtime_oob_rx: UnboundedReceiver<RuntimeAction>,
        session_runtime_tx: UnboundedSender<RuntimeAction>,
        session_runtime_oob_tx: UnboundedSender<RuntimeAction>,
        view_line_action_tx: UnboundedSender<UiAction>,
        incoming_line_history_arc: Arc<Mutex<IncomingLineHistory>>,
        mut compiled_scripts: Vec<v8::Global<v8::Script>>,
        mut script_functions: Rc<RefCell<Vec<v8::Global<v8::Function>>>>,
    ) {
        let mut write_to_socket_tx: Option<UnboundedSender<Arc<String>>> = None;

        let mut deno_event_loop_interval =
            tokio::time::interval(tokio::time::Duration::from_micros(100));
        deno_event_loop_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let mut rustyscript = Some(rustyscript);

        loop {

            std::future::poll_fn(|cx| {
                Poll::Ready(match rustyscript.as_mut().unwrap().deno_runtime().poll_event_loop(cx, PollEventLoopOptions::default()) {
                    Poll::Ready(t) => t.map(|()| false),
                    Poll::Pending => Ok(true),
                })
            }).await.map_err(|err| {
                warn!("Error in deno event loop: {:?}", err);
                Runtime::warn_line(format!("{:?}", err).as_str(), &view_line_action_tx)
            })
            .ok();

            select! {
                biased;
                _ = deno_event_loop_interval.tick() => {
                    // this serves to trigger a cancel on the pending receive below when it's time
                    // for the event loop above to tick
                }
                // TODO: until I have a better way of handling OOB messages, this block must perfectly mirror the one below it
                Some(action) = session_runtime_oob_rx.recv() => {
                    trace!("Handling OOB action: {:?}", action);
                    match Runtime::handle_incoming_action(
                    &id,
                    server_name.clone(),
                    &session_runtime_tx,
                    &session_runtime_oob_tx,
                    &mut rustyscript,
                    &mut trigger_manager,
                    &view_line_action_tx,
                    &incoming_line_history_arc,
                    &mut write_to_socket_tx,
                    &mut compiled_scripts,
                    &mut script_functions,
                    action,
                    ).await {
                        Ok(ActionResult::RequestRepaint) => {
                            warn!("ActionResult::RequestRepaint called but not implemented");
                        }
                        Ok(ActionResult::SkipRepaint) => {}
                        Ok(ActionResult::Echo(line)) => {
                            match Runtime::echo_line(line.as_str(), &view_line_action_tx) {
                                Ok(_) => {
                                    warn!("ActionResult::Echo should request a repaint but it is not implemented");
                                }
                                Err(_) => {
                                    warn!("Failed to echo line");
                                    break;
                                }
                            }
                        }
                        Ok(ActionResult::CloseSession) => {
                            trace!("Session runtime event loop ending");
                            break;
                        }
                        Err(err) => {
                            warn!("Error in script runtime: {:?}, ending", err);
                            break;
                        }
                    }
                }
                Some(action) = session_runtime_rx.recv() => {
                    trace!("Handling action: {:?}", action);
                    match Runtime::handle_incoming_action(
                        &id,
                        server_name.clone(),
                        &session_runtime_tx,
                        &session_runtime_oob_tx,
                        &mut rustyscript,
                        &mut trigger_manager,
                        &view_line_action_tx,
                        &incoming_line_history_arc,
                        &mut write_to_socket_tx,
                        &mut compiled_scripts,
                        &mut script_functions,
                        action,
                    ).await {
                        Ok(ActionResult::RequestRepaint) => {
                            warn!("ActionResult::RequestRepaint called but not implemented");
                        }
                        Ok(ActionResult::SkipRepaint) => {}
                        Ok(ActionResult::Echo(line)) => {
                            match Runtime::echo_line(line.as_str(), &view_line_action_tx) {
                                Ok(_) => {
                                    warn!("ActionResult::Echo should request a repaint but it is not implemented");
                                }
                                Err(_) => {
                                    warn!("Failed to echo line");
                                    break;
                                }
                            }
                        }
                        Ok(ActionResult::CloseSession) => {
                            trace!("Session runtime event loop ending");
                            break;
                        }
                        Err(err) => {
                            warn!("Error in script runtime: {:?}, ending", err);
                            break;
                        }
                    }
                }
            }
        }
    }
}
