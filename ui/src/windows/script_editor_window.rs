use std::collections::BTreeMap;
use std::sync::Arc;

use crate::assets::{self, bootstrap_icons, fonts};
use crate::components::Update;
use crate::helpers::hotkeys::{self as hotkey_helpers, MaybePhysicalKey};
use crate::theme::Element as ThemedElement;
use crate::theme::builtins::{self, button};
use crate::widgets::hotkey_input::HotkeyInput;
use iced::alignment::{Horizontal, Vertical};
use iced::font::Weight;
use iced::keyboard::Key;
use iced::widget::{
    self, Column, column, container, radio, row, scrollable, text, text_editor, text_input, tooltip,
};
use iced::{Element, Font, Length, Padding, Task};
use smudgy_core::models::aliases::AliasDefinition;
use smudgy_core::models::{self, ScriptLang, aliases, hotkeys, triggers};

#[derive(Debug, Clone)]
pub enum Message {
    AddFolder,
    AddAlias,
    AddTrigger,
    AddHotkey,
    ReloadScripts,
    ScriptsLoaded(BTreeMap<String, Script>, Arc<Vec<String>>),
    ShowErrorPane(Arc<Vec<String>>),
    ScriptSelected(Option<Box<str>>, Box<str>, Script),

    SetScriptName(String),
    SetScriptLanguage(ScriptLang),
    DiscardScriptChanges,
    DeleteScript,
    SaveScript,

    SetAliasPattern(String),

    MarkHotkeyState(Vec<MaybePhysicalKey>),

    // Trigger pattern messages
    AddTriggerPattern,
    RemoveTriggerPattern(usize),
    SetTriggerPatternAt(usize, String),
    AddTriggerAntiPattern,
    RemoveTriggerAntiPattern(usize),
    SetTriggerAntiPatternAt(usize, String),
    AddTriggerRawPattern,
    RemoveTriggerRawPattern(usize),
    SetTriggerRawPatternAt(usize, String),

    ScriptEditorAction(text_editor::Action),
}

#[derive(Debug, Clone)]
pub enum Event {
    ScriptsChanged { server_name: String },
}

#[derive(Debug, Clone)]
pub enum Script {
    Alias(models::aliases::AliasDefinition),
    Hotkey(models::hotkeys::HotkeyDefinition),
    Trigger(models::triggers::TriggerDefinition),
    Folder(bool, BTreeMap<String, Script>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ScriptType {
    Alias,
    Hotkey,
    Trigger,
    Folder,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ScriptListItem {
    script_type: ScriptType,
    name: String,
}

impl From<(&String, &Script)> for ScriptListItem {
    fn from((name, script): (&String, &Script)) -> Self {
        Self {
            name: name.clone(),
            script_type: ScriptType::from(script),
        }
    }
}

impl From<&Script> for ScriptType {
    fn from(script: &Script) -> Self {
        match script {
            Script::Alias(_) => ScriptType::Alias,
            Script::Hotkey(_) => ScriptType::Hotkey,
            Script::Trigger(_) => ScriptType::Trigger,
            Script::Folder(_, _) => ScriptType::Folder,
        }
    }
}

#[derive(Clone, Debug)]
enum ScriptEditorMode {
    Create,
    Edit,
}

#[derive(Default, Debug, Clone)]
pub enum Pane {
    #[default]
    WelcomePane,
    ErrorPane(Arc<Vec<String>>),
    /// Editor for creating or modifying a script
    ///
    /// # Fields
    /// * `mode` - Whether we're creating a new script or editing an existing one
    /// * `original_name` - The name of the script before editing (empty for new scripts)
    /// * `name` - The current name of the script being edited
    /// * `script` - The script definition being edited
    /// * `editor_content` - The content of the script editor
    /// * `error` - An error message to display on the pane
    ScriptEditor(
        ScriptEditorMode,
        Option<String>,
        Option<String>,
        Script,
        Option<String>,
    ),
}

struct ScriptKey {
    pub folder_name: Option<Box<str>>,
    pub script_name: Box<str>,
}

impl PartialEq for ScriptKey {
    fn eq(&self, other: &Self) -> bool {
        self.folder_name == other.folder_name && self.script_name == other.script_name
    }
}

impl Script {
    fn folder_name(&self) -> Option<&str> {
        match self {
            Script::Alias(alias) => alias.package.as_ref().map(|p| p.as_str()),
            Script::Hotkey(hotkey) => hotkey.package.as_ref().map(|p| p.as_str()),
            Script::Trigger(trigger) => trigger.package.as_ref().map(|p| p.as_str()),
            Script::Folder(_, _) => None,
        }
    }
}
pub struct ScriptEditorWindow {
    server_name: String,
    scripts: BTreeMap<String, Script>,
    scripts_flat: Vec<ScriptListItem>,
    selected_script: Option<ScriptKey>,
    pane: Pane,
    hotkey_state: Vec<MaybePhysicalKey>,
    editor_content: text_editor::Content,
}

const FIELD_HEIGHT: f32 = 32.0;

impl ScriptEditorWindow {
    pub fn new(server_name: String) -> Self {
        Self {
            server_name,
            scripts: BTreeMap::new(),
            scripts_flat: Vec::new(),
            pane: Default::default(),
            selected_script: None,
            editor_content: text_editor::Content::new(),
            hotkey_state: Vec::new(),
        }
    }

    pub fn init(&self) -> Task<Message> {
        Task::done(self.load_scripts())
    }

    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    fn load_scripts(&self) -> Message {
        let mut errors = Vec::new();

        let aliases = aliases::load_aliases(&self.server_name)
            .map_err(|e| errors.push(e.to_string()))
            .unwrap_or_default()
            .into_iter()
            .map(|(name, alias)| (name, Script::Alias(alias)));

        let hotkeys = hotkeys::load_hotkeys(&self.server_name)
            .map_err(|e| errors.push(e.to_string()))
            .unwrap_or_default()
            .into_iter()
            .map(|(name, hotkey)| (name, Script::Hotkey(hotkey)));

        let triggers = triggers::load_triggers(&self.server_name)
            .map_err(|e| errors.push(e.to_string()))
            .unwrap_or_default()
            .into_iter()
            .map(|(name, trigger)| (name, Script::Trigger(trigger)));

        let combined = aliases
            .into_iter()
            .chain(hotkeys)
            .chain(triggers)
            .collect::<Vec<(String, Script)>>();

        let mut scripts = BTreeMap::new();

        for (name, script) in combined {
            match upsert_script_folder(&mut scripts, script.folder_name()) {
                Ok(folder) => {
                    folder.insert(name, script);
                }
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            }
        }

        Message::ScriptsLoaded(scripts, Arc::new(errors))
    }

    /// Extracts all scripts of a specific type from the hierarchical structure.
    ///
    /// This method flattens the folder hierarchy and collects all scripts that match
    /// the provided extraction function. The extraction function should return `Some(T)`
    /// for scripts of the desired type and `None` for others.
    ///
    /// # Arguments
    /// * `extract_fn` - A function that attempts to extract a script of type T from a Script enum
    ///
    /// # Returns
    /// A HashMap mapping script names to their extracted definitions
    fn extract_scripts_by_type<T, F>(&self, extract_fn: F) -> std::collections::HashMap<String, T>
    where
        F: Fn(&Script) -> Option<T> + Copy,
        T: Clone,
    {
        let mut result = std::collections::HashMap::new();
        self.extract_scripts_recursive(&self.scripts, &mut result, extract_fn);
        result
    }

    /// Recursively traverses the folder hierarchy to extract scripts.
    ///
    /// This method walks through the BTreeMap structure, drilling down into folders
    /// and applying the extraction function to leaf scripts.
    ///
    /// # Arguments
    /// * `scripts` - The current level of the script hierarchy
    /// * `result` - The accumulating HashMap of extracted scripts
    /// * `extract_fn` - The extraction function to apply to each script
    fn extract_scripts_recursive<T, F>(
        &self,
        scripts: &BTreeMap<String, Script>,
        result: &mut std::collections::HashMap<String, T>,
        extract_fn: F,
    ) where
        F: Fn(&Script) -> Option<T> + Copy,
        T: Clone,
    {
        for (name, script) in scripts {
            match script {
                Script::Folder(_, folder_scripts) => {
                    // Recursively process folder contents
                    self.extract_scripts_recursive(folder_scripts, result, extract_fn);
                }
                _ => {
                    // Extract script if it matches the type
                    if let Some(extracted) = extract_fn(script) {
                        result.insert(name.clone(), extracted);
                    }
                }
            }
        }
    }

    /// Serializes all scripts to their respective JSON files.
    ///
    /// This method extracts all aliases, hotkeys, and triggers from the hierarchical
    /// structure and saves them to their respective JSON files (aliases.json,
    /// hotkeys.json, triggers.json) in the server directory.
    ///
    /// The hierarchical structure is flattened during serialization, as the JSON
    /// files store scripts in a flat map where the package field determines folder
    /// organization.
    ///
    /// # Returns
    /// * `Ok(())` if all scripts were successfully serialized
    /// * `Err(...)` if any serialization step failed
    fn serialize_scripts(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Extract aliases from the hierarchy
        let aliases = self.extract_scripts_by_type(|script| match script {
            Script::Alias(alias) => Some(alias.clone()),
            _ => None,
        });

        // Extract hotkeys from the hierarchy
        let hotkeys = self.extract_scripts_by_type(|script| match script {
            Script::Hotkey(hotkey) => Some(hotkey.clone()),
            _ => None,
        });

        // Extract triggers from the hierarchy
        let triggers = self.extract_scripts_by_type(|script| match script {
            Script::Trigger(trigger) => Some(trigger.clone()),
            _ => None,
        });

        // Save each type to its respective JSON file
        aliases::save_aliases(&self.server_name, &aliases)
            .map_err(|e| format!("Failed to save aliases: {}", e))?;

        hotkeys::save_hotkeys(&self.server_name, &hotkeys)
            .map_err(|e| format!("Failed to save hotkeys: {}", e))?;

        triggers::save_triggers(&self.server_name, &triggers)
            .map_err(|e| format!("Failed to save triggers: {}", e))?;

        Ok(())
    }

    /// Checks if a script with the given name exists in the hierarchy
    fn script_exists(&self, name: &str) -> bool {
        self.script_exists_recursive(&self.scripts, name)
    }

    /// Recursively checks if a script exists in the hierarchical structure
    fn script_exists_recursive(&self, scripts: &BTreeMap<String, Script>, name: &str) -> bool {
        for (script_name, script) in scripts {
            if script_name == name {
                return true;
            }
            if let Script::Folder(_, folder_scripts) = script {
                if self.script_exists_recursive(folder_scripts, name) {
                    return true;
                }
            }
        }
        false
    }

    /// Removes a script by name from the hierarchy
    fn remove_script_by_name(&mut self, name: &str) {
        Self::remove_script_recursive(&mut self.scripts, name);
    }

    /// Recursively removes a script from the hierarchical structure
    fn remove_script_recursive(scripts: &mut BTreeMap<String, Script>, name: &str) -> bool {
        if scripts.remove(name).is_some() {
            return true;
        }

        for (_, script) in scripts.iter_mut() {
            if let Script::Folder(_, folder_scripts) = script {
                if Self::remove_script_recursive(folder_scripts, name) {
                    return true;
                }
            }
        }

        false
    }

    /// Converts a HotkeyDefinition back to Vec<MaybePhysicalKey> format
    /// for initializing the HotkeyInput widget with existing shortcuts
    fn hotkey_definition_to_maybe_physical_keys(
        hotkey: &hotkeys::HotkeyDefinition,
    ) -> Vec<MaybePhysicalKey> {
        use iced::keyboard::{Key, key::Named};

        let mut keys = Vec::new();

        // Add modifiers first
        for modifier in &hotkey.modifiers {
            let modifier_key = match modifier.as_str() {
                "CTRL" => MaybePhysicalKey::Key(Key::Named(Named::Control)),
                "ALT" => MaybePhysicalKey::Key(Key::Named(Named::Alt)),
                "SHIFT" => MaybePhysicalKey::Key(Key::Named(Named::Shift)),
                "SUPER" => MaybePhysicalKey::Key(Key::Named(Named::Super)),
                _ => continue, // Skip unknown modifiers
            };
            keys.push(modifier_key);
        }

        // Add the main key
        let main_key = hotkey_helpers::hotkey_to_maybe_physical_key(hotkey);
        keys.push(main_key);

        keys
    }

    pub fn update(&mut self, message: Message) -> Update<Message, Event> {
        match message {
            Message::ScriptsLoaded(scripts, errors) => {
                self.scripts = scripts;
                if errors.is_empty() {
                    Update::none()
                } else {
                    Update::with_task(Task::done(Message::ShowErrorPane(errors)))
                }
            }
            Message::ShowErrorPane(errors) => {
                self.pane = Pane::ErrorPane(errors);
                Update::none()
            }
            Message::ScriptSelected(folder_name, name, script) => {
                self.selected_script = Some(ScriptKey {
                    folder_name: folder_name,
                    script_name: name.clone(),
                });

                // Don't allow editing folders
                if matches!(script, Script::Folder(_, _)) {
                    return Update::none();
                }

                // Load the script content into the editor
                let script_content = match &script {
                    Script::Alias(alias) => alias.script.clone().unwrap_or_default(),
                    Script::Hotkey(hotkey) => hotkey.script.clone().unwrap_or_default(),
                    Script::Trigger(trigger) => trigger.script.clone().unwrap_or_default(),
                    Script::Folder(_, _) => String::new(), // This case is already handled above
                };

                self.editor_content = text_editor::Content::with_text(&script_content);

                // For hotkeys, initialize the hotkey state with the existing shortcut
                if let Script::Hotkey(hotkey) = &script {
                    self.hotkey_state = Self::hotkey_definition_to_maybe_physical_keys(hotkey);
                }

                self.pane = Pane::ScriptEditor(
                    ScriptEditorMode::Edit,
                    Some(name.to_string()), // original_name
                    Some(name.to_string()), // current name
                    script,
                    None, // no error
                );
                Update::none()
            }
            Message::AddFolder => {
                self.pane = Pane::ScriptEditor(
                    ScriptEditorMode::Create,
                    None,
                    None,
                    Script::Folder(false, BTreeMap::new()),
                    None,
                );
                Update::none()
            }
            Message::AddAlias => {
                self.editor_content = text_editor::Content::with_text("");
                self.pane = Pane::ScriptEditor(
                    ScriptEditorMode::Create,
                    None,
                    None,
                    Script::Alias(aliases::AliasDefinition {
                        pattern: "".to_string(),
                        script: None,
                        package: None,
                        enabled: true,
                        language: ScriptLang::Plaintext,
                    }),
                    None,
                );
                Update::none()
            }
            Message::AddTrigger => {
                self.editor_content = text_editor::Content::with_text("");
                self.pane = Pane::ScriptEditor(
                    ScriptEditorMode::Create,
                    None,
                    None,
                    Script::Trigger(triggers::TriggerDefinition {
                        patterns: Some(vec!["".to_string()]),
                        anti_patterns: None,
                        raw_patterns: None,
                        script: None,
                        package: None,
                        language: ScriptLang::Plaintext,
                        enabled: true,
                        prompt: false,
                    }),
                    None,
                );
                Update::none()
            }
            Message::AddHotkey => {
                self.editor_content = text_editor::Content::with_text("");
                self.hotkey_state.clear();
                self.pane = Pane::ScriptEditor(
                    ScriptEditorMode::Create,
                    None,
                    None,
                    Script::Hotkey(hotkeys::HotkeyDefinition {
                        key: "".to_string(),
                        modifiers: vec![],
                        script: None,
                        package: None,
                        language: ScriptLang::Plaintext,
                        enabled: true,
                    }),
                    None,
                );
                Update::none()
            }
            Message::SetScriptName(name) => {
                match self.pane {
                    Pane::ScriptEditor(_, _, ref mut stored_name, _, _) => {
                        *stored_name = Some(name);
                    }
                    _ => return Update::none(),
                };

                Update::none()
            }
            Message::SetAliasPattern(pattern) => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Alias(alias) => {
                                alias.pattern = pattern;
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };

                Update::none()
            }
            Message::SetScriptLanguage(language) => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Alias(alias) => {
                                alias.language = language;
                            }
                            Script::Hotkey(hotkey) => {
                                hotkey.language = language;
                            }
                            Script::Trigger(trigger) => {
                                trigger.language = language;
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };

                Update::none()
            }

            Message::ScriptEditorAction(action) => {
                self.editor_content.perform(action);
                Update::none()
            }
            Message::DiscardScriptChanges => {
                self.pane = Pane::WelcomePane;
                Update::none()
            }
            Message::DeleteScript => {
                // Extract the original name from the current pane
                let original_name = match &self.pane {
                    Pane::ScriptEditor(ScriptEditorMode::Edit, Some(name), _, _, _) => name.clone(),
                    _ => {
                        return self.update(Message::ShowErrorPane(Arc::new(vec![
                            "Delete called on invalid pane or new script".to_string(),
                        ])));
                    }
                };

                // Remove the script from the hierarchy
                self.remove_script_by_name(&original_name);

                // Serialize all scripts to disk
                if let Err(serialize_error) = self.serialize_scripts() {
                    return self.update(Message::ShowErrorPane(Arc::new(vec![format!(
                        "Failed to save after delete: {}",
                        serialize_error
                    )])));
                }

                // Return to welcome pane
                self.pane = Pane::WelcomePane;
                Update::with_event(Event::ScriptsChanged {
                    server_name: self.server_name.clone(),
                })
            }
            Message::SaveScript => {
                // Extract values from pane to avoid borrowing issues
                let (mode, original_name, name, script) = match &mut self.pane {
                    Pane::ScriptEditor(mode, original_name, name, script, error) => {
                        *error = None;
                        let name = name.as_ref().unwrap_or(&String::from("")).clone();

                        if name.is_empty() {
                            *error = Some("Name cannot be empty".to_string());
                            return Update::none();
                        }

                        if !name.chars().all(|c| c.is_alphanumeric()) {
                            *error =
                                Some("Name can only contain alphanumeric characters".to_string());
                            return Update::none();
                        }

                        (mode.clone(), original_name.clone(), name, script.clone())
                    }
                    _ => {
                        return self.update(Message::ShowErrorPane(Arc::new(vec![
                            "Save called on invalid pane".to_string(),
                        ])));
                    }
                };

                // Check for name conflicts
                match mode {
                    ScriptEditorMode::Create => {
                        if self.script_exists(&name) {
                            if let Pane::ScriptEditor(_, _, _, _, ref mut error) = self.pane {
                                *error = Some("Name already in use".to_string());
                            }
                            return Update::none();
                        }
                    }
                    ScriptEditorMode::Edit => {
                        // Allow keeping the same name, but check for conflicts with different scripts
                        if let Some(ref orig_name) = original_name {
                            if &name != orig_name && self.script_exists(&name) {
                                if let Pane::ScriptEditor(_, _, _, _, ref mut error) = self.pane {
                                    *error = Some("Name already in use".to_string());
                                }
                                return Update::none();
                            }
                        }
                    }
                }

                // Handle hotkey-specific validation and updates
                let mut updated_script = script;
                if let Script::Hotkey(ref mut hotkey) = updated_script {
                    if !self.hotkey_state.is_empty() {
                        hotkey_helpers::set_key_and_modifiers_from_maybe_physical(
                            hotkey,
                            self.hotkey_state.clone(),
                        );
                    }
                }

                let final_script = match updated_script {
                    Script::Alias(alias) => Script::Alias(AliasDefinition {
                        script: Some(self.editor_content.text()),
                        ..alias
                    }),
                    Script::Hotkey(hotkey) => Script::Hotkey(hotkeys::HotkeyDefinition {
                        script: Some(self.editor_content.text()),
                        ..hotkey
                    }),
                    Script::Trigger(trigger) => Script::Trigger(triggers::TriggerDefinition {
                        script: Some(self.editor_content.text()),
                        ..trigger
                    }),
                    Script::Folder(_, _) => return Update::none(),
                };

                // Handle script renaming in edit mode
                if let ScriptEditorMode::Edit = mode {
                    if let Some(ref orig_name) = original_name {
                        if &name != orig_name {
                            // Remove the old script when renaming
                            self.remove_script_by_name(orig_name);
                        }
                    }
                }

                // Update in-memory state - insert into correct folder based on package
                match upsert_script_folder(&mut self.scripts, final_script.folder_name()) {
                    Ok(folder) => {
                        folder.insert(name.clone(), final_script);
                    }
                    Err(e) => {
                        if let Pane::ScriptEditor(_, _, _, _, ref mut error) = self.pane {
                            *error = Some(format!("Failed to organize script into folder: {}", e));
                        }
                        return Update::none();
                    }
                }

                // Serialize all scripts to disk
                if let Err(serialize_error) = self.serialize_scripts() {
                    if let Pane::ScriptEditor(_, _, _, _, ref mut error) = self.pane {
                        *error = Some(format!("Failed to save: {}", serialize_error));
                    }
                    return Update::none();
                }

                // Update pane state
                if let Pane::ScriptEditor(ref mut mode, ref mut original_name, _, _, _) = self.pane
                {
                    *mode = ScriptEditorMode::Edit;
                    *original_name = Some(name);
                }

                Update::with_event(Event::ScriptsChanged {
                    server_name: self.server_name.clone(),
                })
            }
            Message::MarkHotkeyState(keys) => {
                self.hotkey_state = keys;
                Update::none()
            }
            Message::AddTriggerPattern => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Trigger(trigger) => {
                                if trigger.patterns.is_none() {
                                    trigger.patterns = Some(Vec::new());
                                }
                                if let Some(patterns) = &mut trigger.patterns {
                                    patterns.push(String::new());
                                }
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };
                Update::none()
            }
            Message::RemoveTriggerPattern(index) => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Trigger(trigger) => {
                                if let Some(patterns) = &mut trigger.patterns {
                                    if index < patterns.len() {
                                        patterns.remove(index);
                                    }
                                }
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };
                Update::none()
            }
            Message::SetTriggerPatternAt(index, value) => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Trigger(trigger) => {
                                if let Some(patterns) = &mut trigger.patterns {
                                    if index < patterns.len() {
                                        patterns[index] = value;
                                    }
                                }
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };
                Update::none()
            }
            Message::AddTriggerAntiPattern => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Trigger(trigger) => {
                                if trigger.anti_patterns.is_none() {
                                    trigger.anti_patterns = Some(Vec::new());
                                }
                                if let Some(anti_patterns) = &mut trigger.anti_patterns {
                                    anti_patterns.push(String::new());
                                }
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };
                Update::none()
            }
            Message::RemoveTriggerAntiPattern(index) => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Trigger(trigger) => {
                                if let Some(anti_patterns) = &mut trigger.anti_patterns {
                                    if index < anti_patterns.len() {
                                        anti_patterns.remove(index);
                                    }
                                }
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };
                Update::none()
            }
            Message::SetTriggerAntiPatternAt(index, value) => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Trigger(trigger) => {
                                if let Some(anti_patterns) = &mut trigger.anti_patterns {
                                    if index < anti_patterns.len() {
                                        anti_patterns[index] = value;
                                    }
                                }
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };
                Update::none()
            }
            Message::AddTriggerRawPattern => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Trigger(trigger) => {
                                if trigger.raw_patterns.is_none() {
                                    trigger.raw_patterns = Some(Vec::new());
                                }
                                if let Some(raw_patterns) = &mut trigger.raw_patterns {
                                    raw_patterns.push(String::new());
                                }
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };
                Update::none()
            }
            Message::RemoveTriggerRawPattern(index) => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Trigger(trigger) => {
                                if let Some(raw_patterns) = &mut trigger.raw_patterns {
                                    if index < raw_patterns.len() {
                                        raw_patterns.remove(index);
                                    }
                                }
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };
                Update::none()
            }
            Message::SetTriggerRawPatternAt(index, value) => {
                match self.pane {
                    Pane::ScriptEditor(_, _, _, ref mut script, _) => {
                        match script {
                            Script::Trigger(trigger) => {
                                if let Some(raw_patterns) = &mut trigger.raw_patterns {
                                    if index < raw_patterns.len() {
                                        raw_patterns[index] = value;
                                    }
                                }
                            }
                            _ => return Update::none(),
                        };
                    }
                    _ => return Update::none(),
                };
                Update::none()
            }
            Message::ReloadScripts => Update::none(),
        }
    }

    fn view_script_column<'a>(&self, script_map: &'a BTreeMap<String, Script>) -> ThemedElement<'a, Message> {
        let script_items =
            Self::build_script_tree_items(script_map, 0, self.selected_script.as_ref());

        let mut script_list_tree = Column::new().spacing(5).padding(5);
        for item in script_items {
            script_list_tree = script_list_tree.push(item);
        }

        scrollable(script_list_tree).into()
    }

    fn build_script_tree_items<'a>(
        script_map: &'a BTreeMap<String, Script>,
        indent_level: usize,
        selected_script: Option<&ScriptKey>,
    ) -> Vec<ThemedElement<'a, Message>> {
        let mut items = Vec::new();

        for (name, script) in script_map {
            let is_selected = selected_script.as_ref()
                .map(| sk| {
                    sk.script_name.as_ref() == name && 
                    sk.folder_name.as_ref().map(|f| f.as_ref()) == script.folder_name()
                })
                .unwrap_or(false);

            let icon = match script {
                Script::Folder(_, _) => bootstrap_icons::FOLDER_PLUS,
                Script::Alias(_) => bootstrap_icons::AT,
                Script::Trigger(_) => bootstrap_icons::LIGHTNING,
                Script::Hotkey(_) => bootstrap_icons::DPAD,
            };

            let padding = Padding {
                left: (indent_level * 20) as f32,
                right: 0.0,
                top: 0.0,
                bottom: 0.0,
            };

            let item_button = widget::button(
                row![
                    text(icon).font(fonts::BOOTSTRAP_ICONS).size(16.0),
                    text(name.clone())
                ]
                .spacing(8)
                .align_y(Vertical::Center),
            )
            .style(if is_selected { button::list_item_selected } else { button::list_item })
            .on_press(Message::ScriptSelected(
                script.folder_name().map(Box::from),
                Box::from(name.as_str()),
                script.clone(),
            ))
            .padding(padding)
            .width(Length::Fill);

            items.push(item_button.into());

            // Recursively add folder contents
            if let Script::Folder(_, folder_contents) = script {
                let mut child_items =
                    Self::build_script_tree_items(folder_contents, indent_level + 1, selected_script);
                items.append(&mut child_items);
            }
        }

        items
    }

    fn view_script_list_pane(&self) -> impl Into<ThemedElement<Message>> {
        let button_bar = row![
            tooltip(
                widget::button(
                    text(bootstrap_icons::FOLDER_PLUS)
                        .font(fonts::BOOTSTRAP_ICONS)
                        .size(24.0)
                )
                .style(button::secondary),
                // .on_press(Message::AddFolder),
                "Create folder",
                widget::tooltip::Position::Bottom
            ),
            tooltip(
                widget::button(
                    text(bootstrap_icons::AT)
                        .font(fonts::BOOTSTRAP_ICONS)
                        .size(24.0)
                )
                .style(button::secondary)
                .on_press(Message::AddAlias),
                "Create alias",
                widget::tooltip::Position::Bottom
            ),
            tooltip(
                widget::button(
                    text(bootstrap_icons::LIGHTNING)
                        .font(fonts::BOOTSTRAP_ICONS)
                        .size(24.0)
                )
                .style(button::secondary)
                .on_press(Message::AddTrigger),
                "Create trigger",
                widget::tooltip::Position::Bottom
            ),
            tooltip(
                widget::button(
                    text(bootstrap_icons::DPAD)
                        .font(fonts::BOOTSTRAP_ICONS)
                        .size(24.0)
                )
                .style(button::secondary)
                .on_press(Message::AddHotkey),
                "Create hotkey",
                widget::tooltip::Position::Bottom
            ),
        ]
        .spacing(10);

        column![button_bar, self.view_script_column(&self.scripts)]
            .height(Length::Fill)
            .width(250.0)
    }

    fn view_error_pane(&self, errors: &[String]) -> impl Into<ThemedElement<Message>> {
        column(errors.iter().map(|err| text(err.to_string()).into()))
            .width(Length::Fill)
            .height(Length::Fill)
    }

    fn view_welcome_pane(&self) -> impl Into<ThemedElement<Message>> {
        column![text("Welcome Pane"),]
            .width(Length::Fill)
            .height(Length::Fill)
    }

    fn view_alias_pane(
        &self,
        name: &Option<String>,
        alias: &models::aliases::AliasDefinition,
    ) -> impl Into<ThemedElement<Message>> {
        column![row![
            column![
                text("Name")
                    .height(FIELD_HEIGHT)
                    .align_y(Vertical::Center)
                    .align_x(Horizontal::Left),
                text("Pattern")
                    .height(FIELD_HEIGHT)
                    .align_y(Vertical::Center)
                    .align_x(Horizontal::Left),
                text("Behavior")
                    .height(FIELD_HEIGHT)
                    .align_y(Vertical::Center)
                    .align_x(Horizontal::Left),
            ]
            .padding(Padding {
                bottom: 10.0,
                top: 10.0,
                left: 0.0,
                right: 0.0,
            })
            .spacing(10),
            column![
                text_input(
                    "e.g. myCoolAlias",
                    name.as_ref().map(|n| n.as_str()).unwrap_or("")
                )
                .on_input(Message::SetScriptName),
                text_input("e.g. ^do_thing$", alias.pattern.as_str())
                    .on_input(Message::SetAliasPattern),
                row![
                    radio(
                        "Send as text",
                        ScriptLang::Plaintext,
                        Some(alias.language),
                        Message::SetScriptLanguage
                    ),
                    radio(
                        "JavaScript",
                        ScriptLang::JS,
                        Some(alias.language),
                        Message::SetScriptLanguage
                    ),
                ]
                .spacing(20)
                .padding(Padding {
                    bottom: 00.0,
                    top: 8.0,
                    left: 0.0,
                    right: 0.0,
                })
                .height(FIELD_HEIGHT),
            ]
            .padding(10)
            .spacing(10),
        ],]
    }

    fn view_hotkey_pane(
        &self,
        name: &Option<String>,
        hotkey: &models::hotkeys::HotkeyDefinition,
    ) -> impl Into<ThemedElement<Message>> {
        column![row![
            column![
                text("Name")
                    .height(FIELD_HEIGHT)
                    .align_y(Vertical::Center)
                    .align_x(Horizontal::Left),
                text("Shortcut")
                    .height(FIELD_HEIGHT)
                    .align_y(Vertical::Center)
                    .align_x(Horizontal::Left),
                text("Behavior")
                    .height(FIELD_HEIGHT)
                    .align_y(Vertical::Center)
                    .align_x(Horizontal::Left),
            ]
            .padding(Padding {
                bottom: 10.0,
                top: 10.0,
                left: 0.0,
                right: 0.0,
            })
            .spacing(10),
            column![
                text_input(
                    "e.g. doTheThing",
                    name.as_ref().map(|n| n.as_str()).unwrap_or("")
                )
                .on_input(Message::SetScriptName),
                Element::new(
                    HotkeyInput::new(&self.hotkey_state, true)
                        .height(FIELD_HEIGHT)
                        .on_action(Message::MarkHotkeyState)
                ),
                row![
                    radio(
                        "Send as text",
                        ScriptLang::Plaintext,
                        Some(hotkey.language),
                        Message::SetScriptLanguage
                    ),
                    radio(
                        "JavaScript",
                        ScriptLang::JS,
                        Some(hotkey.language),
                        Message::SetScriptLanguage
                    ),
                ]
                .spacing(20)
                .padding(Padding {
                    bottom: 00.0,
                    top: 8.0,
                    left: 0.0,
                    right: 0.0,
                })
                .height(FIELD_HEIGHT),
            ]
            .padding(10)
            .spacing(10),
        ],]
    }

    fn view_trigger_pane(
        &self,
        name: &Option<String>,
        trigger: &models::triggers::TriggerDefinition,
    ) -> impl Into<ThemedElement<Message>> {
        let mut patterns_section = Column::new().spacing(5);
        let num_patterns = trigger.patterns.as_ref().map(|p| p.len()).unwrap_or(0);
        let num_anti_patterns = trigger.anti_patterns.as_ref().map(|p| p.len()).unwrap_or(0);
        let num_raw_patterns = trigger.raw_patterns.as_ref().map(|p| p.len()).unwrap_or(0);

        if let Some(patterns) = &trigger.patterns {
            for (i, pattern) in patterns.iter().enumerate() {
                patterns_section = patterns_section.push(
                    row![
                        text_input("e.g. ^pattern$", pattern)
                            .on_input(move |value| Message::SetTriggerPatternAt(i, value))
                            .width(Length::Fill),
                        widget::button(
                            text(bootstrap_icons::TRASH_3)
                                .font(fonts::BOOTSTRAP_ICONS)
                                .size(16.0)
                        )
                        .style(button::secondary)
                        .on_press(Message::RemoveTriggerPattern(i))
                        .padding(8)
                    ]
                    .spacing(5),
                );
            }
        }

        // Anti-patterns section
        let mut anti_patterns_section = Column::new().spacing(5);

        if let Some(anti_patterns) = &trigger.anti_patterns {
            for (i, pattern) in anti_patterns.iter().enumerate() {
                anti_patterns_section = anti_patterns_section.push(
                    row![
                        text_input("e.g. ^exclude$", pattern)
                            .on_input(move |value| Message::SetTriggerAntiPatternAt(i, value))
                            .width(Length::Fill),
                        widget::button(
                            text(bootstrap_icons::TRASH_3)
                                .font(fonts::BOOTSTRAP_ICONS)
                                .size(16.0)
                        )
                        .style(button::secondary)
                        .on_press(Message::RemoveTriggerAntiPattern(i))
                        .padding(8)
                    ]
                    .spacing(5),
                );
            }
        }

        // Raw patterns section
        let mut raw_patterns_section = Column::new().spacing(5);

        if let Some(raw_patterns) = &trigger.raw_patterns {
            for (i, pattern) in raw_patterns.iter().enumerate() {
                raw_patterns_section = raw_patterns_section.push(
                    row![
                        text_input("e.g. raw pattern", pattern)
                            .on_input(move |value| Message::SetTriggerRawPatternAt(i, value))
                            .width(Length::Fill),
                        widget::button(
                            text(bootstrap_icons::TRASH_3)
                                .font(fonts::BOOTSTRAP_ICONS)
                                .size(16.0)
                        )
                        .style(button::secondary)
                        .on_press(Message::RemoveTriggerRawPattern(i))
                        .padding(8)
                    ]
                    .spacing(5),
                );
            }
        }

        let mut labels = Column::new().padding(Padding {
            bottom: 10.0,
            top: 10.0,
            left: 0.0,
            right: 0.0,
        });

        labels = labels.push(
            text("Name")
                .height(FIELD_HEIGHT)
                .align_y(Vertical::Center)
                .align_x(Horizontal::Left),
        );

        if num_patterns > 0 {
            labels = labels.push(
                text("Patterns")
                    .height(
                        FIELD_HEIGHT * num_patterns as f32 + (10.0 * (num_patterns as f32 - 1.0)),
                    )
                    .align_y(Vertical::Center)
                    .align_x(Horizontal::Left),
            );
        }

        if num_anti_patterns > 0 {
            labels = labels.push(
                text("Anti-Patterns")
                    .height(
                        FIELD_HEIGHT * num_anti_patterns as f32
                            + (10.0 * (num_anti_patterns as f32 - 1.0)),
                    )
                    .align_y(Vertical::Center)
                    .align_x(Horizontal::Left),
            );
        }

        if num_raw_patterns > 0 {
            labels = labels.push(
                text("Raw Patterns")
                    .height(
                        FIELD_HEIGHT * num_raw_patterns as f32
                            + (10.0 * (num_raw_patterns as f32 - 1.0)),
                    )
                    .align_y(Vertical::Center)
                    .align_x(Horizontal::Left),
            );
        }

        labels = labels.push(
            text("Behavior")
                .height(FIELD_HEIGHT)
                .align_y(Vertical::Center)
                .align_x(Horizontal::Left),
        );

        column![row![
            labels,
            column![
                text_input(
                    "e.g. myTrigger",
                    name.as_ref().map(|n| n.as_str()).unwrap_or("")
                )
                .on_input(Message::SetScriptName),
                patterns_section,
                anti_patterns_section,
                raw_patterns_section,
                row![
                    radio(
                        "Send as text",
                        ScriptLang::Plaintext,
                        Some(trigger.language),
                        Message::SetScriptLanguage
                    ),
                    radio(
                        "JavaScript",
                        ScriptLang::JS,
                        Some(trigger.language),
                        Message::SetScriptLanguage
                    ),
                ]
                .spacing(20)
                .padding(Padding {
                    bottom: 0.0,
                    top: 8.0,
                    left: 0.0,
                    right: 0.0,
                })
                .height(FIELD_HEIGHT),
                widget::button("Add Pattern")
                    .style(button::secondary)
                    .on_press(Message::AddTriggerPattern),
                widget::button("Add Anti-Pattern")
                    .style(button::secondary)
                    .on_press(Message::AddTriggerAntiPattern),
                widget::button("Add Raw Pattern")
                    .style(button::secondary)
                    .on_press(Message::AddTriggerRawPattern)
            ]
            .padding(10)
            .spacing(10),
        ],]
    }

    fn view_script_editor_pane(
        &self,
        mode: &ScriptEditorMode,
        _original_name: &Option<String>,
        name: &Option<String>,
        script: &Script,
        error: &Option<String>,
    ) -> impl Into<ThemedElement<Message>> {
        let (title, pane, language_token) = match script {
            Script::Alias(alias) => (
                "Alias",
                column![self.view_alias_pane(name, alias).into()],
                match alias.language {
                    ScriptLang::JS => "js".to_string(),
                    ScriptLang::TS => "ts".to_string(),
                    ScriptLang::Plaintext => "txt".to_string(),
                },
            ),
            Script::Hotkey(hotkey) => (
                "Hotkey",
                column![self.view_hotkey_pane(name, hotkey).into()],
                match hotkey.language {
                    ScriptLang::JS => "js".to_string(),
                    ScriptLang::TS => "ts".to_string(),
                    ScriptLang::Plaintext => "txt".to_string(),
                },
            ),
            Script::Trigger(trigger) => (
                "Trigger",
                column![self.view_trigger_pane(name, trigger).into()],
                match trigger.language {
                    ScriptLang::JS => "js".to_string(),
                    ScriptLang::TS => "ts".to_string(),
                    ScriptLang::Plaintext => "txt".to_string(),
                },
            ),
            Script::Folder(_, _) => ("Folder", column![text("Folder Pane")], "txt".to_string()),
        };

        let title = text(format!("{:?} {title}", mode))
            .font(Font {
                weight: Weight::Light,
                ..assets::fonts::GEIST_VF
            })
            .size(36.0);

        let mut column = Column::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .spacing(10)
            .push(title)
            .push(iced::widget::horizontal_rule(1.0));

        if let Some(error) = error {
            column = column.push(text(error.clone()).style(builtins::text::danger));
        }

        let editor = text_editor(&self.editor_content)
            .highlight_with::<iced::highlighter::Highlighter>(
                iced::highlighter::Settings {
                    theme: iced::highlighter::Theme::SolarizedDark,
                    token: language_token,
                },
                |h: &iced::highlighter::Highlight, _| h.to_format(),
            )
            .on_action(Message::ScriptEditorAction)
            .height(Length::Fill);

        column
            .push(pane)
            .push(container(editor).padding(Padding {
                bottom: 10.0,
                top: 0.0,
                left: 0.0,
                right: 10.0,
            }))
            .push(container({
                let mut buttons = row![];

                // Only show delete button for existing scripts (Edit mode)
                if matches!(mode, ScriptEditorMode::Edit) {
                    buttons = buttons.push(
                        widget::button("Delete")
                            .style(builtins::button::secondary)
                            .on_press(Message::DeleteScript),
                    );
                    buttons = buttons.push(widget::horizontal_space());
                }

                // Always show discard and save buttons
                buttons = buttons
                    .push(
                        widget::button("Discard Changes")
                            .style(builtins::button::secondary)
                            .on_press(Message::DiscardScriptChanges),
                    )
                    .push(widget::button("Save").on_press(Message::SaveScript));

                buttons.padding(10).spacing(20).align_y(Vertical::Center)
            }))
    }

    fn view_main_pane(&self) -> impl Into<ThemedElement<Message>> {
        match self.pane {
            Pane::WelcomePane => self.view_welcome_pane().into(),
            Pane::ErrorPane(ref errors) => self.view_error_pane(errors).into(),
            Pane::ScriptEditor(ref mode, ref original_name, ref name, ref script, ref error) => {
                self.view_script_editor_pane(mode, original_name, name, script, error)
                    .into()
            }
        }
    }

    pub fn view(&self) -> ThemedElement<Message> {
        row![
            self.view_script_list_pane().into(),
            self.view_main_pane().into()
        ]
        .padding(10)
        .spacing(20)
        .into()
    }
}

fn upsert_script_folder<'a>(
    scripts: &'a mut BTreeMap<String, Script>,
    folder_name: Option<&str>,
) -> Result<&'a mut BTreeMap<String, Script>, String> {
    let mut current_folder = scripts;

    if let Some(folder_name) = folder_name {
        let folders = folder_name.split('/').enumerate();

        for (i, folder) in folders {
            let existing_folder_entry = match current_folder.get_mut(folder) {
                Some(Script::Folder(_, folder)) => Some(folder),
                Some(_) => {
                    return Err(format!(
                        "Failed to load a script which belongs in '{}', which is not a folder",
                        folder_name
                            .split('/')
                            .take(i)
                            .collect::<Vec<&str>>()
                            .join("/")
                    ));
                }
                None => None,
            };

            if existing_folder_entry.is_none() {
                current_folder.insert(folder.to_string(), Script::Folder(false, BTreeMap::new()));
            }

            current_folder = match current_folder.get_mut(folder) {
                Some(Script::Folder(_, folder)) => folder,
                _ => return Err("Failed to create a script folder".to_string()),
            };
        }
    }

    Ok(current_folder)
}
