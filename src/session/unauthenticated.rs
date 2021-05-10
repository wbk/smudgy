use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::account::Account;

use super::{
    handler::{Handler, Response},
    lines_in_codec::LinesInCodec,
    signup::Signup,
};

enum Status {
    WaitingLogin,
    WaitingPassword(String),
}
pub struct Unauthenticated {
    status: Status,
}

impl Unauthenticated {
    pub async fn new(connection: &mut Framed<TcpStream, LinesInCodec>) -> Option<Unauthenticated> {
        match super::motd::send(connection).await {
            Ok(_) => Some(Unauthenticated {
                status: Status::WaitingLogin,
            }),
            Err(_) => None,
        }
    }
}

impl Handler for Unauthenticated {
    fn handle(&mut self, msg: &str) -> Response {
        match &self.status {
            Status::WaitingLogin => match msg {
                "new" => Response {
                    new_handler: Some(Box::new(Signup::new())),
                    msg: Some(String::from(
                        "\r\n\r\nWhat account name would you like to use to log in? ",
                    )),
                },
                "" => Response {
                    new_handler: None,
                    msg: Some(String::from("Login: ")),
                },
                msg => {
                    self.status = Status::WaitingPassword(String::from(msg));

                    Response {
                        new_handler: None,
                        msg: Some(String::from("Password: ")),
                    }
                }
            },
            Status::WaitingPassword(login) => {
                match Account::from(login.as_str(), msg) {
                    None => {
                        self.status = Status::WaitingLogin;

                        Response {
                            new_handler: None,
                            msg: Some(String::from("Unable to log in with this login and password. Retry?\r\n\r\nLogin: ")),
                        }
                    }
                    Some(account) => {
                        self.status = Status::WaitingLogin;

                        Response {
                            new_handler: None,
                            msg: Some(String::from(format!(
                                "Logging you in as {}?\r\n\r\nLogin: ",
                                account.login()
                            ))),
                        }
                    }
                }
            }
        }
    }
}
