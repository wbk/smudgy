use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
    vec,
};

use anyhow::{bail, Result};
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
    EvalJavascript(usize),
}

#[derive(Debug)]
pub struct TriggerManager {
    trigger_regex_set: RegexSet,
    alias_regex_set: RegexSet,
    triggers: Vec<Trigger>,
    aliases: Vec<Alias>,
    script_eval_tx: UnboundedSender<RuntimeAction>,
}

fn line_splitter(ch: char) -> bool {
    ch == ';' || ch == '\n'
}

impl TriggerManager {
    pub fn new(script_eval_tx: UnboundedSender<RuntimeAction>) -> Self {
        let triggers = Vec::new();
        let aliases = Vec::new();
        let trigger_regex_set = RegexSet::empty();
        let alias_regex_set = RegexSet::empty();

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
            script: Action::ProcessAlias(Arc::new(
                "exa corpse;get all.pile.coins corpse".into(),
            )),
        });

        me.push_alias(Alias {
            name: "order joy".into(),
            regex: Regex::new(r"^oj\s+(?<command>.*)$").unwrap(),

            script: Action::EvalJavascript(me.get_precompiled_alias_from_script(
                r#"

                `order joy ${matches.command}`

                "#,
            )),
        });

        me.push_alias(Alias {
            name: "watch joy".into(),
            regex: Regex::new(r"^wj$").unwrap(),

            script: Action::EvalJavascript(me.get_precompiled_alias_from_script(
                r#"

                ["watch", "joy"].join(' ')

                "#,
            )),
        });

        me.push_alias(Alias {
            name: "unlock/open".into(),
            regex: Regex::new(r"^unop\s+(.*)$").unwrap(),

            script: Action::EvalJavascript(me.get_precompiled_alias_from_script(
                r#"

                `unlock ${matches.$1};open ${matches.$1}`

                "#,
            )),
        });

        me.push_alias(Alias {
            name: "do whatever".into(),
            regex: Regex::new(r"^/js (.*)$").unwrap(),

            script: Action::EvalJavascript(me.get_precompiled_alias_from_script(
                r#"

                eval(matches.$1)

                "#,
            )),
        });

        me
    }

    fn push_trigger(&mut self, trigger: Trigger) {
        self.triggers.push(trigger);
        self.rebuild_trigger_regex_set();
    }

    fn push_alias(&mut self, alias: Alias) {
        self.aliases.push(alias);
        self.rebuild_alias_regex_set();
    }

    fn rebuild_trigger_regex_set(&mut self) {
        self.trigger_regex_set = RegexSet::new(self.triggers.iter().map(|trigger| trigger.regex.as_str())).unwrap();
    }

    fn rebuild_alias_regex_set(&mut self) {
        self.alias_regex_set = RegexSet::new(self.aliases.iter().map(|alias| alias.regex.as_str())).unwrap();
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
        let regex_set = &self.trigger_regex_set;
        let matches: Vec<_> = regex_set.matches(line.as_str()).iter().collect();
        if matches.len() > 0 {
            let triggers = &self.triggers;
            for trigger_idx in matches {
                match triggers.get(trigger_idx).unwrap().script {
                    Action::Noop => {}
                    Action::SendRaw(ref str) => {
                        self.script_eval_tx.send(RuntimeAction::SendRaw(str.clone())).unwrap();
                    }
                    Action::ProcessAlias(ref str) => {
                        self.process_outgoing_line(str.as_str());
                    }
                    Action::EvalJavascript(_script_id) => {
                        unimplemented!()
                    }
                }
            }
        } else {
            self.script_eval_tx
                .send(RuntimeAction::PassthroughCompleteLine(line))
                .unwrap();
        }
    }

    #[inline(always)]
    fn process_outgoing_line_inner(&self, line: &str, depth: u32) -> Result<()> {
        if depth > 100 {
            bail!("Alias processor bailing, depth limit reached. Do you have an alias that triggers itself?");
        }
        // Technically an outgoing line can be split into multiple lines, separated by newlines or ';' characters so we need to process each one
        for line in line.split(line_splitter) {
            let line_arc = Arc::new(line.to_string());

            let matches: Vec<_> = self.alias_regex_set.matches(line).iter().collect();
            if matches.len() > 0 {
                let aliases = &self.aliases;
                for match_idx in matches {
                    match aliases.get(match_idx).unwrap() {
                        Alias {
                            name: _,
                            regex,
                            script: Action::EvalJavascript(script),
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
                                            v.and_then(|v| Some(v.as_str()))
                                                .unwrap_or("")
                                                .to_string(),
                                        );
                                        i += 1;
                                        pair
                                    })
                                    .collect(),
                            );
                            let (tx, rx) = oneshot::channel();
                            self.script_eval_tx.send(RuntimeAction::EvalJavascriptAlias(
                                line_arc.clone(),
                                    *script,
                                    captures,
                                    Arc::new(tx),
                            ))?;
                            rx.blocking_recv().map(|response| {
                                response.map(|line| {
                                    self.process_outgoing_line_inner(line.as_str(), depth + 1)
                                })
                            })?;
                        }
                        Alias {
                            name: _,
                            regex: _,
                            script: Action::ProcessAlias(script),
                        } => self.process_outgoing_line_inner(script.as_str(), depth + 1)?,
                        Alias {
                            name: _,
                            regex: _,
                            script: Action::SendRaw(script),
                        } => self
                            .script_eval_tx
                            .send(RuntimeAction::SendRaw(script.clone()))?,
                        Alias {
                            name: _,
                            regex: _,
                            script: Action::Noop,
                        } => {}
                    }
                }
            } else {
                self.script_eval_tx
                    .send(RuntimeAction::SendRaw(Arc::new(String::from(
                        line,
                    ))))?;
            }
        }
        Ok(())
    }

    pub fn process_outgoing_line(&self, line: &str) {
        self.process_outgoing_line_inner(line, 0).unwrap();
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
    pub script: Action,
}

impl Trigger {
    pub fn new(name: String, regex: Regex, script: Action) -> Self {
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
