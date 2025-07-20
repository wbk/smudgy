use crate::models::ScriptLang;
use crate::models::aliases::AliasDefinition;
use crate::models::triggers::TriggerDefinition;
use crate::session::connection::vt_processor::AnsiColor;
use crate::session::runtime::script_engine::mapper::JSArea;
use crate::session::runtime::RuntimeAction;
use crate::session::runtime::line_operation::LineOperation;
use crate::session::runtime::script_engine::FunctionId;
use crate::session::styled_line::{Color, Style, StyledLine};
use crate::session::{SessionId, registry};

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Weak};

use anyhow::{Error as AnyError, bail};
use deno_core::OpState;
use deno_core::op2;
use deno_core::v8;
use smudgy_map::{AreaId, Uuid};
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
    op_smudgy_line_insert,
    op_smudgy_line_replace,
    op_smudgy_line_highlight,
    op_smudgy_line_remove,
    op_smudgy_insert,
    op_smudgy_replace,
    op_smudgy_highlight,
    op_smudgy_remove,
    op_smudgy_gag,
    op_smudgy_get_current_line,
    op_smudgy_get_current_line_number,
    op_smudgy_mapper_set_current_location,
    op_smudgy_capture,
    ],
  options = {
    session_id: SessionId,
    script_functions: Rc<RefCell<Vec<v8::Global<v8::Function>>>>,
    runtime_oob_tx: UnboundedSender<RuntimeAction>,
    pending_line_operations: Rc<RefCell<Vec<LineOperation>>>,
    current_line: Rc<RefCell<Weak<StyledLine>>>,
    emitted_line_count: std::rc::Weak<Cell<usize>>,
  },
  state = |state, options| {
    state.put::<SessionId>(options.session_id);
    state.put::<Rc<RefCell<Vec<v8::Global<v8::Function>>>>>(options.script_functions);
    state.put::<UnboundedSender<RuntimeAction>>(options.runtime_oob_tx);
    state.put::<Rc<RefCell<Vec<LineOperation>>>>(options.pending_line_operations);
    state.put::<Rc<RefCell<Weak<StyledLine>>>>(options.current_line);
    state.put::<std::rc::Weak<Cell<usize>>>(options.emitted_line_count);
    state.put::<Capture>(Capture(false));
  },
);

pub struct Capture(pub bool);

#[op2(fast)]
fn op_smudgy_get_current_session(state: &mut OpState) -> u32 {
    u32::from(*state.borrow::<SessionId>())
}

#[op2]
fn op_smudgy_get_sessions<'s>(scope: &mut v8::HandleScope<'s>) -> v8::Local<'s, v8::Array> {
    let session_ids = registry::get_all_session_ids();

    let sessions: Vec<v8::Local<v8::Value>> = session_ids
        .iter()
        .map(|&session_id| v8::Integer::new_from_unsigned(scope, u32::from(session_id)).into())
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
fn op_smudgy_session_echo(session_id: u32, #[string] line: &str) {
    // Convert the session_id to our SessionId type
    let session_id = SessionId::from(session_id);

    // Get the runtime for this session
    let runtime = match registry::get_runtime(session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };

    // Send the echo action to the runtime
    let _ = runtime
        .oob_tx
        .send(crate::session::runtime::RuntimeAction::Echo(Arc::new(
            line.to_string(),
        )));
}

#[op2(fast)]
fn op_smudgy_session_send(session_id: u32, #[string] line: &str) {
    // Convert the session_id to our SessionId type
    let session_id = SessionId::from(session_id);

    // Get the runtime for this session
    let runtime = match registry::get_runtime(session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };

    // Send the send action to the runtime
    let _ = runtime
        .oob_tx
        .send(crate::session::runtime::RuntimeAction::Send(Arc::new(
            line.to_string(),
        )));
}

#[op2(fast)]
fn op_smudgy_session_send_raw(session_id: u32, #[string] line: &str) {
    // Convert the session_id to our SessionId type
    let session_id = SessionId::from(session_id);

    // Get the runtime for this session
    let runtime = match registry::get_runtime(session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };

    // Send the send raw action to the runtime
    let _ = runtime
        .oob_tx
        .send(crate::session::runtime::RuntimeAction::SendRaw(Arc::new(
            line.to_string(),
        )));
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
        Ok(patterns) => {
            if patterns.is_empty() {
                None
            } else {
                Some(patterns)
            }
        }
        Err(_) => return, // Silently fail on error
    };

    let raw_patterns_vec = match v8_array_to_vec_str(scope, raw_patterns) {
        Ok(patterns) => {
            if patterns.is_empty() {
                None
            } else {
                Some(patterns)
            }
        }
        Err(_) => return, // Silently fail on error
    };

    let anti_patterns_vec = match v8_array_to_vec_str(scope, anti_patterns) {
        Ok(patterns) => {
            if patterns.is_empty() {
                None
            } else {
                Some(patterns)
            }
        }
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
fn op_smudgy_set_alias_enabled(state: &mut OpState, #[string] name: String, enabled: bool) {
    // Get the runtime tx from OpState
    let tx = state.borrow::<UnboundedSender<RuntimeAction>>();

    // Send the enable alias action
    let _ = tx.send(RuntimeAction::EnableAlias(Arc::new(name), enabled));
}

#[allow(clippy::inline_always)]
#[op2(fast)]
fn op_smudgy_set_trigger_enabled(state: &mut OpState, #[string] name: String, enabled: bool) {
    // Get the runtime tx from OpState
    let tx = state.borrow::<UnboundedSender<RuntimeAction>>();

    // Send the enable trigger action
    let _ = tx.send(RuntimeAction::EnableTrigger(Arc::new(name), enabled));
}

/// Helper function to parse a color from a JavaScript value
fn parse_color_from_js(
    scope: &mut v8::HandleScope,
    color_val: v8::Local<v8::Value>,
) -> Result<Color, AnyError> {
    if color_val.is_string() {
        let color_str = color_val.to_rust_string_lossy(scope);
        match color_str.as_str() {
            "black" => Ok(Color::Ansi {
                color: AnsiColor::Black,
                bold: true,
            }),
            "red" => Ok(Color::Ansi {
                color: AnsiColor::Red,
                bold: true,
            }),
            "green" => Ok(Color::Ansi {
                color: AnsiColor::Green,
                bold: true,
            }),
            "yellow" => Ok(Color::Ansi {
                color: AnsiColor::Yellow,
                bold: true,
            }),
            "blue" => Ok(Color::Ansi {
                color: AnsiColor::Blue,
                bold: true,
            }),
            "magenta" => Ok(Color::Ansi {
                color: AnsiColor::Magenta,
                bold: true,
            }),
            "cyan" => Ok(Color::Ansi {
                color: AnsiColor::Cyan,
                bold: true,
            }),
            "white" => Ok(Color::Ansi {
                color: AnsiColor::White,
                bold: true,
            }),
            "echo" => Ok(Color::Echo),
            "output" => Ok(Color::Output),
            "warn" => Ok(Color::Warn),
            _ => bail!("Unknown color: {}", color_str),
        }
    } else if color_val.is_object() {
        let obj = color_val.to_object(scope).unwrap();

        // Check if it's an RGB color
        let r_key = v8::String::new(scope, "r").unwrap().into();
        let g_key = v8::String::new(scope, "g").unwrap().into();
        let b_key = v8::String::new(scope, "b").unwrap().into();

        if let (Some(r_val), Some(g_val), Some(b_val)) = (
            obj.get(scope, r_key),
            obj.get(scope, g_key),
            obj.get(scope, b_key),
        ) {
            if r_val.is_number() && g_val.is_number() && b_val.is_number() {
                let r = r_val.number_value(scope).unwrap_or(0.0) as u8;
                let g = g_val.number_value(scope).unwrap_or(0.0) as u8;
                let b = b_val.number_value(scope).unwrap_or(0.0) as u8;
                return Ok(Color::Rgb { r, g, b });
            }
        }

        // Check if it's an ANSI color with bold
        let color_key = v8::String::new(scope, "color").unwrap().into();
        let bold_key = v8::String::new(scope, "bold").unwrap().into();

        if let Some(color_val) = obj.get(scope, color_key) {
            let bold = obj
                .get(scope, bold_key)
                .map_or(false, |v| v.boolean_value(scope));
            let color_str = color_val.to_rust_string_lossy(scope);
            let ansi_color = match color_str.as_str() {
                "black" => AnsiColor::Black,
                "red" => AnsiColor::Red,
                "green" => AnsiColor::Green,
                "yellow" => AnsiColor::Yellow,
                "blue" => AnsiColor::Blue,
                "magenta" => AnsiColor::Magenta,
                "cyan" => AnsiColor::Cyan,
                "white" => AnsiColor::White,
                _ => bail!("Unknown ANSI color: {}", color_str),
            };
            return Ok(Color::Ansi {
                color: ansi_color,
                bold,
            });
        }

        bail!("Invalid color object")
    } else {
        bail!("Color must be a string or object")
    }
}

/// Helper function to create a Style from JavaScript values
fn parse_style_from_js(
    scope: &mut v8::HandleScope,
    fg_val: Option<v8::Local<v8::Value>>,
    bg_val: Option<v8::Local<v8::Value>>,
) -> Result<Style, AnyError> {
    let fg = match fg_val {
        Some(val) => parse_color_from_js(scope, val)?,
        None => Color::Ansi {
            color: AnsiColor::White,
            bold: false,
        },
    };

    let bg = match bg_val {
        Some(val) => parse_color_from_js(scope, val)?,
        None => Color::DefaultBackground,
    };

    Ok(Style { fg, bg })
}

#[op2(fast)]
fn op_smudgy_insert(
    scope: &mut v8::HandleScope,
    state: &mut OpState,
    #[string] text: String,
    begin: u32,
    end: u32,
    fg_color: v8::Local<v8::Value>,
    bg_color: v8::Local<v8::Value>,
) {
    // Parse the style
    let style = match parse_style_from_js(
        scope,
        if fg_color.is_null_or_undefined() {
            None
        } else {
            Some(fg_color)
        },
        if bg_color.is_null_or_undefined() {
            None
        } else {
            Some(bg_color)
        },
    ) {
        Ok(style) => style,
        Err(_) => return, // Silently fail on error
    };

    // Get pending line operations from state
    let pending_ops = state.borrow::<Rc<RefCell<Vec<LineOperation>>>>();
    let mut ops = pending_ops.borrow_mut();

    // Add the insert operation
    ops.push(LineOperation::Insert {
        str: Arc::new(text),
        begin: begin as usize,
        end: end as usize,
        style,
    });
}

#[op2(fast)]
fn op_smudgy_replace(state: &mut OpState, #[string] text: String, begin: u32, end: u32) {
    // Get pending line operations from state
    let pending_ops = state.borrow::<Rc<RefCell<Vec<LineOperation>>>>();
    let mut ops = pending_ops.borrow_mut();

    // Add the replace operation
    ops.push(LineOperation::Replace {
        str: Arc::new(text),
        begin: begin as usize,
        end: end as usize,
    });
}

#[op2(fast)]
fn op_smudgy_highlight(
    scope: &mut v8::HandleScope,
    state: &mut OpState,
    begin: u32,
    end: u32,
    fg_color: v8::Local<v8::Value>,
    bg_color: v8::Local<v8::Value>,
) {
    // Parse the style
    let style = match parse_style_from_js(
        scope,
        if fg_color.is_null_or_undefined() {
            None
        } else {
            Some(fg_color)
        },
        if bg_color.is_null_or_undefined() {
            None
        } else {
            Some(bg_color)
        },
    ) {
        Ok(style) => style,
        Err(_) => return, // Silently fail on error
    };

    // Get pending line operations from state
    let pending_ops = state.borrow::<Rc<RefCell<Vec<LineOperation>>>>();
    let mut ops = pending_ops.borrow_mut();

    // Add the highlight operation
    ops.push(LineOperation::Highlight {
        begin: begin as usize,
        end: end as usize,
        style,
    });
}

#[op2(fast)]
fn op_smudgy_remove(state: &mut OpState, begin: u32, end: u32) {
    // Get pending line operations from state
    let pending_ops = state.borrow::<Rc<RefCell<Vec<LineOperation>>>>();
    let mut ops = pending_ops.borrow_mut();

    // Add the remove operation
    ops.push(LineOperation::Remove {
        begin: begin as usize,
        end: end as usize,
    });
}

#[op2(fast)]
fn op_smudgy_gag(state: &mut OpState) {
    // Get pending line operations from state
    let pending_ops = state.borrow::<Rc<RefCell<Vec<LineOperation>>>>();
    let mut ops = pending_ops.borrow_mut();

    // Add the gag operation
    ops.push(LineOperation::Gag);
}

#[op2]
fn op_smudgy_get_current_line<'s>(
    scope: &mut v8::HandleScope<'s>,
    state: &mut OpState,
) -> v8::Local<'s, v8::String> {
    let current_line = state.borrow::<Rc<RefCell<Weak<StyledLine>>>>();
    if let Some(line) = Weak::upgrade(&current_line.borrow()) {
        return v8::String::new(scope, &line.text).unwrap().into();
    }
    v8::String::new(scope, "").unwrap().into()
}

#[op2]
fn op_smudgy_get_current_line_number<'s>(
    scope: &mut v8::HandleScope<'s>,
    state: &mut OpState,
) -> v8::Local<'s, v8::Number> {
    let emitted_line_count = state.borrow::<std::rc::Weak<Cell<usize>>>();
    if let Some(line) = std::rc::Weak::upgrade(emitted_line_count) {
        return v8::Number::new(scope, (1 + line.get()) as f64).into();
    }
    return v8::Number::new(scope, 0.0).into();
}

#[op2(fast)]
fn op_smudgy_line_insert(
    scope: &mut v8::HandleScope,
    state: &mut OpState,
    line_number: u32,
    #[string] text: String,
    begin: u32,
    end: u32,
    fg_color: v8::Local<v8::Value>,
    bg_color: v8::Local<v8::Value>,
) {
    let style = match parse_style_from_js(
        scope,
        if fg_color.is_null_or_undefined() {
            None
        } else {
            Some(fg_color)
        },
        if bg_color.is_null_or_undefined() {
            None
        } else {
            Some(bg_color)
        },
    ) {
        Ok(style) => style,
        Err(_) => return, // Silently fail on error
    };

    let session_id = state.borrow::<SessionId>();

    // Get the runtime for this session
    let runtime = match registry::get_runtime(*session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };

    let _ = runtime.oob_tx.send(
        crate::session::runtime::RuntimeAction::PerformLineOperation {
            line_number: line_number as usize,
            operation: (LineOperation::Insert {
                str: Arc::new(text),
                begin: begin as usize,
                end: end as usize,
                style,
            }),
        },
    );
}

#[op2(fast)]
fn op_smudgy_line_replace(
    state: &mut OpState,
    line_number: u32,
    #[string] text: String,
    begin: u32,
    end: u32,
) {
    let session_id = state.borrow::<SessionId>();

    // Get the runtime for this session
    let runtime = match registry::get_runtime(*session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };

    let _ = runtime.oob_tx.send(
        crate::session::runtime::RuntimeAction::PerformLineOperation {
            line_number: line_number as usize,
            operation: (LineOperation::Replace {
                str: Arc::new(text),
                begin: begin as usize,
                end: end as usize,
            }),
        },
    );
}

#[op2(fast)]
fn op_smudgy_line_highlight(
    scope: &mut v8::HandleScope,
    line_number: u32,
    state: &mut OpState,
    begin: u32,
    end: u32,
    fg_color: v8::Local<v8::Value>,
    bg_color: v8::Local<v8::Value>,
) {
    // Parse the style
    let style = match parse_style_from_js(
        scope,
        if fg_color.is_null_or_undefined() {
            None
        } else {
            Some(fg_color)
        },
        if bg_color.is_null_or_undefined() {
            None
        } else {
            Some(bg_color)
        },
    ) {
        Ok(style) => style,
        Err(_) => return, // Silently fail on error
    };

    let session_id = state.borrow::<SessionId>();

    // Get the runtime for this session
    let runtime = match registry::get_runtime(*session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };

    let _ = runtime.oob_tx.send(
        crate::session::runtime::RuntimeAction::PerformLineOperation {
            line_number: line_number as usize,
            operation: (LineOperation::Highlight {
                begin: begin as usize,
                end: end as usize,
                style,
            }),
        },
    );
}

#[op2(fast)]
fn op_smudgy_line_remove(state: &mut OpState, line_number: u32, begin: u32, end: u32) {
    let session_id = state.borrow::<SessionId>();

    // Get the runtime for this session
    let runtime = match registry::get_runtime(*session_id) {
        Some(runtime) => runtime,
        None => return, // Silently fail if session not found
    };

    let _ = runtime.oob_tx.send(
        crate::session::runtime::RuntimeAction::PerformLineOperation {
            line_number: line_number as usize,
            operation: (LineOperation::Remove {
                begin: begin as usize,
                end: end as usize,
            }),
        },
    );
}


#[allow(clippy::inline_always)]
#[op2]
fn op_smudgy_mapper_set_current_location(state: &mut OpState, #[serde] area_id: (u64, u64), room_number: Option<i32>) {
    // Get the runtime tx from OpState
    let tx = state.borrow::<UnboundedSender<RuntimeAction>>();

    let area_id = AreaId(Uuid::from_u64_pair(area_id.0, area_id.1));
    let _ = tx.send(RuntimeAction::SetCurrentLocation(area_id, room_number));
}

#[op2(fast)]
fn op_smudgy_capture(state: &mut OpState, value: bool) {
    let captured = state.borrow_mut::<Capture>();
    captured.0 = value;
}