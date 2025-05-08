use std::sync::Arc;

use tokio::{
    io::{self, AsyncWriteExt, Interest},
    net::TcpStream,
    select,
    sync::{mpsc::UnboundedSender, oneshot},
};
use vt_processor::VtProcessor;
use vtparse::VTParser;

use super::runtime::{RuntimeAction, Runtime};

pub mod vt_processor;
pub struct Connection {
    disconnect: Option<oneshot::Sender<()>>,
    session_runtime_tx: UnboundedSender<RuntimeAction>,
    send_on_connect: String,
}

impl Connection {
    pub fn new(session_runtime: &Arc<Runtime>, send_on_connect: &str) -> Self {
        Self {
            disconnect: None,
            session_runtime_tx: session_runtime.tx(),
            send_on_connect: send_on_connect.to_string(),
        }
    }

    pub fn connect(&mut self, host: &str, port: u16) {
        let addr = format!("{host}:{port}");
        let session_runtime_tx = self.session_runtime_tx.clone();
        let (tx, mut disconnect_rx) = oneshot::channel();

        if let Some(disconnect) = self.disconnect.take() {
            // This will error if the channel is already closed, which is fine
            disconnect.send(()).ok();
        }

        self.disconnect = Some(tx);
        let send_on_connect = self.send_on_connect.clone();

        tokio::spawn(async move {
            let mut vt_parser = VTParser::new();
            let mut vt_processor = VtProcessor::new(session_runtime_tx.clone());
            let (write_to_socket_tx, mut write_to_socket_rx) = tokio::sync::mpsc::unbounded_channel::<Arc<String>>();

            session_runtime_tx.send(RuntimeAction::Echo(Arc::new(format!("\r\nConnecting to {addr}...")))).unwrap();
            trace!("Connecting to {addr}...");

            match TcpStream::connect(addr).await {
                Ok(mut stream) => {
                    stream.set_nodelay(true).unwrap();
                    trace!("Connected");
                    session_runtime_tx.send(RuntimeAction::UpdateWriteToSocketTx(Some(write_to_socket_tx))).unwrap();

                    if !send_on_connect.is_empty() {
                        for line in send_on_connect.split(|ch| ch == '\n' || ch == ';') {
                            stream.write_all(format!("{line}\r\n").as_bytes()).await.unwrap();
                        }
                    }

                    loop {
                        select! {
                            Ok(ready) = stream.ready(Interest::READABLE) => {
                                if ready.is_readable() {
                                    let mut data: Vec<u8> = Vec::with_capacity(4096);

                                    match stream.try_read_buf(&mut data) {
                                        Ok(n) => {
                                            if n == 0 {
                                                // TODO: notify session that the connection was reset
        //                                        echo_tx.send("Connection reset!".into()).unwrap();
                                                break;
                                            }

                                            for b in &data {
                                                if *b != '\n' as u8 && *b != '\r' as u8 {
                                                    vt_processor.push_raw_incoming_byte(*b);
                                                }
                                                vt_parser.parse_byte(*b, &mut vt_processor);
                                            }

                                            vt_processor.notify_end_of_buffer();
                                        }
                                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                            continue;
                                        }
                                        Err(_) => {
                                            // TODO: notify session that the try_read_buf errored
                                            // return Err::<(), anyhow::Error>(e.into());
                                            break;
                                        }
                                    }
                                }
                            }
                            Some(ref data) = write_to_socket_rx.recv() => {
                                if stream.write_all(data.as_bytes()).await.is_err() {
                                    break;
                                }
                            }
                            _ = &mut disconnect_rx => {
                                break;
                            }
                            else => {
                                break;
                            }
                        }
                    }

                    // Silently ignore errors here; when a session is closing the runtime may already be gone by the time
                    // we get here
                    session_runtime_tx.send(RuntimeAction::UpdateWriteToSocketTx(None)).map(|_| {
                        session_runtime_tx.send(RuntimeAction::Echo(Arc::new(format!("\r\nConnection lost")))).ok();
                    }).ok();
                }
                _ => {
                    session_runtime_tx.send(RuntimeAction::Echo(Arc::new(format!("\r\nConnection failed")))).map_err(|_| {
                        warn!("Error notifying runtime of connection failure; ignoring");
                    }).ok();
                }
            }
            trace!("Connection cleaning up");
        });
    }
}
