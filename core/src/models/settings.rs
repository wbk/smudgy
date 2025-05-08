use serde::{Deserialize, Serialize};
use crate::get_smudgy_home;
use anyhow::{Context, Result};
use std::{fs, io};

/// Represents the user's theme choice in a serializable way.
/// The actual `iced::Theme` is derived from this in the UI layer.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")] // Use "light", "dark", etc. in JSON
#[derive(Default)]
pub enum ThemeChoice {
    Light,
    #[default]
    Dark,
    TokyoNight,
}


/// Represents the global application settings.
///
/// Loaded from / saved to `settings.json` in the main smudgy config directory.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The selected UI theme.
    #[serde(default)]
    pub theme: ThemeChoice,
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
            theme: ThemeChoice::default(),
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
#[must_use] pub fn load_settings() -> Settings {
    match try_load_settings() {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("Warning: Failed to load settings, using defaults: {e}");
            Settings::default()
        }
    }
}

/// Internal helper function to attempt loading settings and return a Result.
fn try_load_settings() -> Result<Settings> {
    let smudgy_dir = get_smudgy_home()?;
    let settings_path = smudgy_dir.join("settings.json");

    match fs::read_to_string(&settings_path) {
        Ok(content) => {
            let settings: Settings = serde_json::from_str(&content)
                .context("Failed to parse settings.json")?;
            Ok(settings)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // File not found is expected on first run, return default.
            Ok(Settings::default())
        }
        Err(e) => {
            // Other read errors are propagated
            Err(e).context(format!("Failed to read settings.json at {settings_path:?}"))
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
/// Returns an error if the smudgy home directory cannot be determined or the file
/// cannot be written.
pub fn save_settings(settings: &Settings) -> Result<()> {
    let smudgy_dir = get_smudgy_home()?;
    let settings_path = smudgy_dir.join("settings.json");

    let json_content = serde_json::to_string_pretty(settings)
        .context("Failed to serialize settings")?;

    fs::write(&settings_path, json_content).context(format!(
        "Failed to write settings.json at {settings_path:?}"
    ))?;

    Ok(())
} 