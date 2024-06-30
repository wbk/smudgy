use std::{
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    rc::Weak,
    time::SystemTime,
};

use anyhow::{Context, Result};
use deno_core::serde::{Deserialize, Serialize};
use humantime::{self, format_duration, Duration};
use super::Profile;

#[derive(Debug, Clone)]
pub struct Character {
    name: String,
    subtext: String,
    send_on_connect: String,
    send_on_connect_hidden: bool,
    profile: Weak<Profile>,
}

#[derive(Serialize, Deserialize, Default)]
struct CharacterData {
    subtext: String,
    send_on_connect: String,
    send_on_connect_hidden: bool,
}

const CHARACTER_JSON_FILENAME: &str = "character.json";

impl Character {
    pub fn new(name: &str, profile: Weak<Profile>) -> Self {
        let char = Character {
            name: name.to_string(),
            profile,
            subtext: String::default(),
            send_on_connect: String::default(),
            send_on_connect_hidden: false,
        };

        if !Character::exists(char.name(), char.profile.clone()) {
            char.save().unwrap();
        }

        char
    }

    fn exists(name: &str, profile: Weak<Profile>) -> bool {
        let mut dir = Self::dir_for(name, profile);
        dir.push(CHARACTER_JSON_FILENAME);
        return Path::exists(&dir);
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn send_on_connect(&self) -> &str {
        self.send_on_connect.as_str()
    }

    pub fn send_on_connect_hidden(&self) -> bool {
        self.send_on_connect_hidden
    }

    pub fn subtext(&self) -> &str {
        self.subtext.as_str()
    }

    fn dir(&self) -> PathBuf {
        Character::dir_for(self.name(), self.profile.clone())
    }

    fn dir_for(name: &str, profile: Weak<Profile>) -> PathBuf {
        let mut dir = profile.upgrade().unwrap().dir().clone();
        dir.push("characters");
        dir.push(name);
        fs::create_dir_all(dir.clone()).expect("Could not create directory for character");
        dir
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        let mut filename = self.dir();
        filename.push(CHARACTER_JSON_FILENAME);

        let json = serde_json::to_string_pretty(&CharacterData {
            send_on_connect: self.send_on_connect().to_string(),
            send_on_connect_hidden: self.send_on_connect_hidden(),
            subtext: self.subtext().to_string(),
        })
        .context("Could not generate character json")?;

        fs::write(filename, json).context("Could not save character")?;

        Ok(())
    }

    pub fn touch(&self) {
        let mut filename = self.dir();
        filename.push(CHARACTER_JSON_FILENAME);
        let filename = filename.canonicalize().unwrap();

        let file = fs::File::options().append(true).open(filename.clone()).unwrap();
        file.set_modified(SystemTime::now()).with_context(||format!("Couldn't set modified time on {:?}", filename)).unwrap();
    }

    pub fn last_used_string(&self) -> String {
        let mut filename = Character::dir_for(self.name(), self.profile.clone());
        filename.push(CHARACTER_JSON_FILENAME);


        fs::metadata(filename)
            .map(|meta| {
                Ok::<std::string::String, anyhow::Error>(
                    format_duration(meta.modified()?.elapsed()?).to_string(),
                )
            })
            .unwrap_or(Ok("never".to_string()))
            .unwrap()
    }

    pub fn load(name: &str, profile: Weak<Profile>) -> Result<Self, anyhow::Error> {
        let mut filename = Character::dir_for(name, profile.clone());
        filename.push(CHARACTER_JSON_FILENAME);

        let file = File::open(filename).context("Could not open character for reading")?;
        let reader = BufReader::new(file);

        // Read the JSON contents of the file as an instance of `User`.
        let char: CharacterData =
            serde_json::from_reader(reader).unwrap_or(CharacterData::default());

        Ok(Character {
            name: name.to_string(),
            send_on_connect: char.send_on_connect,
            send_on_connect_hidden: char.send_on_connect_hidden,
            subtext: char.subtext,
            profile,
        })
    }

    pub fn iter_all(profile: Weak<Profile>) -> impl Iterator<Item = Character> {
        let mut dir = profile.upgrade().unwrap().dir();
        dir.push(format!("characters"));

        let characters: Vec<_> = fs::read_dir(dir)
            .context("Could not read from profile's characters directory.")
            .unwrap()
            .filter(|entry| {
                if let Ok(entry) = entry {
                    entry.file_type().unwrap().is_dir()
                } else {
                    false
                }
            })
            .map(|dir| dir.unwrap().file_name().to_str().unwrap().to_string())
            .filter(|name| Character::exists(name, profile.clone()))
            .map(|name| Character::load(&name, profile.clone()).unwrap())
            .collect();

        characters.into_iter()
    }
}

impl From<Character> for smudgy_connect_window::Character {
    fn from(value: Character) -> Self {
        smudgy_connect_window::Character {
            name: value.name().into(),
            last_used: value.last_used_string().into(),
            send_on_connect: value.send_on_connect().into(),
            send_on_connect_hidden: value.send_on_connect_hidden(),
            subtext: value.subtext().into(),
        }
    }
}
