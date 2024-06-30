use std::{fs, path::{Path, PathBuf}, sync::LazyLock};

use anyhow::Context;

mod character;
mod profile;

pub use character::Character;
pub use profile::{Profile, ProfileData};
use regex::Regex;
use validator::ValidationError;

static SMUDGY_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut dir = dirs::document_dir().unwrap();
    dir.push("smudgy");
    fs::create_dir_all(dir.clone()).context(format!("Failed to create {}, bailing", dir.to_string_lossy())).unwrap();
    dir
});

static REGEX_VALID_NAME_CHARACTERS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z0-9 \-_]+$").unwrap()
});

static REGEX_VALID_NAME_STARTING_CHARACTERS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z0-9_]+").unwrap()
});

static REGEX_VALID_NAME_ENDING_CHARACTERS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9_]+$").unwrap()
});

pub fn validate_name(value: &str) -> Result<(), ValidationError> {
    if !REGEX_VALID_NAME_CHARACTERS.is_match(value) {
        return Err(ValidationError::new("invalid_char").with_message(std::borrow::Cow::Owned("Name must contain only alphanumeric characters, spaces, dashes, or underscores.".into())));
    } 
    if !REGEX_VALID_NAME_ENDING_CHARACTERS.is_match(value) {
        return Err(ValidationError::new("invalid_end_char").with_message(std::borrow::Cow::Owned("Name must end with a letter, number, or underscore character.".into())));
    }
    if !REGEX_VALID_NAME_STARTING_CHARACTERS.is_match(value) {
        return Err(ValidationError::new("invalid_start_char").with_message(std::borrow::Cow::Owned("Name must start with a letter, number, or underscore character.".into())));
    }
    Ok(())
}
