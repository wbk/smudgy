use std::{fs, io, path::{Path, PathBuf}, sync::LazyLock};

use anyhow::Context;

static PROFILES_HOME: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut dir = super::SMUDGY_HOME.clone();
    dir.push("profiles");
    fs::create_dir_all(dir.clone()).context(format!("Failed to create {}, bailing", dir.to_string_lossy())).unwrap();
    dir
});

#[derive(Debug, Clone)]
pub struct Profile {
    name: String,
    host: String,
    port: u16,
}


impl Profile {
    pub fn new(name: Option<&str>) -> Self {
        Self {
            name: name.or(Some("New Profile")).unwrap().to_string(),
            host: String::new(),
            port: 8080
        }
    }

    pub fn host(&self) -> &str {
        self.host.as_str()
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_host(&mut self, host: &str) -> &mut Self {
        self.host = host.to_string();
        self
    }

    pub fn set_port(&mut self, port: u16) -> &mut Self {
        self.port = port;
        self
    }
    // pub fn iter_all() -> impl Iterator<Item = Profile> {
    //     let entries: Vec<_> = fs::read_dir(PROFILES_HOME.clone()).context("Could not read from profiles directory.").unwrap().collect();
    //     entries.retain(|entry| {
    //         if entry.is_err() {

    //             false 
    //         } else {
    //             true
    //         }
    //     });
    //     entries.iter(.map
    // }
}
