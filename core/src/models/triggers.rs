use serde::{Deserialize, Serialize};
use crate::get_smudgy_home;
use anyhow::{Context, Result};
use std::{collections::HashMap, fs, io, path::PathBuf};
use crate::models::packages::PackageTree;

// Helper function for serde to default boolean fields to true.
fn default_true() -> bool {
    true
}

// Represents the programming language of a script.
// TODO: Consolidate with other models if identical.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptLang { JS, TS }

/// Represents the definition of a single trigger.
///
/// This structure is used as the value in the map representation
/// of the `triggers.json` file.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)] // Default needed for serde defaults
pub struct TriggerDefinition {
    /// Optional single regex pattern to match against incoming server text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Optional list of regex patterns to match against incoming server text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patterns: Option<Vec<String>>,
    /// Optional single raw/literal string to match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_pattern: Option<String>,
    /// Optional list of raw/literal strings to match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_patterns: Option<Vec<String>>,
    /// Optional single regex pattern that must *not* match for the trigger to fire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anti_pattern: Option<String>,
    /// Optional list of regex patterns that must *not* match for the trigger to fire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anti_patterns: Option<Vec<String>>,

    /// Stores inline script content. If None during load, implies script is file-based.
    /// If None during save, this field is omitted from the JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>,
    /// Optional package path (e.g., "combat/healing"). `None` indicates root package.
    #[serde(default)]
    pub package: Option<String>,
    /// Whether this specific trigger is enabled. Defaults to true.
    #[serde(default = "default_true")]
    pub enabled: bool,

    // TODO: Add other trigger-specific fields like sequence, sound file, highlighting, etc.
}

impl TriggerDefinition {
    /// Checks if the trigger is effectively enabled.
    #[must_use] pub fn is_effectively_enabled(&self, package_tree: &PackageTree) -> bool {
        if !self.enabled {
            return false;
        }
        match &self.package {
            None => true,
            Some(path_str) => {
                super::packages::is_package_effectively_enabled(path_str, package_tree)
            }
        }
    }

    /// Attempts to retrieve the script content and its language.
    pub fn get_script_content(&self, trigger_name: &str, server_name: &str) -> Result<Option<(String, ScriptLang)>> {
        if let Some(inline_content) = &self.script {
            // TODO: Determine language of inline scripts. Assume JS for now.
            return Ok(Some((inline_content.clone(), ScriptLang::JS)));
        }

        let base_path = get_smudgy_home()?.join(server_name).join("triggers");

        let ts_path = base_path.join(format!("{trigger_name}.ts"));
        if ts_path.exists() {
            match fs::read_to_string(&ts_path) {
                Ok(content) => return Ok(Some((content, ScriptLang::TS))),
                Err(e) => return Err(anyhow::Error::from(e).context(format!("Failed to read script file: {ts_path:?}"))),
            }
        }

        let js_path = base_path.join(format!("{trigger_name}.js"));
        if js_path.exists() {
            match fs::read_to_string(&js_path) {
                Ok(content) => return Ok(Some((content, ScriptLang::JS))),
                Err(e) => return Err(anyhow::Error::from(e).context(format!("Failed to read script file: {js_path:?}"))),
            }
        }
        Ok(None)
    }

    /// Gets the expected filesystem path for a file-based script.
    pub fn get_expected_script_path(&self, trigger_name: &str, server_name: &str, lang: ScriptLang) -> Result<PathBuf> {
        let ext = match lang {
            ScriptLang::JS => "js",
            ScriptLang::TS => "ts",
        };
        Ok(get_smudgy_home()?
            .join(server_name)
            .join("triggers") // Use triggers subdir
            .join(format!("{trigger_name}.{ext}")))
    }

    /// Checks if the trigger definition has any pattern specified.
    #[must_use] pub fn has_patterns(&self) -> bool {
        self.pattern.is_some()
            || self.patterns.is_some()
            || self.raw_pattern.is_some()
            || self.raw_patterns.is_some()
            || self.anti_pattern.is_some()
            || self.anti_patterns.is_some()
    }

}

/// Loads all trigger definitions from `triggers.json` for a given server.
///
/// If `triggers.json` does not exist within the server's `triggers` directory,
/// returns an empty `HashMap` successfully.
///
/// # Arguments
///
/// * `server_name` - The name of the server whose triggers should be loaded.
///
/// # Errors
///
/// Returns an error if the server or triggers directory cannot be accessed, or if
/// `triggers.json` exists but cannot be read or parsed.
pub fn load_triggers(server_name: &str) -> Result<HashMap<String, TriggerDefinition>> {
    let smudgy_dir = get_smudgy_home()?;
    let triggers_path = smudgy_dir
        .join(server_name)
        .join("triggers") // Use triggers subdir
        .join("triggers.json");

    match fs::read_to_string(&triggers_path) {
        Ok(content) => {
            let triggers: HashMap<String, TriggerDefinition> = serde_json::from_str(&content)
                .context(format!(
                    "Failed to parse triggers.json for server '{server_name}'"
                ))?;
            Ok(triggers)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // File not found is okay, just return an empty map
            Ok(HashMap::new())
        }
        Err(e) => {
            // Other read errors are propagated
            Err(e).context(format!(
                "Failed to read triggers.json for server '{server_name}'"
            ))
        }
    }
}

/// Saves the trigger definitions map to `triggers.json` for a given server.
///
/// This will overwrite the existing file if it exists. It assumes the
/// parent `triggers` directory already exists.
///
/// # Arguments
///
/// * `server_name` - The name of the server whose triggers should be saved.
/// * `triggers` - The `HashMap<String, TriggerDefinition>` data structure to save.
///
/// # Errors
///
/// Returns an error if the server or triggers directory cannot be accessed, or if
/// `triggers.json` cannot be written.
pub fn save_triggers(
    server_name: &str,
    triggers: &HashMap<String, TriggerDefinition>,
) -> Result<()> {
    let smudgy_dir = get_smudgy_home()?;
    let triggers_dir = smudgy_dir.join(server_name).join("triggers");

    // Basic check to ensure the triggers directory exists.
    if !triggers_dir.is_dir() {
        return Err(anyhow::anyhow!(
            "Triggers directory not found for server '{}': {:?}",
            server_name,
            triggers_dir
        ));
    }

    let triggers_path = triggers_dir.join("triggers.json");

    let json_content = serde_json::to_string_pretty(triggers).context(format!(
        "Failed to serialize triggers for server '{server_name}'"
    ))?;

    fs::write(&triggers_path, json_content).context(format!(
        "Failed to write triggers.json for server '{server_name}' at {triggers_path:?}"
    ))?;

    Ok(())
} 