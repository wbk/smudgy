use std::error::Error;

pub struct Account {
    login: String,
}

type Result<T> = std::result::Result<T, Box<dyn Error>>;

impl Account {
    pub fn new(login: &str) -> Result<Self> {
        Ok(Account {
            login: String::from("Test"),
        })
    }

    pub fn from(login: &str, password: &str) -> Option<Account> {
        Some(Account {
            login: String::from(login),
        })
    }

    pub fn exists(login: &str) -> bool {
        false
    }

    pub fn login(&self) -> &str {
        self.login.as_str()
    }
}
