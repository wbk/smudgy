use std::fs;

use argonautica::Verifier;
use quick_error::quick_error;

use serde::Deserialize;
use serde::Serialize;

use argonautica::Hasher;

use crate::config;

#[derive(Debug, Serialize, Deserialize)]
pub struct Account {
    pub login: String,
    pub password_hash: String,
    pub email: String,
}

pub struct AccountParams {
    pub login: String,
    pub password: String,
    pub email: String,
}

quick_error! {
    #[derive(Debug)]
    pub enum AccountError {
        InvalidLogin {}
        InvalidPassword {}
        AlreadyExists {}
        SerializationError(err: toml::ser::Error) {
            from()
        }
        DeserializationError(err: toml::de::Error) {
            from()
        }
        ArgonError(err: argonautica::Error) {
            from()
        }
        IOError(err: std::io::Error) {
            from()
        }
    }
}

type Result<T> = std::result::Result<T, AccountError>;

impl Account {
    pub fn new(params: AccountParams) -> Result<Self> {
        if Account::exists(&params.login) {
            return Err(AccountError::AlreadyExists);
        }

        let mut hasher = Hasher::default();
        let hash = hasher
            .with_password(&params.password)
            .with_secret_key(config::PASSWORD_SECRET_KEY_PEPPER)
            .hash()
            .unwrap();

        let account = Account {
            login: params.login,
            password_hash: hash,
            email: params.email,
        };

        account.save()?;

        Ok(account)
    }

    pub fn login(login: &str, password: &str) -> Result<Account> {
        let account = Account::load(login)?;

        let mut verifier = Verifier::default();

        let password_matches = verifier
            .with_hash(&account.password_hash)
            .with_password(password)
            .with_secret_key(config::PASSWORD_SECRET_KEY_PEPPER)
            .verify()?;

        if !password_matches {
            Err(AccountError::InvalidPassword)
        } else {
            Ok(account)
        }
    }

    pub fn exists(login: &str) -> bool {
        let filename = match Account::file_name(login) {
            Ok(filename) => filename,
            _ => return false,
        };

        match fs::metadata(filename.as_str()) {
            Ok(foo) => foo.is_file(),
            _ => false,
        }
    }

    fn load(login: &str) -> Result<Account> {
        if !(config::MIN_LOGIN_LEN..=config::MAX_LOGIN_LEN).contains(&login.len()) {
            return Err(AccountError::InvalidLogin);
        }

        let file_name = Account::file_name(login)?;
        let str = fs::read_to_string(file_name)?;
        let account: Account = toml::from_str(str.as_str())?;

        Ok(account)
    }

    fn save(&self) -> Result<()> {
        let str = toml::to_string(&self)?;
        let path = Account::file_name(self.login.as_str())?;

        Ok(fs::write(path, str)?)
    }

    fn file_name(login: &str) -> Result<String> {
        let mut path = format!("./data/accounts/{}.toml", login);
        path.make_ascii_lowercase();

        Ok(path)
    }
}
