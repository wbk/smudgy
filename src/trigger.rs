use std::sync::{Arc, Mutex};

use regex::{Regex, RegexSet};
use tokio::sync::{mpsc::UnboundedSender, oneshot};

use crate::{script_runtime::RuntimeAction, session::StyledLine};

pub enum TriggerResult {
    Processed,
    Unrecognized,
}

#[derive(Clone, Debug)]
enum Action {
    Noop,
    SendRaw(Arc<String>),
    ProcessAlias(Arc<String>),
    ProcessJavascriptAlias(usize),
}

// Similar to above, but allows for args to be passed to scripts
#[derive(Clone, Debug)]
pub enum InterpretedAction {
    Noop,
    SendRaw(Arc<String>),
    ProcessAlias(Arc<String>),
    ProcessJavascriptAlias(usize, Arc<Vec<(String, String)>>),
}

#[derive(Debug)]
pub struct TriggerManager {
    trigger_regex_set: Arc<Mutex<RegexSet>>,
    alias_regex_set: Arc<Mutex<RegexSet>>,
    triggers: Arc<Mutex<Vec<Trigger>>>,
    aliases: Arc<Mutex<Vec<Alias>>>,
    script_eval_tx: UnboundedSender<RuntimeAction>,
}

impl TriggerManager {
    pub fn new(script_eval_tx: UnboundedSender<RuntimeAction>) -> Self {
        let triggers = Arc::new(Mutex::new(Vec::new()));
        let aliases = Arc::new(Mutex::new(Vec::new()));
        let trigger_regex_set = Arc::new(Mutex::new(RegexSet::empty()));
        let alias_regex_set = Arc::new(Mutex::new(RegexSet::empty()));

        let mut me = Self {
            trigger_regex_set,
            alias_regex_set,
            triggers,
            aliases,
            script_eval_tx,
        };

        me.push_trigger(Trigger {
            name: "autoloot".into(),
            regex: Regex::new(r"is dead! R\.I\.P\.$").unwrap(),
            script: InterpretedAction::ProcessAlias(Arc::new(
                "exa corpse;get all.pile.coins corpse".into(),
            )),
        });

        me.push_alias(Alias {
            name: "order joy".into(),
            regex: Regex::new(r"^oj\s+(?<command>.*)$").unwrap(),

            script: Action::ProcessJavascriptAlias(me.get_precompiled_alias_from_script(
                r#"

                `order joy ${matches.command}`

                "#,
            )),
        });

        me.push_alias(Alias {
            name: "watch joy".into(),
            regex: Regex::new(r"^wj$").unwrap(),

            script: Action::ProcessJavascriptAlias(me.get_precompiled_alias_from_script(
                r#"

                ["watch", "joy"].join(' ')

                "#,
            )),
        });

        me.push_alias(Alias {
            name: "unlock/open".into(),
            regex: Regex::new(r"^unop\s+(.*)$").unwrap(),

            script: Action::ProcessJavascriptAlias(me.get_precompiled_alias_from_script(
                r#"

                `unlock ${matches.$1};open ${matches.$1}`

                "#,
            )),
        });

        me.push_alias(Alias {
            name: "do whatever".into(),
            regex: Regex::new(r"^/js (.*)$").unwrap(),

            script: Action::ProcessJavascriptAlias(me.get_precompiled_alias_from_script(
                r#"

                eval(matches.$1)

                "#,
            )),
        });

        me
    }

    fn push_trigger(&mut self, trigger: Trigger) {
        let mut triggers = self.triggers.lock().unwrap();
        triggers.push(trigger);
        drop(triggers);
        self.rebuild_trigger_regex_set();
    }

    fn push_alias(&mut self, alias: Alias) {
        let mut aliases = self.aliases.lock().unwrap();
        aliases.push(alias);
        drop(aliases);
        self.rebuild_alias_regex_set();
    }

    fn rebuild_trigger_regex_set(&mut self) {
        let triggers = self.triggers.lock().unwrap();
        let mut regex_set = self.trigger_regex_set.lock().unwrap();
        *regex_set = RegexSet::new(triggers.iter().map(|trigger| trigger.regex.as_str())).unwrap();
    }

    fn rebuild_alias_regex_set(&mut self) {
        let aliases = self.aliases.lock().unwrap();
        let mut regex_set = self.alias_regex_set.lock().unwrap();
        *regex_set = RegexSet::new(aliases.iter().map(|alias| alias.regex.as_str())).unwrap();
    }

    fn get_precompiled_alias_from_script(&self, source: &str) -> usize {
        let (tx, rx) = oneshot::channel();
        self.script_eval_tx
            .send(RuntimeAction::CompileJavascriptAlias(
                Arc::new(source.to_string()),
                Arc::new(tx),
            ))
            .unwrap();
        rx.blocking_recv().unwrap()
    }

    pub fn process_incoming_line(&self, line: Arc<StyledLine>) {
        let regex_set = self.trigger_regex_set.lock().unwrap();
        let matches: Vec<_> = regex_set.matches(line.as_str()).iter().collect();
        if matches.len() > 0 {
            let triggers = self.triggers.lock().unwrap();
            let to_send: Vec<InterpretedAction> = matches
                .iter()
                .map(|i| triggers.get(*i).unwrap().script.clone())
                .collect();
            self.script_eval_tx
                .send(RuntimeAction::EvalTriggerScripts(line, Arc::new(to_send)))
                .unwrap();
        } else {
            self.script_eval_tx
                .send(RuntimeAction::PassthroughCompleteLine(line))
                .unwrap();
        }
    }

    pub fn process_outgoing_line(&self, line: &str) {
        let regex_set = self.alias_regex_set.lock().unwrap();
        let matches: Vec<_> = regex_set.matches(line).iter().collect();
        if matches.len() > 0 {
            let aliases = self.aliases.lock().unwrap();
            let to_send: Vec<InterpretedAction> = matches
                .iter()
                .map(|i| match aliases.get(*i).unwrap() {
                    Alias {
                        name: _,
                        regex,
                        script: Action::ProcessJavascriptAlias(script),
                    } => {
                        let mut i = 0;
                        let captures: Arc<Vec<_>> = Arc::new(
                            regex
                                .capture_names()
                                .zip(regex.captures(line).unwrap().iter())
                                .map(|(k, v)| {
                                    let pair = (
                                        k.and_then(|k| Some(k.to_string()))
                                            .unwrap_or_else(|| format!("${i}")),
                                        v.and_then(|v| Some(v.as_str())).unwrap_or("").to_string(),
                                    );
                                    i += 1;
                                    pair
                                })
                                .collect(),
                        );
                        InterpretedAction::ProcessJavascriptAlias(*script, captures)
                    }
                    Alias {
                        name: _,
                        regex: _,
                        script: Action::ProcessAlias(script),
                    } => InterpretedAction::ProcessAlias(script.clone()),
                    Alias {
                        name: _,
                        regex: _,
                        script: Action::SendRaw(script),
                    } => InterpretedAction::SendRaw(script.clone()),
                    Alias {
                        name: _,
                        regex: _,
                        script: Action::Noop,
                    } => InterpretedAction::Noop,
                })
                .collect();
            self.script_eval_tx
                .send(RuntimeAction::EvalAliasScripts(
                    Arc::new(String::from(line)),
                    Arc::new(to_send),
                    0,
                ))
                .unwrap();
        } else {
            self.script_eval_tx
                .send(RuntimeAction::StringLiteralCommand(Arc::new(String::from(
                    line,
                ))))
                .unwrap();
        }
    }

    pub fn process_partial_line(&self, line: Arc<StyledLine>) {
        //TODO: support partial line/prompt triggers
        self.script_eval_tx
            .send(RuntimeAction::PassthroughPartialLine(line))
            .unwrap();
    }

    pub fn request_repaint(&self) {
        self.script_eval_tx
            .send(RuntimeAction::RequestRepaint)
            .unwrap();
    }
}

#[derive(Debug)]
pub struct Trigger {
    pub name: String,
    pub regex: Regex,
    pub script: InterpretedAction,
}

impl Trigger {
    pub fn new(name: String, regex: Regex, script: InterpretedAction) -> Self {
        Self {
            name,
            regex,
            script,
        }
    }
}

#[derive(Debug)]
pub struct Alias {
    name: String,
    regex: Regex,
    script: Action,
}

impl Alias {
    pub fn new(name: String, regex: Regex, script: Action) -> Self {
        Self {
            name,
            regex,
            script,
        }
    }
}
