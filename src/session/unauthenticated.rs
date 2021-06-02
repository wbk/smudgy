use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::account::{Account, AccountError};

use super::{
    account_menu::AccountMenu,
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
                "new" => Response::NewHandler(Box::new(Signup::new())),
                "" => "Login: ".into(),

                msg => {
                    self.status = Status::WaitingPassword(String::from(msg));

                    "Password: ".into()
                }
            },

            Status::WaitingPassword(login) => match Account::login(login.as_str(), msg) {
                Err(err) => {
                    self.status = Status::WaitingLogin;

                    match err {
                        AccountError::InvalidPassword => {
                            "Invalid login or password. Retry?\r\n\r\nLogin: ".into()
                        }
                        AccountError::InvalidLogin => {
                            "Invalid login or password. Retry?\r\n\r\nLogin: ".into()
                        }
                        _ => "An unknown error occured. Retry?\r\n\r\nLogin: ".into(),
                    }
                }

                Ok(account) => Response::NewHandler(Box::new(AccountMenu::new(account))),
            },
        }
    }
}
