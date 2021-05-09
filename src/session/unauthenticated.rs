use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use super::{
    handler::{Handler, Response},
    lines_in_codec::LinesInCodec,
};
pub struct Unauthenticated {}

impl Unauthenticated {
    pub async fn new(connection: &mut Framed<TcpStream, LinesInCodec>) -> Option<Unauthenticated> {
        match super::motd::send(connection).await {
            Ok(_) => Some(Unauthenticated {}),
            Err(_) => None,
        }
    }
}

impl Handler for Unauthenticated {
    fn handle(&mut self, msg: &str) -> Response {
        Response {
            new_handler: None,
            msg: Some(format!("You entered: '{}'\r\n> ", msg)),
        }
    }
}
