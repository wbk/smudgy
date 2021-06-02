use crate::{
    account::{Account, AccountParams},
    config,
};

use super::handler::{Handler, Response};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref ALPHANUMERIC: Regex = Regex::new(r"^[a-zA-Z0-9]+$").unwrap();
    static ref STARTS_WITH_LETTER: Regex = Regex::new(r"^[a-zA-Z].").unwrap();
}

#[derive(PartialEq, Debug)]
enum Status {
    WaitingLogin,
    WaitingPassword,
    WaitingConfirmPassword,
    WaitingEmail,
}
pub struct Signup {
    status: Status,
    login: Option<String>,
    password: Option<String>,
}

impl Signup {
    pub fn new() -> Signup {
        Signup {
            status: Status::WaitingLogin,
            login: None,
            password: None,
        }
    }
}

impl Handler for Signup {
    fn preamble(&self) -> Option<String> {
        Some(String::from("\r\n\r\nWhat account name would you like to use to log in? "))
    }

    fn handle(&mut self, msg: &str) -> Response {
        match &self.status {
            Status::WaitingLogin => match msg {
                "" => {
                    "Login can't be blank\r\n\r\nWhat account name would you like to use to log in? ".into()
                }
                login if login.len() < config::MIN_LOGIN_LEN => {
                    format!("Your account name must be at least {} letters.\r\n\r\nWhat account name would you like to use to log in? ", config::MIN_LOGIN_LEN).into()
                }
                login if login.len() > config::MAX_LOGIN_LEN => {
                    format!("Your account name must be at most {} letters.\r\n\r\nWhat account name would you like to use to log in? ", config::MAX_LOGIN_LEN).into()
                }
                login if !STARTS_WITH_LETTER.is_match(login) => {
                    "Your account name must start with a letter.\r\n\r\nWhat account name would you like to use to log in? ".into()

                }
                login if !ALPHANUMERIC.is_match(login) => {
                    "Your account name must contain only letters and numbers.\r\n\r\nWhat account name would you like to use to log in? ".into()

                }
                login if Account::exists(login) => {
                    "An account with this name already exists. Please choose another.\r\n\r\nWhat account name would you like to use to log in? ".into()
                }
                login => {
                    self.login = Some(String::from(login));
                    self.status = Status::WaitingPassword;

                    "Password: ".into()
                }
            },
            Status::WaitingPassword => match msg {
                password if password.len() < 6 => {
                    "Your password must be at least 6 characters.\r\n\r\nPlease choose a safe and secure password: ".into()
                }
                password if password.len() > 256 => {
                    "Your password may be at most 256 characters.\r\n\r\nPlease choose a safe and secure password: ".into()
                }
                password if !validator::validate_non_control_character(password) => {
                    "Your password may not contain any unicode control characters.\r\n\r\nPlease choose a safe and secure password: ".into()
                }
                password => {
                    self.password = Some(String::from(password));
                    self.status = Status::WaitingConfirmPassword;

                    "Confirm password: ".into()
                }
            }
            Status::WaitingConfirmPassword => {
                let password = self.password.as_ref().unwrap().as_str();
                if !password.eq(msg) {
                    "Your passwords don't match.\r\n\r\nConfirm password: ".into()
                } else {
                    self.status = Status::WaitingEmail;

                    "Passwords confirmed!\r\n\r\nEmail address: ".into()
                }
            }
            Status::WaitingEmail => {
                match msg {
                    "" =>  "Email address: ".into(),

                    email if !validator::validate_email(email) => {
                        "Your email address must be valid.\r\n\r\nEmail address: ".into()
                        
                    }
                    email => {
                        match Account::new(AccountParams {
                            login: self.login.as_ref().unwrap().clone(),
                            password: self.password.as_ref().unwrap().clone(),
                            email: String::from(email)
                        }) {
                            Ok(account) => Response::Message(format!("Account created! hashed password is {}", account.password_hash))                              ,
                            _ => "Something went wrong!".into()
                    }                        
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::session::{handler::{Handler, Response}, signup::Status};

    use super::Signup;

    #[test]
    fn test_signup() {
        let mut signup = Signup::new();

        assert_eq!(Status::WaitingLogin, signup.status);

        let result = signup.handle("");
        assert!(matches!(result, Response::Message(msg) if msg.contains("can\'t be blank")));

        let result = signup.handle("ab");
        assert!(matches!(result, Response::Message(msg) if msg.contains("must be at least 3 letters")));

        let result = signup.handle("abcdefghijklm");
        assert!(matches!(result, Response::Message(msg) if msg.contains("must be at most 12 letters")));

        let result = signup.handle("9lives");
        assert!(matches!(result, Response::Message(msg) if msg.contains("must start with a letter")));

        let result = signup.handle("ca$hmoney");
        assert!(matches!(result, Response::Message(msg) if msg.contains("must contain only letters and numbers")));

        signup.handle("valid");
        assert_eq!(Status::WaitingPassword, signup.status);

        let result = signup.handle("12345");
        assert!(matches!(result, Response::Message(msg) if msg.contains("must be at least 6")));

        let result = signup.handle("12345");
        assert!(matches!(result, Response::Message(msg) if msg.contains("must be at least 6")));

        let long_password: String = ['x'; 257].iter().collect();
        let result = signup.handle(long_password.as_str());
        assert!(matches!(result, Response::Message(msg) if msg.contains("may be at most 256")));

        signup.handle("s3cure_enough");
        assert_eq!(Status::WaitingConfirmPassword, signup.status);

        signup.handle("s3cure_enough");
        assert_eq!(Status::WaitingEmail, signup.status);

        let result = signup.handle("not_an_email_address");
        assert!(matches!(result, Response::Message(msg) if msg.contains("email address must be valid")));
    }
}
