use confy;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::default::Default;

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
    bind_address: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bind_address: String::from("0.0.0.0:4040"),
        }
    }
}

impl Config {
    pub fn get_bind_address(&self) -> &String {
        &self.bind_address
    }
}

lazy_static! {
    pub static ref CONFIG: Config = confy::load("smudgy").expect("Failed to load configuration");
}
