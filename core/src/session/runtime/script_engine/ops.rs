use crate::session::runtime::RuntimeAction;
use crate::session::runtime::script_engine::FunctionId;
use crate::session::{SessionId, registry};
use crate::models::aliases::AliasDefinition;
use crate::models::triggers::TriggerDefinition;
use crate::models::ScriptLang;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::{anyhow, bail, Error as AnyError};
use deno_core::op2;
use deno_core::v8;
use deno_core::OpState;
use tokio::sync::mpsc::UnboundedSender;


deno_core::extension!(
    smudgy_ops,
    ops = [
      op_smudgy_get_current_session,
      op_smudgy_get_session_character,
      op_smudgy_get_sessions,
      op_smudgy_session_echo,
      op_smudgy_session_send,
      op_smudgy_session_send_raw,
      op_smudgy_create_simple_alias,
      op_smudgy_create_simple_trigger,
      op_smudgy_create_javascript_function_trigger,
      op_smudgy_create_javascript_function_alias,
      op_smudgy_set_alias_enabled,
      op_smudgy_set_trigger_enabled,
      ],
    options = {
      session_id: SessionId,
      script_functions: Rc<RefCell<Vec<v8::Global<v8::Function>>>>,
      runtime_oob_tx: UnboundedSender<RuntimeAction>,
    },
    state = |state, options| {
      state.put::<SessionId>(options.session_id);
      state.put::<Rc<RefCell<Vec<v8::Global<v8::Function>>>>>(options.script_functions);
      state.put::<UnboundedSender<RuntimeAction>>(options.runtime_oob_tx);
    },
  );


#[op2(fast)]
fn op_smudgy_get_current_session(state: &mut OpState) -> u32 {
    u32::from(*state.borrow::<SessionId>())
}

#[op2]
fn op_smudgy_get_sessions<'s>(
    scope: &mut v8::HandleScope<'s>,
) -> v8::Local<'s, v8::Array> {
    let session_ids = registry::get_all_session_ids();
    
    let sessions: Vec<v8::Local<v8::Value>> = session_ids
        .iter()
        .map(|&session_id| {
            v8::Integer::new_from_unsigned(scope, u32::from(session_id)).into()
        })
        .collect();

    v8::Array::new_with_elements(scope, sessions.as_slice())
}

#[op2]
fn op_smudgy_get_session_character<'s>(
    scope: &mut v8::HandleScope<'s>,
    session_id: u32,
) -> v8::Local<'s, v8::Object> {
    // Convert the session_id to our SessionId type
    let session_id = SessionId::from(session_id);
    
    // Get the runtime for this session
    let runtime = match registry::get_runtime(session_id) {
        Some(runtime) => runtime,
        None => return v8::Object::new(scope), // Return empty object if session not found
    };
    
    // Create the return object
    let ret = v8::Object::new(scope);
    
    let name_k = v8::String::new(scope, "name").unwrap().into();
    let name_v = v8::String::new(scope, &runtime.profile_name)
        .expect("Unable to create v8 string from character name")
        .into();
    
    let subtext_k = v8::String::new(scope, "subtext").unwrap().into();
    let subtext_v = v8::String::new(scope, &runtime.profile_subtext)
        .expect("Unable to create v8 string from character subtext")
        .into();
    
    ret.create_data_property(scope, name_k, name_v);
    ret.create_data_property(scope, subtext_k, subtext_v);
    
    ret
}

#[op2(fast)]
fn op_smudgy_session_echo(
    session_id: u32,
    #[string] line: &str,
) {
    // Convert the session_id to our SessionId type
    let session_id = SessionId::from(session_id);
    
    // Get the runtime for this session
    let runtime = match registry::get_runtime(session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };
    
    // Send the echo action to the runtime
    let _ = runtime.oob_tx.send(crate::session::runtime::RuntimeAction::Echo(
        Arc::new(line.to_string())
    ));
}

#[op2(fast)]
fn op_smudgy_session_send(
    session_id: u32,
    #[string] line: &str,
) {
    // Convert the session_id to our SessionId type
    let session_id = SessionId::from(session_id);
    
    // Get the runtime for this session
    let runtime = match registry::get_runtime(session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };
    
    // Send the send action to the runtime
    let _ = runtime.oob_tx.send(crate::session::runtime::RuntimeAction::Send(
        Arc::new(line.to_string())
    ));
}

#[op2(fast)]
fn op_smudgy_session_send_raw(
    session_id: u32,
    #[string] line: &str,
) {
    // Convert the session_id to our SessionId type
    let session_id = SessionId::from(session_id);
    
    // Get the runtime for this session
    let runtime = match registry::get_runtime(session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };
    
    // Send the send raw action to the runtime
    let _ = runtime.oob_tx.send(crate::session::runtime::RuntimeAction::SendRaw(
        Arc::new(line.to_string())
    ));
}

/// Helper function to convert a V8 array to Vec<String>
fn v8_array_to_vec_str(
    scope: &mut v8::HandleScope,
    arr: &v8::Array,
) -> Result<Vec<String>, AnyError> {
    (0..arr.length())
        .map(|i| {
            arr.get_index(scope, i).map_or_else(
                || bail!("Unable to get element {} from array", i),
                |val| Ok(val.to_rust_string_lossy(scope)),
            )
        })
        .collect()
}

#[op2(fast)]
fn op_smudgy_create_simple_alias(
    scope: &mut v8::HandleScope,
    state: &mut OpState,
    #[string] name: String,
    patterns: &v8::Array,
    #[string] script: String,
) {
    // Get the runtime tx from OpState
    let tx = state.borrow::<UnboundedSender<RuntimeAction>>();
    
    // Convert patterns array to Vec<String>
    let patterns_vec = match v8_array_to_vec_str(scope, patterns) {
        Ok(patterns) => patterns,
        Err(_) => return, // Silently fail on error
    };
    
    // Create AliasDefinition
    let alias_def = AliasDefinition {
        pattern: patterns_vec.into_iter().next().unwrap_or_default(),
        script: Some(script),
        package: None,
        enabled: true,
        language: ScriptLang::Plaintext,
    };
    
    // Send the add alias action
    let _ = tx.send(RuntimeAction::AddAlias {
        name: Arc::new(name),
        alias: alias_def,
    });
}

#[allow(clippy::too_many_arguments)]
#[op2(fast)]
fn op_smudgy_create_simple_trigger(
    scope: &mut v8::HandleScope,
    state: &mut OpState,
    #[string] name: String,
    patterns: &v8::Array,
    raw_patterns: &v8::Array,
    anti_patterns: &v8::Array,
    #[string] script: String,
    prompt: bool,
    enabled: bool,
) {
    // Get the runtime tx from OpState
    let tx = state.borrow::<UnboundedSender<RuntimeAction>>();
    
    // Convert all pattern arrays to Vec<String>
    let patterns_vec = match v8_array_to_vec_str(scope, patterns) {
        Ok(patterns) => if patterns.is_empty() { None } else { Some(patterns) },
        Err(_) => return, // Silently fail on error
    };
    
    let raw_patterns_vec = match v8_array_to_vec_str(scope, raw_patterns) {
        Ok(patterns) => if patterns.is_empty() { None } else { Some(patterns) },
        Err(_) => return, // Silently fail on error
    };
    
    let anti_patterns_vec = match v8_array_to_vec_str(scope, anti_patterns) {
        Ok(patterns) => if patterns.is_empty() { None } else { Some(patterns) },
        Err(_) => return, // Silently fail on error
    };
    
    // Create TriggerDefinition
    let trigger_def = TriggerDefinition {
        patterns: patterns_vec,
        raw_patterns: raw_patterns_vec,
        anti_patterns: anti_patterns_vec,
        script: Some(script),
        package: None,
        language: ScriptLang::Plaintext,
        enabled,
        prompt,
    };
    
    // Send the add trigger action
    let _ = tx.send(RuntimeAction::AddTrigger {
        name: Arc::new(name),
        trigger: trigger_def,
    });
}

#[allow(clippy::too_many_arguments)]
#[op2]
fn op_smudgy_create_javascript_function_trigger(
    scope: &mut v8::HandleScope,
    state: &mut OpState,
    #[string] name: String,
    patterns: &v8::Array,
    raw_patterns: &v8::Array,
    anti_patterns: &v8::Array,
    #[global] f: v8::Global<v8::Function>,
    prompt: bool,
    enabled: bool,
) {
    // Store the function and get the function_id
    let function_id = {
        let mut script_functions = state
            .borrow::<Rc<RefCell<Vec<v8::Global<v8::Function>>>>>()
            .borrow_mut();
        let function_id = FunctionId(script_functions.len());
        script_functions.push(f);
        function_id
    };

    // Get the runtime tx from OpState
    let tx = state.borrow::<UnboundedSender<RuntimeAction>>();
    
    // Convert all pattern arrays to Vec<String>
    let patterns_vec = match v8_array_to_vec_str(scope, patterns) {
        Ok(patterns) => patterns,
        Err(_) => return, // Silently fail on error
    };
    let raw_patterns_vec = match v8_array_to_vec_str(scope, raw_patterns) {
        Ok(patterns) => patterns,
        Err(_) => return, // Silently fail on error
    };
    let anti_patterns_vec = match v8_array_to_vec_str(scope, anti_patterns) {
        Ok(patterns) => patterns,
        Err(_) => return, // Silently fail on error
    };
    
    // Send the add javascript function trigger action
    let _ = tx.send(RuntimeAction::AddJavascriptFunctionTrigger {
        name: Arc::new(name),
        patterns: Arc::new(patterns_vec),
        raw_patterns: Arc::new(raw_patterns_vec),
        anti_patterns: Arc::new(anti_patterns_vec),
        function_id,
        prompt,
        enabled,
    });
}

#[allow(clippy::inline_always)]
#[op2]
fn op_smudgy_create_javascript_function_alias(
    scope: &mut v8::HandleScope,
    state: &mut OpState,
    #[string] name: String,
    patterns: &v8::Array,
    #[global] f: v8::Global<v8::Function>,
) {
    // Store the function and get the function_id
    let function_id = {
        let mut script_functions = state
            .borrow::<Rc<RefCell<Vec<v8::Global<v8::Function>>>>>()
            .borrow_mut();
        let function_id = FunctionId(script_functions.len());
        script_functions.push(f);
        function_id
    };

    // Get the runtime tx from OpState
    let tx = state.borrow::<UnboundedSender<RuntimeAction>>();
    
    // Convert patterns array to Vec<String>
    let patterns_vec = match v8_array_to_vec_str(scope, patterns) {
        Ok(patterns) => patterns,
        Err(_) => return, // Silently fail on error
    };
    
    // Send the add javascript function alias action
    let _ = tx.send(RuntimeAction::AddJavascriptFunctionAlias {
        name: Arc::new(name),
        patterns: Arc::new(patterns_vec),
        function_id,
    });
}

#[allow(clippy::inline_always)]
#[op2(fast)]
fn op_smudgy_set_alias_enabled(
    state: &mut OpState,
    #[string] name: String,
    enabled: bool,
) {
    // Get the runtime tx from OpState
    let tx = state.borrow::<UnboundedSender<RuntimeAction>>();
    
    // Send the enable alias action
    let _ = tx.send(RuntimeAction::EnableAlias(Arc::new(name), enabled));
}

#[allow(clippy::inline_always)]
#[op2(fast)]
fn op_smudgy_set_trigger_enabled(
    state: &mut OpState,
    #[string] name: String,
    enabled: bool,
) {
    // Get the runtime tx from OpState
    let tx = state.borrow::<UnboundedSender<RuntimeAction>>();
    
    // Send the enable trigger action
    let _ = tx.send(RuntimeAction::EnableTrigger(Arc::new(name), enabled));
}

