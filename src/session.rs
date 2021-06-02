use std::{
    error::Error,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use bytes::BufMut;
use futures::SinkExt;
use tokio::{net::TcpStream, sync::mpsc};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use lazy_static::lazy_static;

use crate::{config, session::handler::Response};

use self::{handler::Handler, lines_in_codec::LinesInCodec, unauthenticated::Unauthenticated};

mod account_menu;
mod create_character;
mod handler;
mod lines_in_codec;
mod motd;
mod signup;
mod unauthenticated;

type Connection = Framed<TcpStream, LinesInCodec>;

lazy_static! {
    static ref SESSIONS: Arc<Mutex<Vec<Tx>>> = Arc::new(Mutex::new(Vec::new()));
}

pub enum Message {
    Buffered(String),
    Flushed(String),
    Close,
}

type Tx = mpsc::UnboundedSender<Message>;
type Rx = mpsc::UnboundedReceiver<Message>;

pub struct Session {
    connection: Connection,
    peer_address: SocketAddr,
    tx: Tx,
    rx: Rx,
    handler: Box<dyn Handler + Send>,
}

impl Session {
    async fn new(stream: TcpStream) -> Option<Session> {
        let addr = stream.peer_addr().unwrap();

        let mut connection = Framed::new(
            stream,
            LinesInCodec::new_with_max_length(config::MAX_INPUT_LINE_LENGTH),
        );

        if let Some(handler) = Unauthenticated::new(&mut connection).await {
            let (tx, rx) = mpsc::unbounded_channel();

            Arc::clone(&SESSIONS).lock().unwrap().push(tx.clone());

            Some(Session {
                peer_address: addr,
                connection,
                tx,
                rx,
                handler: Box::new(handler),
            })
        } else {
            None
        }
    }

    pub async fn process(stream: TcpStream) {
        let mut session = match Session::new(stream).await {
            Some(it) => it,
            _ => return,
        };
        loop {
            tokio::select! {
                result = session.connection.next() => {
                    match result {
                    None => break,
                    Some(Ok(msg)) => {
                        if let Err(e) = session.handle(msg.as_str()).await {
                            println!("Error occured in session.handle: {:?}", e);
                            break;
                        }
                    },
                    Some(Err(e)) => {
                        println!("An error occured: {:?}", e);
                        break;
                    }
                }
            }

                recv = session.rx.recv() => match recv {
                    None => break,
                    Some(Message::Close) => break,
                    Some(Message::Buffered(msg)) => {
                        if session.connection.feed(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Message::Flushed(msg)) => {
                        if session.connection.feed(msg).await.is_err() {
                            break;
                        }

                        session.send_iac_ga();

                        if session.connection.send(String::from("")).await.is_err() {
                            break;
                        }
                    }
                },
            }
        }

        println!("{} disconnected", session.peer_address);
    }

    fn send_iac_ga(&mut self) {
        println!("Sending IAC GA to {}", self.peer_address);

        let buffer = self.connection.write_buffer_mut();
        buffer.reserve(2);
        buffer.put(&b"\xff\xf9"[..]);
    }

    async fn handle(&mut self, msg: &str) -> Result<(), Box<dyn Error>> {
        let handler = self.handler.as_mut();

        match handler.handle(msg) {
            Response::Message(msg) => {
                self.connection.feed(msg).await?;
                self.send_iac_ga();
                self.connection.send(String::from("")).await?;
            }

            Response::NewHandler(handler) => {
                self.handler = handler;
                let preamble = self.handler.preamble();

                if let Some(msg) = preamble {
                    self.connection.feed(msg).await?;
                    self.send_iac_ga();
                    self.connection.send(String::from("")).await?;
                }
            }
        }

        println!("Message in from {}: {}", self.peer_address, msg);

        Ok(())
    }

    pub fn send(&self, msg: Message) {
        self.tx.send(msg);
    }
}

/// Broadcast a message to all connected sessions.
///
/// ```ignore
/// use smudgy::session;
/// session::broadcast("Hello!");
/// ```
pub fn broadcast(msg: &str) {
    let sessions = Arc::clone(&SESSIONS);
    let mut sessions = sessions.lock().unwrap();
    sessions.retain(|session| session.send(Message::Flushed(String::from(msg))).is_ok());
}

pub fn close_all() {
    let sessions = Arc::clone(&SESSIONS);
    let mut sessions = sessions.lock().unwrap();
    sessions.retain(|session| session.send(Message::Close).is_ok());
}
