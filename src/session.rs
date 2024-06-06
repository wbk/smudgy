use std::{
    collections::HashSet,
    num::{NonZeroU32},
    rc::Rc,
    sync::{Arc, Mutex},
};

use crate::{
    hotkey::{HotkeyManager, HotkeyResult},
    script_runtime::ScriptRuntime,
    trigger::TriggerManager,
    SessionKeyPressResponse, SessionKeyPressResponseType,
};

use command_history::CommandHistory;
use connection::Connection;
use regex::Regex;
use slint::VecModel;
use terminal_view::TerminalView;

use crate::{AutocompleteResult, MainWindow};
use incoming_line_history::IncomingLineHistory;

mod command_history;

mod connection;

pub mod incoming_line_history;
mod styled_line;
mod terminal_view;
pub use styled_line::StyledLine;
pub use terminal_view::ViewAction;

// Regex which matches on word boundaries
static BOUNDARY_REGEX: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\b").unwrap());

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Default)]
struct AutocompleteState {
    autocomplete_prefix: String,
    text_prior_to_autocomplete: String,
    history: HashSet<String>,
}

pub struct Session {
    pub id: Arc<Mutex<i32>>,
    incoming_line_history: Arc<Mutex<IncomingLineHistory>>,
    view: Rc<TerminalView>,
    trigger_manager: Arc<TriggerManager>,
    profile: Profile,
    synced_width: NonZeroU32,
    synced_height: NonZeroU32,
    autocomplete_state: AutocompleteState,
    command_history: CommandHistory,
    hotkey_manager: HotkeyManager,

    // ----
    connection: Connection,
}

impl Session {
    pub fn new(id: i32, weak_window: slint::Weak<MainWindow>, profile: Profile) -> Session {
        let id = Arc::new(Mutex::new(id));
        let view = Rc::new(TerminalView::new(weak_window.clone()));

        let incoming_line_history = Arc::new(Mutex::new(IncomingLineHistory::new()));
        let script_runtime = Arc::new(ScriptRuntime::new(
            view.tx.clone(),
            weak_window.clone(),
            incoming_line_history.clone(),
        ));

        let trigger_manager = Arc::new(TriggerManager::new(script_runtime.tx()));

        let connection = Connection::new(trigger_manager.clone(), script_runtime.clone());

        let hotkey_manager = HotkeyManager::new(script_runtime.clone());

        Self {
            id,
            view,
            incoming_line_history,
            profile: profile.clone(),
            synced_width: NonZeroU32::MIN,
            synced_height: NonZeroU32::MIN,
            autocomplete_state: AutocompleteState::default(),
            command_history: CommandHistory::default(),
            hotkey_manager,
            trigger_manager,
            connection,
        }
    }

    pub fn set_id(&mut self, new_id: i32) {
        let mut id = self.id.lock().unwrap();
        *id = new_id
    }

    pub fn prepare_render(&self, width: u32, height: u32) {
        let nz_width = NonZeroU32::new(width).unwrap_or(NonZeroU32::MIN);
        let nz_height = NonZeroU32::new(height).unwrap_or(NonZeroU32::MIN);

        if self.synced_width != nz_width || self.synced_height != nz_height {
            self.view.set_viewable_size(nz_width, nz_height);
            self.view.handle_incoming_lines();
        }
    }

    pub fn on_session_accepted(&mut self, line: &str) {
        self.command_history.push(&line);
        self.trigger_manager.process_outgoing_line(line);
    }

    pub fn on_history_up(&mut self, input_line: &str) -> SessionKeyPressResponse {
        match self.command_history.next(input_line) {
            Some(str) => SessionKeyPressResponse {
                response: SessionKeyPressResponseType::ReplaceInput,
                str_args: Rc::new(VecModel::from(vec![str.into()])).into(),
                int_args: Rc::new(VecModel::from(vec![])).into(),
            },
            _ => SessionKeyPressResponse {
                response: SessionKeyPressResponseType::Accept,
                str_args: Rc::new(VecModel::from(vec![])).into(),
                int_args: Rc::new(VecModel::from(vec![])).into(),
            },
        }
    }

    pub fn on_history_down(&mut self, _input_line: &str) -> SessionKeyPressResponse {
        match self.command_history.prev() {
            Some(str) => SessionKeyPressResponse {
                response: SessionKeyPressResponseType::ReplaceInput,
                str_args: Rc::new(VecModel::from(vec![str.into()])).into(),
                int_args: Rc::new(VecModel::from(vec![])).into(),
            },
            _ => SessionKeyPressResponse {
                response: SessionKeyPressResponseType::Accept,
                str_args: Rc::new(VecModel::from(vec![])).into(),
                int_args: Rc::new(VecModel::from(vec![])).into(),
            },
        }
    }

    pub fn on_key_pressed(
        &mut self,
        ev: i_slint_core::items::KeyEvent,
        input_line: &str,
    ) -> SessionKeyPressResponse {
        if ev.modifiers.control {
            println!("{ev:?}");
        }

        match self.hotkey_manager.process_keypress(&ev) {
            HotkeyResult::Processed => {
                return SessionKeyPressResponse {
                    response: SessionKeyPressResponseType::Accept,
                    str_args: Rc::new(VecModel::from(vec![])).into(),
                    int_args: Rc::new(VecModel::from(vec![])).into(),
                }
            }
            _ => {}
        }

        if !ev.modifiers.alt && !ev.modifiers.shift && !ev.modifiers.meta && !ev.modifiers.control {
            if ev.scancode == 0xe048 {
                self.on_history_up(&input_line)
            } else if ev.scancode == 0xe050 {
                self.on_history_down(&input_line)
            } else {
                SessionKeyPressResponse {
                    response: SessionKeyPressResponseType::Reject,
                    str_args: Rc::new(VecModel::from(vec![])).into(),
                    int_args: Rc::new(VecModel::from(vec![])).into(),
                }
            }
        } else {
            SessionKeyPressResponse {
                response: SessionKeyPressResponseType::Reject,
                str_args: Rc::new(VecModel::from(vec![])).into(),
                int_args: Rc::new(VecModel::from(vec![])).into(),
            }
        }
    }

    pub fn on_request_autocomplete(
        &mut self,
        line: &str,
        continue_from_last_request: bool,
    ) -> AutocompleteResult {
        if !continue_from_last_request {
            self.autocomplete_state.history.clear();

            let all_words: Vec<&str> = line.split_inclusive(&*BOUNDARY_REGEX).collect();
            let last_word = all_words.last().or(Some(&"")).unwrap().trim();

            let mut text_prior = all_words.join("");
            text_prior.truncate(std::cmp::max(text_prior.len() - last_word.len(), 0));
            self.autocomplete_state.text_prior_to_autocomplete = text_prior;
            self.autocomplete_state.autocomplete_prefix = last_word.to_string();
        }

        let scrollback_guard = self.incoming_line_history.lock().unwrap();
        let search_result = scrollback_guard.find_recent_word_by_prefix(
            &self.autocomplete_state.autocomplete_prefix,
            Some(&self.autocomplete_state.history),
            500,
        );
        drop(scrollback_guard);

        match search_result {
            Some(found) => {
                self.autocomplete_state.history.insert(found.clone());

                let mut new_line = self.autocomplete_state.text_prior_to_autocomplete.clone();
                new_line.push_str(&found);

                AutocompleteResult {
                    success: true,
                    new_line: new_line.into(),
                    autocompleted_start: self.autocomplete_state.text_prior_to_autocomplete.len()
                        as i32,
                    autocompleted_end: (self.autocomplete_state.text_prior_to_autocomplete.len()
                        + found.len()) as i32,
                }
            }
            None => AutocompleteResult {
                success: false,
                new_line: "".into(),
                autocompleted_start: 0,
                autocompleted_end: 0,
            },
        }
    }

    pub fn view(&self) -> Rc<TerminalView> {
        self.view.clone()
    }

    pub fn connect(&mut self) {
        self.connection
            .connect(&self.profile.host, self.profile.port);
    }
}
