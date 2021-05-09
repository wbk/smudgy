use std::{
    error::Error,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use futures::SinkExt;
use tokio::{net::TcpStream, sync::mpsc};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use lazy_static::lazy_static;

use self::{handler::Handler, lines_in_codec::LinesInCodec, unauthenticated::Unauthenticated};

mod handler;
mod lines_in_codec;
mod motd;
mod unauthenticated;

const MAX_INPUT_LINE_LENGTH: usize = 120;

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
            LinesInCodec::new_with_max_length(MAX_INPUT_LINE_LENGTH),
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
        if let Some(mut session) = Session::new(stream).await {
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
                            if session.connection.send(msg).await.is_err() {
                                break;
                            }
                        }
                    },
                }
            }
            println!("{} disconnected", session.peer_address);
        }
    }

    async fn handle(&mut self, msg: &str) -> Result<(), Box<dyn Error>> {
        let handler = self.handler.as_mut();

        let response = handler.handle(msg);

        if let Some(msg) = response.msg {
            self.connection.send(msg).await?;
        }

        if let Some(handler) = response.new_handler {
            self.handler = handler;
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
/// ```
/// let session = Session::new();
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
