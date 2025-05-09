use std::sync::Arc;

use tokio::sync::mpsc::UnboundedSender;
use anyhow::Result;
use crate::session::runtime::{script_engine::{FunctionId, ScriptId}, RuntimeAction};

#[derive(Clone, Debug)]
pub enum ScriptAction {
    Noop,
    SendRaw(Arc<String>),
    SendSimple(Arc<String>),
    EvalJavascript(ScriptId),
    CallJavascriptFunction(FunctionId),
}
