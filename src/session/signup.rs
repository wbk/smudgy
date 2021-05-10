use std::borrow::Borrow;

use crate::account::Account;

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
    email: Option<String>,
}

impl Signup {
    pub fn new() -> Signup {
        Signup {
            status: Status::WaitingLogin,
            login: None,
            password: None,
            email: None,
        }
    }
}

impl Handler for Signup {
    fn handle(&mut self, msg: &str) -> Response {
        match &self.status {
            Status::WaitingLogin => match msg {
                "" => {
                    Response {
                        new_handler: None,
                        msg: Some(String::from("Login can't be blank\r\n\r\nWhat account name would you like to use to log in? ")),
                    }
                }
                login if login.len() < 3 => {
                    Response {
                        new_handler: None,
                        msg: Some(String::from("Your account name must be at least 3 letters.\r\n\r\nWhat account name would you like to use to log in? ")),
                    }
                }
                login if login.len() > 12 => {
                    Response {
                        new_handler: None,
                        msg: Some(String::from("Your account name must be at most 12 letters.\r\n\r\nWhat account name would you like to use to log in? ")),
                    }
                }
                login if !STARTS_WITH_LETTER.is_match(login) => {
                    Response {
                        new_handler: None,
                        msg: Some(String::from("Your account name must start with a letter.\r\n\r\nWhat account name would you like to use to log in? ")),
                    }

                }
                login if !ALPHANUMERIC.is_match(login) => {
                    Response {
                        new_handler: None,
                        msg: Some(String::from("Your account name must contain only letters and numbers.\r\n\r\nWhat account name would you like to use to log in? ")),
                    }

                }
                login if Account::exists(login) => {
                    Response {
                        new_handler: None,
                        msg: Some(String::from("An account with this name already exists. Please choose another.\r\n\r\nWhat account name would you like to use to log in? ")),
                    }
                }
                login => {
                    self.login = Some(String::from(login));
                    self.status = Status::WaitingPassword;

                    Response {
                        new_handler: None,
                        msg: Some(String::from("Password: ")),
                    }
                }
            },
            Status::WaitingPassword => match msg {
                password if password.len() < 6 => {
                    Response {
                        new_handler: None,
                        msg: Some(String::from("Your password must be at least 6 characters.\r\n\r\nPlease choose a safe and secure password: ")),
                    }
                }
                password if password.len() > 256 => {
                    Response {
                        new_handler: None,
                        msg: Some(String::from("Your password may be at most 256 characters.\r\n\r\nPlease choose a safe and secure password: ")),
                    }
                }
                password if !validator::validate_non_control_character(password) => {
                    Response {
                        new_handler: None,
                        msg: Some(String::from("Your password may not contain any unicode control characters.\r\n\r\nPlease choose a safe and secure password: ")),
                    }
                }
                password => {
                    self.password = Some(String::from(password));
                    self.status = Status::WaitingConfirmPassword;

                    Response {
                        new_handler: None,
                        msg: Some(String::from("Confirm password: ")),
                    }
                }

            }
            Status::WaitingConfirmPassword => {
                match self.password.borrow() {
                    None => panic!("We really should have a password here!"),
                    Some(password) => {
                        if !password.as_str().eq(msg) {
                            Response {
                                new_handler: None,
                                msg: Some(String::from("Your passwords don't match.\r\n\r\nConfirm password: ")),
                            }
                        } else {
                            self.status = Status::WaitingEmail;

                            Response {
                                new_handler: None,
                                msg: Some(String::from("Passwords confirmed!\r\n\r\nEmail address: ")),
                            }
                        }
                    }
                }
            }
            Status::WaitingEmail => {
                Response {
                    new_handler: None,
                    msg: Some(String::from("(unimplemented) Email address: ")),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::session::{handler::Handler, signup::Status};

    use super::Signup;

    #[test]
    fn test_signup() {
        let mut signup = Signup::new();

        assert_eq!(Status::WaitingLogin, signup.status);

        let result = signup.handle("");
        assert!(result.msg.unwrap().contains("can\'t be blank"));

        let result = signup.handle("ab");
        assert!(result.msg.unwrap().contains("must be at least 3 letters"));

        let result = signup.handle("abcdefghijklm");
        assert!(result.msg.unwrap().contains("must be at most 12 letters"));

        let result = signup.handle("9lives");
        assert!(result.msg.unwrap().contains("must start with a letter"));

        let result = signup.handle("ca$hmoney");
        assert!(result
            .msg
            .unwrap()
            .contains("must contain only letters and numbers"));

        signup.handle("valid");
        assert_eq!(Status::WaitingPassword, signup.status);
    }
}
