use std::{collections::HashMap, sync::Arc};

use tokio::sync::mpsc::UnboundedSender;

use crate::script_runtime::{RuntimeAction, ScriptRuntime};

pub enum HotkeyResult {
    Processed,
    Unrecognized,
}

impl From<Option<&dyn ExactSizeIterator<Item = &Hotkey>>> for HotkeyResult {
    fn from(value: Option<&dyn ExactSizeIterator<Item = &Hotkey>>) -> Self {
        match value {
            Some(value) if !value.is_empty() => HotkeyResult::Processed,
            _ => HotkeyResult::Unrecognized,
        }
    }
}
pub struct HotkeyManager {
    hotkeys: HashMap<i32, Vec<Hotkey>>,
    script_eval_tx: UnboundedSender<RuntimeAction>,
}

impl HotkeyManager {
    pub fn new(script_runtime: Arc<ScriptRuntime>) -> Self {
        let hotkeys = HashMap::new();

        let mut me = Self {
            hotkeys,
            script_eval_tx: script_runtime.tx(),
        };

        me.push(Hotkey {
            name: "n".into(),
            scancode: 72,
            script: RuntimeAction::StringLiteralCommand(Arc::new("n".into())),
        });
        me.push(Hotkey {
            name: "e".into(),
            scancode: 77,
            script: RuntimeAction::StringLiteralCommand(Arc::new("e".into())),
        });
        me.push(Hotkey {
            name: "s".into(),
            scancode: 80,
            script: RuntimeAction::StringLiteralCommand(Arc::new("s".into())),
        });
        me.push(Hotkey {
            name: "w".into(),
            scancode: 75,
            script: RuntimeAction::StringLiteralCommand(Arc::new("w".into())),
        });
        me.push(Hotkey {
            name: "u".into(),
            scancode: 73,
            script: RuntimeAction::StringLiteralCommand(Arc::new("u".into())),
        });
        me.push(Hotkey {
            name: "d".into(),
            scancode: 81,
            script: RuntimeAction::StringLiteralCommand(Arc::new("d".into())),
        });
        me.push(Hotkey {
            name: "st".into(),
            scancode: 71,
            script: RuntimeAction::StringLiteralCommand(Arc::new("st".into())),
        });
        me.push(Hotkey {
            name: "rest".into(),
            scancode: 79,
            script: RuntimeAction::StringLiteralCommand(Arc::new("rest".into())),
        });
        me.push(Hotkey {
            name: "scan".into(),
            scancode: 78,
            script: RuntimeAction::StringLiteralCommand(Arc::new("scan".into())),
        });
        me.push(Hotkey {
            name: "look".into(),
            scancode: 76,
            script: RuntimeAction::StringLiteralCommand(Arc::new("look".into())),
        });

        me
    }

    fn push(&mut self, hotkey: Hotkey) {
        match self.hotkeys.get_mut(&hotkey.scancode) {
            Some(vec) => {
                vec.push(hotkey);
            }
            None => {
                self.hotkeys.insert(hotkey.scancode, vec![hotkey]);
            }
        }
    }

    pub fn process_keypress(&self, ev: &i_slint_core::items::KeyEvent) -> HotkeyResult {
        if let Some(keys) = self.hotkeys.get(&ev.scancode) {
            let num_matched = keys
                .iter()
                .filter(|hotkey| hotkey.matches(ev))
                .map(|hotkey| self.script_eval_tx.send(hotkey.script.clone()).unwrap())
                .count();
            if num_matched > 0 {
                HotkeyResult::Processed
            } else {
                HotkeyResult::Unrecognized
            }
        } else {
            HotkeyResult::Unrecognized
        }
    }
}

struct Hotkey {
    pub name: String,
    pub scancode: i32,
    pub script: RuntimeAction,
}

impl Hotkey {
    fn new(name: String, scancode: i32, script: RuntimeAction) -> Self {
        Self {
            name,
            scancode,
            script,
        }
    }

    pub fn matches(&self, ev: &i_slint_core::items::KeyEvent) -> bool {
        true
    }
}
