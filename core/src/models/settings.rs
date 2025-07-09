use crate::get_smudgy_home;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, io};
/// Represents the global application settings.
///
/// Loaded from / saved to `settings.json` in the main smudgy config directory.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The api key for smudgy.org
    pub api_key: Option<String>,
    /// The maximum number of lines to keep in the scrollback buffer.
    #[serde(default = "default_scrollback_length")]
    pub scrollback_length: usize,
}

/// Helper for serde default scrollback length.
fn default_scrollback_length() -> usize {
    100_000
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            api_key: None,
            scrollback_length: default_scrollback_length(),
        }
    }
}

/// Loads the global application settings from `settings.json`.
///
/// If the file does not exist or cannot be parsed, returns the default settings.
/// Errors during file reading (other than not found) or parsing are logged.
///
/// # Returns
///
/// The loaded `Settings` or `Settings::default()`.
#[must_use]
pub fn load_settings() -> Settings {
    match try_load_settings() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Warning: Failed to load settings, using defaults: {e}");
            Settings::default()
        }
    }
}

/// Internal helper function to attempt loading settings and return a Result.
///
/// # Errors
///
/// This function can return an error in the following cases:
/// - If the smudgy home directory cannot be determined (e.g., `dirs::home_dir()` is `None`).
/// - If reading `settings.json` fails for reasons other than the file not being found (e.g., permission issues).
/// - If parsing the content of `settings.json` fails (e.g., invalid JSON format).
fn try_load_settings() -> Result<Settings> {
    let smudgy_dir = get_smudgy_home()?;
    let settings_path = smudgy_dir.join("settings.json");

    match fs::read_to_string(&settings_path) {
        Ok(content) => {
            let settings: Settings =
                serde_json::from_str(&content).context("Failed to parse settings.json")?;
            Ok(settings)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // File not found is expected on first run, return default.
            Ok(Settings::default())
        }
        Err(e) => {
            // Other read errors are propagated
            Err(e).context(format!(
                "Failed to read settings.json at {}",
                settings_path.display()
            ))
        }
    }
}

/// Saves the global application settings to `settings.json`.
///
/// This will overwrite the existing file.
///
/// # Arguments
///
/// * `settings` - The `Settings` struct to save.
///
/// # Errors
///
/// Returns an error if:
/// - The smudgy home directory cannot be determined.
/// - The settings cannot be serialized to JSON.
/// - The `settings.json` file cannot be written to disk (e.g., permission issues, disk full).
pub fn save_settings(settings: &Settings) -> Result<()> {
    let smudgy_dir = get_smudgy_home()?;
    let settings_path = smudgy_dir.join("settings.json");

    let json_content =
        serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;

    fs::write(&settings_path, json_content).context(format!(
        "Failed to write settings.json at {}",
        settings_path.display()
    ))?;

    Ok(())
}
