use std::{collections::HashMap, sync::Arc, time::Instant};

use anyhow::{Context, Result, bail};
use regex::{Regex, RegexSet, RegexSetBuilder};
use tokio::sync::mpsc::UnboundedSender;

use crate::session::{runtime::{script_engine::{FunctionId, ScriptId}, RuntimeAction}, styled_line::StyledLine};
use super::ScriptAction;

// limit regex size to 512MB
const MAX_REGEX_SIZE: usize = 512 * 1024 * 1024;

#[derive(Debug)]
pub struct Manager {
    session_runtime_tx: UnboundedSender<RuntimeAction>,
    triggers: Vec<Trigger>,
    aliases: Vec<Trigger>,
    trigger_regex_set_map: Vec<usize>, // Maps index in RegexSet to index in triggers
    trigger_regex_patterns_map: Vec<usize>,
    trigger_regex_set: RegexSet,
    raw_trigger_regex_set_map: Vec<usize>,
    raw_trigger_regex_patterns_map: Vec<usize>,
    raw_trigger_regex_set: RegexSet,
    prompt_trigger_regex_set_map: Vec<usize>,
    prompt_trigger_regex_patterns_map: Vec<usize>,
    prompt_trigger_regex_set: RegexSet,
    prompt_raw_trigger_regex_set_map: Vec<usize>,
    prompt_raw_trigger_regex_patterns_map: Vec<usize>,
    prompt_raw_trigger_regex_set: RegexSet,
    alias_regex_set_map: Vec<usize>,
    alias_regex_patterns_map: Vec<usize>,
    alias_regex_set: RegexSet,
    trigger_indices: HashMap<String, usize>,
    alias_indices: HashMap<String, usize>,
    trigger_regex_set_dirty: bool,
}

pub fn line_splitter(ch: char) -> bool {
    ch == ';' || ch == '\n'
}

#[derive(Clone, Copy)]
enum TriggerMatchType {
    Normal,
    Raw,
}

pub struct PushTriggerParams<'a> {
    pub name: &'a Arc<String>,
    pub patterns: &'a Arc<Vec<String>>,
    pub raw_patterns: &'a Arc<Vec<String>>,
    pub anti_patterns: &'a Arc<Vec<String>>,
    pub action: ScriptAction,
    pub prompt: bool,
    pub enabled: bool,
}

impl Manager {
    pub fn new(session_runtime_tx: UnboundedSender<RuntimeAction>) -> Self {
        let triggers = Vec::new();
        let aliases = Vec::new();
        let trigger_indices = HashMap::new();
        let alias_indices = HashMap::new();
        let trigger_regex_set = RegexSet::empty();
        let raw_trigger_regex_set = RegexSet::empty();
        let prompt_trigger_regex_set = RegexSet::empty();
        let prompt_raw_trigger_regex_set = RegexSet::empty();
        let alias_regex_set = RegexSet::empty();

        Self {
            alias_regex_set,
            trigger_regex_set,
            raw_trigger_regex_set,
            prompt_trigger_regex_set,
            prompt_raw_trigger_regex_set,
            alias_regex_set_map: Vec::new(),
            trigger_regex_set_map: Vec::new(),
            raw_trigger_regex_set_map: Vec::new(),
            prompt_trigger_regex_set_map: Vec::new(),
            prompt_raw_trigger_regex_set_map: Vec::new(),
            alias_regex_patterns_map: Vec::new(),
            trigger_regex_patterns_map: Vec::new(),
            raw_trigger_regex_patterns_map: Vec::new(),
            prompt_trigger_regex_patterns_map: Vec::new(),
            prompt_raw_trigger_regex_patterns_map: Vec::new(),
            aliases,
            triggers,
            alias_indices,
            trigger_indices,
            session_runtime_tx,
            trigger_regex_set_dirty: true,
        }
    }

    fn add_or_update_alias(&mut self, alias: Trigger) {
        debug!(
            "Adding or updating alias: {:?}, {:?}",
            alias.name, alias.patterns
        );
        if let Some(index) = self.alias_indices.get(&alias.name) {
            *self.aliases.get_mut(*index).unwrap() = alias;
        } else {
            self.alias_indices
                .insert(alias.name.clone(), self.aliases.len());
            self.aliases.push(alias);
        }
        self.rebuild_alias_regex_set();
    }

    fn add_or_update_trigger(&mut self, trigger: Trigger) {
        trace!(
            "Adding or updating trigger: {:?}, {:?}",
            trigger.name, trigger.patterns
        );
        if let Some(index) = self.trigger_indices.get(&trigger.name) {
            *self.triggers.get_mut(*index).unwrap() = trigger;
        } else {
            self.trigger_indices
                .insert(trigger.name.clone(), self.triggers.len());
            self.triggers.push(trigger);
        }

        self.trigger_regex_set_dirty = true;
    }

    pub fn push_javascript_alias(
        &mut self,
        name: &Arc<String>,
        patterns: &Arc<Vec<String>>,
        script_id: ScriptId,
    ) -> Result<()> {
        self.add_or_update_alias(Trigger::new_alias(
            name.to_string(),
            patterns.iter(),
            ScriptAction::EvalJavascript(script_id),
        )?);
        Ok(())
    }

    pub fn push_trigger(&mut self, params: PushTriggerParams) -> Result<()> {
        self.add_or_update_trigger(Trigger::new(
            params.name.to_string(),
            params.patterns.iter(),
            params.raw_patterns.iter(),
            params.anti_patterns.iter(),
            params.action,
            params.prompt,
            params.enabled,
        )?);
        Ok(())
    }

    pub fn push_javascript_function_alias(
        &mut self,
        name: Arc<String>,
        patterns: Arc<Vec<String>>,
        function_id: FunctionId,
    ) -> Result<()> {
        self.add_or_update_alias(Trigger::new_alias(
            name.to_string(),
            patterns.iter(),
            ScriptAction::CallJavascriptFunction(function_id),
        )?);
        Ok(())
    }

    pub fn push_simple_alias(
        &mut self,
        name: Arc<String>,
        patterns: Arc<Vec<String>>,
        script: Arc<String>,
    ) -> Result<()> {
        self.add_or_update_alias(Trigger::new_alias(
            name.to_string(),
            patterns.iter(),
            ScriptAction::SendSimple(script),
        )?);
        Ok(())
    }

    pub fn enable_alias(&mut self, name: &str, enabled: bool) {
        if let Some(index) = self.alias_indices.get(name) {
            if let Some(alias) = self.aliases.get_mut(*index) {
                trace!(
                    "{} alias: {:?}, {:?}",
                    if enabled { "Enabling" } else { "Disabling" },
                    alias.name,
                    alias.patterns
                );
                alias.enabled = enabled;
            }
        }
    }

    pub fn enable_trigger(&mut self, name: &str, enabled: bool) {
        if let Some(index) = self.trigger_indices.get(name) {
            if let Some(trigger) = self.triggers.get_mut(*index) {
                trace!(
                    "{} trigger: {:?}, {:?}",
                    if enabled { "Enabling" } else { "Disabling" },
                    trigger.name,
                    trigger.patterns
                );
                trigger.enabled = enabled;
            }
        }
    }

    ///
    /// Builds regex sets for triggers, raw triggers, prompt triggers, and raw prompt triggers
    ///
    /// This could be heavily DRY-ed up, but it just needs to create, for each type of trigger:
    ///  - a `RegexSet` to test when that type of trigger is being tested
    ///  - a `Vec<usize>` to map the indices of the `RegexSet` to the indices of the triggers
    ///  - a `Vec<usize>` to map the indices of the `RegexSet` to the indices of the patterns
    fn rebuild_trigger_regex_set(&mut self) {
        let start = std::time::Instant::now();

        self.trigger_regex_set = RegexSetBuilder::new(
            self.triggers
                .iter()
                .flat_map(|trigger| trigger.patterns.iter().map(regex::Regex::as_str)),
        )
        .size_limit(MAX_REGEX_SIZE)
        .build()
        .unwrap();

        self.trigger_regex_set_map = self
            .triggers
            .iter()
            .enumerate()
            .flat_map(|(i, trigger)| {
                let mut v = Vec::with_capacity(trigger.patterns.len());
                for _ in 0..trigger.patterns.len() {
                    v.push(i);
                }
                v
            })
            .collect();
        self.trigger_regex_patterns_map = self
            .triggers
            .iter()
            .flat_map(|trigger| trigger.patterns.iter().enumerate().map(|(i, _pattern)| i))
            .collect();

        self.raw_trigger_regex_set = RegexSetBuilder::new(
            self.triggers
                .iter()
                .flat_map(|trigger| trigger.raw_patterns.iter().map(regex::Regex::as_str)),
        )
        .size_limit(MAX_REGEX_SIZE)
        .build()
        .unwrap();
        self.raw_trigger_regex_set_map = self
            .triggers
            .iter()
            .enumerate()
            .flat_map(|(i, trigger)| {
                let mut v = Vec::with_capacity(trigger.raw_patterns.len());
                for _ in 0..trigger.raw_patterns.len() {
                    v.push(i);
                }
                v
            })
            .collect();
        self.raw_trigger_regex_patterns_map = self
            .triggers
            .iter()
            .flat_map(|trigger| {
                trigger
                    .raw_patterns
                    .iter()
                    .enumerate()
                    .map(|(i, _pattern)| i)
            })
            .collect();

        self.prompt_trigger_regex_set = RegexSetBuilder::new(
            self.triggers
                .iter()
                .filter(|t| t.fire_on_prompts())
                .flat_map(|trigger| trigger.patterns.iter().map(regex::Regex::as_str)),
        )
        .size_limit(MAX_REGEX_SIZE)
        .build()
        .unwrap();
        self.prompt_trigger_regex_set_map = self
            .triggers
            .iter()
            .enumerate()
            .filter(|(_, t)| t.fire_on_prompts())
            .flat_map(|(i, trigger)| {
                let mut v = Vec::with_capacity(trigger.patterns.len());
                for _ in 0..trigger.patterns.len() {
                    v.push(i);
                }
                v
            })
            .collect();
        self.prompt_trigger_regex_patterns_map = self
            .triggers
            .iter()
            .filter(|t| t.fire_on_prompts())
            .flat_map(|trigger| trigger.patterns.iter().enumerate().map(|(i, _pattern)| i))
            .collect();

        self.prompt_raw_trigger_regex_set = RegexSetBuilder::new(
            self.triggers
                .iter()
                .filter(|t| t.fire_on_prompts())
                .flat_map(|trigger| trigger.raw_patterns.iter().map(regex::Regex::as_str)),
        )
        .size_limit(MAX_REGEX_SIZE)
        .build()
        .unwrap();
        self.prompt_raw_trigger_regex_set_map = self
            .triggers
            .iter()
            .enumerate()
            .filter(|(_, t)| t.fire_on_prompts())
            .flat_map(|(i, trigger)| {
                let mut v = Vec::with_capacity(trigger.raw_patterns.len());
                for _ in 0..trigger.raw_patterns.len() {
                    v.push(i);
                }
                v
            })
            .collect();
        self.prompt_raw_trigger_regex_patterns_map = self
            .triggers
            .iter()
            .filter(|t| t.fire_on_prompts())
            .flat_map(|trigger| {
                trigger
                    .raw_patterns
                    .iter()
                    .enumerate()
                    .map(|(i, _pattern)| i)
            })
            .collect();

        debug!("Time to rebuild trigger regex sets: {:?}", start.elapsed());
    }

    fn rebuild_alias_regex_set(&mut self) {
        self.alias_regex_set = RegexSetBuilder::new(
            self.aliases
                .iter()
                .flat_map(|alias| alias.patterns.iter().map(|pattern| pattern.as_str())),
        )
        .size_limit(MAX_REGEX_SIZE)
        .build()
        .unwrap();
        self.alias_regex_set_map = self
            .aliases
            .iter()
            .enumerate()
            .flat_map(|(i, alias)| {
                let mut v = Vec::with_capacity(alias.patterns.len());
                for _ in 0..alias.patterns.len() {
                    v.push(i);
                }
                v
            })
            .collect();
        self.alias_regex_patterns_map = self
            .aliases
            .iter()
            .flat_map(|alias| alias.patterns.iter().enumerate().map(|(i, _pattern)| i))
            .collect();
    }

    #[allow(clippy::too_many_arguments)]
    fn process_line_inner(
        &self,
        line: &str,
        depth: u32,
        regex_set: &RegexSet,
        triggers: &[Trigger],
        regex_set_to_triggers_map: &[usize],
        regex_set_to_patterns_map: &[usize],
        match_type: TriggerMatchType,
    ) -> Result<bool> {
        if depth > 100 {
            bail!(
                "Script processor bailing, depth limit reached. Do you have an alias that triggers itself?"
            );
        }
        let start = std::time::Instant::now();
        let matches = regex_set.matches(line);
        debug!("Time to test regex matches: {:?}", start.elapsed());

        let start = std::time::Instant::now();
        let matches: Vec<_> = matches.iter().collect();
        debug!("Time to collect regex matches: {:?}", start.elapsed());
        let mut fired = false;
        if !matches.is_empty() {
            for match_indices in matches.chunk_by(|a, b| {
                regex_set_to_triggers_map.get(*a).unwrap()
                    == regex_set_to_triggers_map.get(*b).unwrap()
            }) {
                let match_idx = match_indices[0];
                let trigger = triggers
                    .get(*regex_set_to_triggers_map.get(match_idx).unwrap())
                    .unwrap();

                if !trigger.enabled || trigger.anti_patterns.is_match(line) {
                    continue;
                }

                debug!(
                    "Trigger matched: {:?}, /{}/",
                    trigger.name(),
                    regex_set.patterns().get(match_idx).unwrap()
                );

                let pattern_idx = *regex_set_to_patterns_map.get(match_idx).unwrap();
                if let Some(lines) = trigger.run(
                    line,
                    match_type,
                    pattern_idx,
                    &self.session_runtime_tx,
                    depth + 1,
                )? {
                    for line in lines {
                        self.process_nested_outgoing_line(&line, depth + 1)?;
                    }
                }

                fired = true;
            }
        }
        Ok(fired)
    }

    pub fn process_outgoing_line(&self, line: &str) -> Result<()> {
        self.process_nested_outgoing_line(line, 0)
    }

    pub fn process_nested_outgoing_line(&self, line: &str, depth: u32) -> Result<()> {
        if !self.process_line_inner(
            line,
            depth,
            &self.alias_regex_set,
            &self.aliases,
            &self.alias_regex_set_map,
            &self.alias_regex_patterns_map,
            TriggerMatchType::Normal,
        )? {
            self.session_runtime_tx
                .send(RuntimeAction::SendRaw(Arc::new(line.to_string())))
                .context("Could not send outgoing line to runtime")?;
        }
        Ok(())
    }

    pub fn process_incoming_line(&mut self, line: Arc<StyledLine>) -> Result<()> {
        trace!("Processing incoming line: {line:?}");
        if self.trigger_regex_set_dirty {
            self.rebuild_trigger_regex_set();
            self.trigger_regex_set_dirty = false;
        }

        let start = Instant::now();

        if let Some(line) = line.raw() {
            debug!("Processing raw line: {line:?}");
            self.process_line_inner(
                line,
                0,
                &self.raw_trigger_regex_set,
                &self.triggers,
                &self.raw_trigger_regex_set_map,
                &self.raw_trigger_regex_patterns_map,
                TriggerMatchType::Raw,
            )?;
        }

        self.process_line_inner(
            &line,
            0,
            &self.trigger_regex_set,
            &self.triggers,
            &self.trigger_regex_set_map,
            &self.trigger_regex_patterns_map,
            TriggerMatchType::Normal,
        )?;

        let end = start.elapsed();
        debug!("Time to match and dispatch triggers on incoming line: {end:?}");

        self.session_runtime_tx
            .send(RuntimeAction::CompleteLineTriggersProcessed(line))
            .context("Could not send incoming line to runtime")?;
        Ok(())
    }

    pub fn process_partial_line(&self, line: Arc<StyledLine>) -> Result<()> {
        trace!("Processing incoming partial line: {line:?}");
        let start = Instant::now();

        if let Some(line) = line.raw() {
            self.process_line_inner(
                line,
                0,
                &self.prompt_raw_trigger_regex_set,
                &self.triggers,
                &self.prompt_raw_trigger_regex_set_map,
                &self.prompt_raw_trigger_regex_patterns_map,
                TriggerMatchType::Raw,
            )?;
        }

        self.process_line_inner(
            &line,
            0,
            &self.prompt_trigger_regex_set,
            &self.triggers,
            &self.prompt_trigger_regex_set_map,
            &self.prompt_trigger_regex_patterns_map,
            TriggerMatchType::Normal,
        )?;

        let end = start.elapsed();
        debug!("Time to match and dispatch triggers on incoming partial line: {end:?}");

        self.session_runtime_tx
            .send(RuntimeAction::PartialLineTriggersProcessed(line))
            .context("Could not send incoming line to runtime")?;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.triggers.clear();
        self.trigger_indices.clear();
        self.aliases.clear();
        self.alias_indices.clear();
        self.rebuild_trigger_regex_set();
        self.rebuild_alias_regex_set();
    }
}

#[derive(Debug)]
struct Trigger {
    name: String,
    patterns: Vec<Regex>,
    raw_patterns: Vec<Regex>,
    anti_patterns: RegexSet,
    script: ScriptAction,
    prompt: bool,
    enabled: bool,
}

impl Trigger {
    pub fn new<
        TIterPattern,
        TIterRawPattern,
        TIterAntiPattern,
        TPatternStr,
        TRawPatternStr,
        TAntiPatternStr,
    >(
        name: String,
        patterns: TIterPattern,
        raw_patterns: TIterRawPattern,
        anti_patterns: TIterAntiPattern,
        script: ScriptAction,
        prompt: bool,
        enabled: bool,
    ) -> Result<Self>
    where
        TPatternStr: AsRef<str>,
        TRawPatternStr: AsRef<str>,
        TAntiPatternStr: AsRef<str>,
        TIterPattern: Iterator<Item = TPatternStr>,
        TIterRawPattern: Iterator<Item = TRawPatternStr>,
        TIterAntiPattern: Iterator<Item = TAntiPatternStr>,
    {
        let patterns: Vec<_> = patterns
            .map(|pattern| Regex::new(pattern.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        let raw_patterns: Vec<_> = raw_patterns
            .map(|pattern| Regex::new(pattern.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        let anti_patterns = RegexSet::new(anti_patterns)?;

        Ok(Self {
            name,
            patterns,
            raw_patterns,
            anti_patterns,
            script,
            prompt,
            enabled,
        })
    }

    pub fn new_alias<TIterPattern, TPatternStr>(
        name: String,
        patterns: TIterPattern,
        script: ScriptAction,
    ) -> Result<Self>
    where
        TPatternStr: AsRef<str>,
        TIterPattern: Iterator<Item = TPatternStr>,
    {
        Self::new(
            name,
            patterns,
            std::iter::empty::<&str>(),
            std::iter::empty::<&str>(),
            script,
            false,
            true,
        )
    }
    #[allow(unreachable_code, unused_variables)]
    pub fn run(
        &self,
        line: &str,
        match_type: TriggerMatchType,
        pattern_idx: usize,
        runtime_action_tx: &UnboundedSender<RuntimeAction>,
        depth: u32,
    ) -> Result<Option<Vec<String>>> {
        let pattern = match match_type {
            TriggerMatchType::Normal => self.patterns.get(pattern_idx).unwrap(),
            TriggerMatchType::Raw => self.raw_patterns.get(pattern_idx).unwrap(),
        };
        let mut i = 0;
        let captures: Arc<Vec<_>> = Arc::new(
            pattern
                .capture_names()
                .zip(pattern.captures(line).unwrap().iter())
                .map(|(k, v)| {
                    let pair = (
                        k.map_or_else(|| format!("${i}"), ToString::to_string),
                        v.map_or_else(String::new, |m| m.as_str().to_string()),
                    );
                    i += 1;
                    pair
                })
                .collect(),
        );

        match self.script {
            ScriptAction::EvalJavascript(script_id) => {
                runtime_action_tx.send(RuntimeAction::EvalJavascript {
                    id: script_id,
                    depth,
                    matches: captures,
                })?;

                Ok(None)
           }
            ScriptAction::CallJavascriptFunction(function_id) => {
                runtime_action_tx.send(RuntimeAction::CallJavascriptFunction {
                    id: function_id,
                    depth,
                    matches: captures
                })?;

                Ok(None)
            }
            ScriptAction::SendSimple(ref script) => {
                let mut evaluated = String::clone(script);
                captures.iter().for_each(|(k, v)| {
                    evaluated = evaluated.replace(k, v);
                });

                Ok(Some(
                    evaluated
                        .split(line_splitter)
                        .map(ToString::to_string)
                        .collect(),
                ))
            }
            ScriptAction::SendRaw(ref script) => {
                runtime_action_tx.send(RuntimeAction::SendRaw(script.clone()))?;
                Ok(None)
            }
            ScriptAction::Noop => Ok(None),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fire_on_prompts(&self) -> bool {
        self.prompt
    }
}
