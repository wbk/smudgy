use std::{
    sync::{Arc, Mutex},
    thread,
};

use deno_core::{
    v8::{self, script_compiler::Source, Global, Handle},
    JsRuntime, PollEventLoopOptions,
};
use slint::ComponentHandle;
use tokio::{
    select,
    sync::{
        mpsc::{UnboundedReceiver, UnboundedSender},
        oneshot,
    },
};

use crate::{
    session::{incoming_line_history::IncomingLineHistory, StyledLine, ViewAction},
    trigger::InterpretedAction,
    MainWindow,
};

#[derive(Clone, Debug)]
pub enum RuntimeAction {
    PassthroughCompleteLine(Arc<StyledLine>),
    PassthroughPartialLine(Arc<StyledLine>),
    EvalTriggerScripts(Arc<StyledLine>, Arc<Vec<InterpretedAction>>),
    EvalAliasScripts(Arc<String>, Arc<Vec<InterpretedAction>>, u8),
    StringLiteralCommand(Arc<String>),
    Echo(Arc<String>),
    RequestRepaint,
    UpdateWriteToSocketTx(Option<UnboundedSender<Arc<String>>>),
    CompileJavascriptAlias(Arc<String>, Arc<oneshot::Sender<usize>>),
}

pub struct ScriptRuntime {
    script_action_tx: UnboundedSender<RuntimeAction>,
}

enum ActionResult {
    RequestRepaint,
    SkipRepaint,
}

impl ScriptRuntime {
    pub fn new(
        view_line_action_tx: UnboundedSender<ViewAction>,
        weak_window: slint::Weak<MainWindow>,
        incoming_line_history: Arc<Mutex<IncomingLineHistory>>,
    ) -> Self {
        let (script_action_tx, script_action_rx) =
            tokio::sync::mpsc::unbounded_channel::<RuntimeAction>();

        let script_runtime = Self { script_action_tx };

        thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            runtime.block_on(ScriptRuntime::run_event_loop(
                script_action_rx,
                view_line_action_tx,
                weak_window,
                incoming_line_history,
            ))
        });

        script_runtime
    }

    pub fn tx(&self) -> UnboundedSender<RuntimeAction> {
        self.script_action_tx.clone()
    }

    #[inline(always)]
    fn send_line_as_command_input(
        line: &str,
        view_line_action_tx: &UnboundedSender<ViewAction>,
        write_to_socket_tx: &Option<UnboundedSender<Arc<String>>>,
    ) {
        let styled_line = Arc::new(StyledLine::from_output_str(line));

        // Copy the line into a string with \r\n appended
        let line_str = styled_line.as_str();
        let mut socket_str = String::with_capacity(line_str.len() + 2);
        socket_str.push_str(line_str);
        socket_str.push_str("\r\n");
        let arc_socket_str = Arc::new(socket_str);

        if let Some(ref tx) = write_to_socket_tx {
            tx.send(arc_socket_str).unwrap();
        }

        view_line_action_tx
            .send(ViewAction::AppendCompleteLine(styled_line))
            .unwrap();
    }

    #[inline(always)]
    fn echo_line(line: &str, view_line_action_tx: &UnboundedSender<ViewAction>) {
        let styled_line = Arc::new(StyledLine::from_echo_str(line));
        view_line_action_tx
            .send(ViewAction::AppendCompleteLine(styled_line))
            .unwrap()
    }

    fn compile_javascript(scope: &mut v8::HandleScope, source: &str) -> v8::Global<v8::Script> {
        let v8_script_source =
            v8::String::new_from_utf8(scope, source.as_bytes(), v8::NewStringType::Normal).unwrap();

        let unbound_script = v8::script_compiler::compile_unbound_script(
            scope,
            Source::new(v8_script_source, None),
            v8::script_compiler::CompileOptions::NoCompileOptions,
            v8::script_compiler::NoCacheReason::BecauseV8Extension,
        )
        .unwrap();

        let bound_script = unbound_script.open(scope).bind_to_current_context(scope);

        Global::new(scope, bound_script)
    }

    #[inline(always)]
    fn handle_incoming_action(deno: &mut JsRuntime, view_line_action_tx: &UnboundedSender<ViewAction>, weak_window: &slint::Weak<MainWindow>, incoming_line_history_arc: &Arc<Mutex<IncomingLineHistory>>, write_to_socket_tx: &mut Option<UnboundedSender<Arc<String>>>, compiled_scripts: &mut Vec<v8::Global<v8::Script>>, action: RuntimeAction) {
        let result = match action {
            RuntimeAction::RequestRepaint => ActionResult::RequestRepaint,
            RuntimeAction::Echo(line) => {
                ScriptRuntime::echo_line(line.as_str(), &view_line_action_tx);
                ActionResult::RequestRepaint
            }
            RuntimeAction::PassthroughCompleteLine(line) => {
                view_line_action_tx
                    .send(ViewAction::AppendCompleteLine(line.clone()))
                    .unwrap();
                let mut incoming_line_history = incoming_line_history_arc.lock().unwrap();
                incoming_line_history.extend_line(line);
                incoming_line_history.commit_current_line();
                ActionResult::SkipRepaint
            }
            RuntimeAction::PassthroughPartialLine(line) => {
                view_line_action_tx
                    .send(ViewAction::AppendPartialLine(line.clone()))
                    .unwrap();
                let mut incoming_line_history = incoming_line_history_arc.lock().unwrap();
                incoming_line_history.extend_line(line);
                ActionResult::SkipRepaint
            }
            RuntimeAction::EvalTriggerScripts(line, triggers) => {
                view_line_action_tx
                    .send(ViewAction::AppendCompleteLine(line))
                    .unwrap();

                for trigger in triggers.iter() {
                    match trigger {
                        InterpretedAction::Noop => {}
                        InterpretedAction::SendRaw(str) => {
                            for line in str.split('\n') {
                                ScriptRuntime::send_line_as_command_input(
                                    line,
                                    &view_line_action_tx,
                                    &write_to_socket_tx,
                                );
                            }
                        }
                        InterpretedAction::ProcessAlias(str) => {
                            for line in str.split(|ch| ch == ';' || ch == '\n') {
                                ScriptRuntime::send_line_as_command_input(
                                    line,
                                    &view_line_action_tx,
                                    &write_to_socket_tx,
                                );
                            }
                        }
                        InterpretedAction::ProcessJavascriptAlias(_script_id, _matches) => {
                            unimplemented!()
                        }
                    }
                }

                ActionResult::SkipRepaint
            }
            RuntimeAction::EvalAliasScripts(_line, aliases, _depth) => {
                for alias in aliases.iter() {
                    match alias {
                        InterpretedAction::Noop => {}
                        InterpretedAction::SendRaw(str) => {
                            for line in str.split('\n') {
                                ScriptRuntime::send_line_as_command_input(
                                    line,
                                    &view_line_action_tx,
                                    &write_to_socket_tx,
                                );
                            }
                        }
                        InterpretedAction::ProcessAlias(str) => {
                            for line in str.split(|ch| ch == ';' || ch == '\n') {
                                ScriptRuntime::send_line_as_command_input(
                                    line,
                                    &view_line_action_tx,
                                    &write_to_socket_tx,
                                );
                            }
                        }
                        InterpretedAction::ProcessJavascriptAlias(script_id, matches) => {
                            if let Some(script) = compiled_scripts.get(*script_id) {
                                let local_scope = &mut deno.handle_scope();
                                let try_catch = &mut v8::TryCatch::new(local_scope);

                                let matches_object = v8::Object::new(try_catch);
                                for (k, v) in matches.iter() {
                                    let arg_k = v8::String::new(try_catch, k).unwrap();
                                    let arg_v = v8::String::new(try_catch, v).unwrap();
                                    matches_object.create_data_property(
                                        try_catch,
                                        arg_k.into(),
                                        arg_v.into(),
                                    );
                                }

                                let matches_name =
                                    v8::String::new(try_catch, "matches").unwrap();

                                try_catch.get_current_context().global(try_catch).set(
                                    try_catch,
                                    matches_name.into(),
                                    matches_object.into(),
                                );

                                let result = script.open(try_catch).run(try_catch);

                                if try_catch.has_caught() {
                                    let exc = try_catch.exception().unwrap();
                                    let exc = exc.to_string(try_catch).unwrap();
                                    let exc = exc.to_rust_string_lossy(try_catch);
                                    ScriptRuntime::echo_line(
                                        exc.as_str(),
                                        &view_line_action_tx,
                                    );
                                } else {
                                    if let Some(value) = result {
                                        if value.boolean_value(try_catch) {
                                            let str = value
                                                .open(try_catch)
                                                .to_rust_string_lossy(try_catch);

                                            for line in str.split(|ch| ch == ';' || ch == '\n')
                                            {
                                                ScriptRuntime::send_line_as_command_input(
                                                    line,
                                                    &view_line_action_tx,
                                                    &write_to_socket_tx,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                ActionResult::RequestRepaint
            }
            RuntimeAction::StringLiteralCommand(str) => {
                for line in str.split(|ch| ch == ';' || ch == '\n') {
                    ScriptRuntime::send_line_as_command_input(
                        line,
                        &view_line_action_tx,
                        &write_to_socket_tx,
                    );
                }
                ActionResult::RequestRepaint
            }
            RuntimeAction::UpdateWriteToSocketTx(option_tx) => {
                *write_to_socket_tx = option_tx;
                ActionResult::SkipRepaint
            }
            RuntimeAction::CompileJavascriptAlias(source, reply_arc) => {
                let f = ScriptRuntime::compile_javascript(
                    &mut deno.handle_scope(),
                    source.as_str(),
                );

                let module_id = compiled_scripts.len();
                compiled_scripts.push(f);

                if let Some(reply) = Arc::into_inner(reply_arc) {
                    reply.send(module_id).unwrap();
                }

                ActionResult::SkipRepaint
            }
        };

        match result {
            ActionResult::RequestRepaint => {
                weak_window
                    .upgrade_in_event_loop(move |handle| handle.window().request_redraw())
                    .unwrap();
            }
            ActionResult::SkipRepaint => {}
        }
    }

    async fn run_event_loop(
        mut scripted_action_rx: UnboundedReceiver<RuntimeAction>,
        view_line_action_tx: UnboundedSender<ViewAction>,
        weak_window: slint::Weak<MainWindow>,
        incoming_line_history_arc: Arc<Mutex<IncomingLineHistory>>,
    ) {
        let mut write_to_socket_tx: Option<UnboundedSender<Arc<String>>>= None;

        let mut deno = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
            ..Default::default()
        });

        let mut compiled_scripts: Vec<v8::Global<v8::Script>> = Vec::new();

        let mut deno_event_loop_interval =
            tokio::time::interval(tokio::time::Duration::from_micros(100));
        deno_event_loop_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            deno.run_event_loop(PollEventLoopOptions::default())
                .await
                .unwrap();

            select! {
                _ = deno_event_loop_interval.tick() => {
                    // this serves to trigger a cancel on the pending receive below when it's time
                    // for the event loop above to tick
                }
                Some(action) = scripted_action_rx.recv() => ScriptRuntime::handle_incoming_action(
                    &mut deno,
                    &view_line_action_tx,
                    &weak_window,
                    &incoming_line_history_arc,
                    &mut write_to_socket_tx,
                    &mut compiled_scripts,
                    action,
                )
            }
        }
    }
}
