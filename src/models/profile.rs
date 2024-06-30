use std::{
    borrow::Cow, fs::{self, File}, io::{BufReader, ErrorKind}, path::{Path, PathBuf}, rc::Rc, sync::LazyLock
};

use anyhow::{anyhow, bail, Context, Result};
use deno_core::serde::{Deserialize, Serialize};
use slint::VecModel;
use validator::{Validate, ValidationErrors};

use super::Character;

static PROFILES_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut dir = super::SMUDGY_HOME.clone();
    dir.push("profiles");

    fs::create_dir_all(dir.clone())
        .with_context(|| format!("Failed to create {}, bailing", dir.to_string_lossy()))
        .unwrap();

    dir
});

#[derive(Debug, Clone)]
pub struct Profile {
    name: String,
    host: String,
    port: u16,
}

#[derive(Serialize, Deserialize, Validate)]
pub struct ProfileData {
    #[validate(length(min = 1, message = "Name must not be empty"), custom(function = super::validate_name))]
    #[serde(skip)]
    pub name: String,

    #[validate(length(min = 1, message = "Host must not be empty"))]
    pub host: String,

    #[validate(range(min = 1, max = 65535, message = "Port must be between 1 and 65535"))]
    pub port: u16,
}

const PROFILE_JSON_FILENAME: &str = "profile.json";

impl Profile {
    pub fn new<T>(profile: T) -> Result<Self>
    where
        T: TryInto<Profile, Error = ValidationErrors>,
    {
        let profile = profile.try_into().map_err(|e| anyhow!("Unable to create profile:\n\n{}",e.to_string()))?;
        /* .map_err(|e| {
            let first_error = e.field_errors().into_values().into_iter().next().unwrap().first().unwrap();
            anyhow!(first_error.message.clone().or_else(|| Some(Cow::Owned(first_error.to_string()))).unwrap())
        })?;*/

        if Profile::exists(&profile.name) {
            bail!("A profile with this name already exists");
        }

        profile.save()?;

        Ok(profile)
    }

    fn exists(name: &str) -> bool {
        let mut dir = Profile::dir_for(name);
        dir.push(PROFILE_JSON_FILENAME);
        return Path::exists(&dir);
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn host(&self) -> &str {
        self.host.as_str()
    }

    pub fn set_host(&mut self, host: &str) {
        self.host = host.to_string();
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }

    pub fn dir(&self) -> PathBuf {
        Profile::dir_for(self.name())
    }

    fn dir_for(name: &str) -> PathBuf {
        let mut dir = PROFILES_HOME.clone();
        dir.push(name);
        fs::create_dir_all(dir.clone()).expect("Could not create directory for profile");

        for subdir in vec!["characters", "triggers", "hotkeys", "aliases"] {
            let mut dir = dir.clone();
            dir.push(subdir);

            fs::create_dir_all(dir.clone())
                .with_context(|| format!("Failed to create {}, bailing", dir.to_string_lossy()))
                .unwrap();
        }
        dir
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        let mut filename = self.dir();
        filename.push(PROFILE_JSON_FILENAME);

        let data = ProfileData::try_from(self.clone())?;
        let json =
            serde_json::to_string_pretty(&data).context("Could not generate profile json")?;

        fs::write(filename, json).context("Could not save profile")?;

        Ok(())
    }

    pub fn load(name: &str) -> Result<Self, anyhow::Error> {
        let mut filename = Profile::dir_for(name);
        filename.push(PROFILE_JSON_FILENAME);

        let file = File::open(filename).context("Could not open profile for reading")?;
        let reader = BufReader::new(file);

        // Read the JSON contents of the file as an instance of `User`.
        let data: ProfileData =
            serde_json::from_reader(reader).context("Could not parse profile.json")?;

        Ok(Profile {
            name: name.to_string(),
            host: data.host,
            port: data.port,
        })
    }

    pub fn delete(profile: Profile) -> Result<()> {
        match fs::remove_dir_all(profile.dir()) {
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            response => response.context("Failed to delete profile"),
        }
    }

    pub fn iter_all() -> impl Iterator<Item = Profile> {
        fs::read_dir(PROFILES_HOME.clone())
            .context("Could not read from profiles directory.")
            .unwrap()
            .filter(|entry| {
                if let Ok(entry) = entry {
                    entry.file_type().unwrap().is_dir()
                } else {
                    false
                }
            })
            .map(|dir| dir.unwrap().file_name().to_str().unwrap().to_string())
            .filter(|name| Profile::exists(name))
            .map(|name| Profile::load(&name).unwrap())
    }
}

impl From<Profile> for smudgy_connect_window::Profile {
    fn from(value: Profile) -> Self {
        let value = Rc::new(value);

        let characters: Vec<smudgy_connect_window::Character> =
            Character::iter_all(Rc::downgrade(&value))
                .map(|c| c.into())
                .collect();

        smudgy_connect_window::Profile {
            name: value.name().into(),
            host: value.host().into(),
            port: value.port as i32,
            characters: Rc::new(VecModel::from(characters)).into(),
        }
    }
}

impl From<smudgy_connect_window::Profile> for ProfileData {
    fn from(value: smudgy_connect_window::Profile) -> Self {
        ProfileData {
            name: value.name.to_string(),
            host: value.host.to_string(),
            port: value.port as u16,
        }
    }
}

impl TryFrom<ProfileData> for Profile {
    type Error = ValidationErrors;
    fn try_from(value: ProfileData) -> Result<Self, Self::Error> {
        ProfileData::validate(&value)?;

        Ok(Profile {
            name: value.name,
            host: value.host,
            port: value.port,
        })
    }
}

impl TryFrom<Profile> for ProfileData {
    type Error = ValidationErrors;
    fn try_from(value: Profile) -> Result<Self, Self::Error> {
        let profile_data = ProfileData {
            name: value.name,
            host: value.host,
            port: value.port,
        };
        ProfileData::validate(&profile_data)?;
        Ok(profile_data)
    }
}
