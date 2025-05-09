use serde::{Deserialize, Serialize};

pub mod aliases;
pub mod hotkeys;
pub mod modules;
pub mod packages;
pub mod profile;
pub mod server;
pub mod settings;
pub mod triggers;

/// Represents the programming language of a script.
#[derive(Serialize, Deserialize, Debug, Default,Clone, Copy, PartialEq, Eq)]
pub enum ScriptLang {
    #[default]
    Plaintext,
    JS,
    TS,
}
