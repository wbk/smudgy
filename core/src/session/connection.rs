use std::sync::{Arc, RwLock};

use iced::futures::channel::mpsc;
use tokio::{
    io::{self, AsyncWriteExt, Interest},
    net::TcpStream,
    select,
    sync::{
        mpsc::{UnboundedSender, WeakUnboundedSender},
        oneshot,
    },
};
use vt_processor::VtProcessor;
use vtparse::VTParser;

use super::{TaggedSessionEvent, runtime::RuntimeAction};

pub mod vt_processor;
pub struct Connection {
    disconnect: Option<oneshot::Sender<()>>,
    runtime_tx: UnboundedSender<RuntimeAction>,
    ui_tx: mpsc::Sender<TaggedSessionEvent>,
    socket_tx: Arc<RwLock<Option<WeakUnboundedSender<Arc<String>>>>>,
    on_connect: Option<Box<dyn FnOnce() -> () + Send>>,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("disconnect", &self.disconnect)
            .field("runtime_tx", &self.runtime_tx)
            .field("ui_tx", &self.ui_tx)
            .field("socket_tx", &self.socket_tx)
            .field("on_connect", &self.on_connect.is_some())
            .finish()
    }
}


impl Connection {
    #[must_use]
    pub fn new(
        runtime_tx: UnboundedSender<RuntimeAction>,
        ui_tx: iced::futures::channel::mpsc::Sender<TaggedSessionEvent>,
    ) -> Self {
        Self {
            disconnect: None,
            runtime_tx,
            ui_tx,
            socket_tx: Arc::new(RwLock::new(None)),
            on_connect: None,
        }
    }

    pub fn write(&self, data: Arc<String>) -> Result<(), anyhow::Error> {
        let socket_tx = self.socket_tx.read().unwrap();
        if let Some(socket_tx) = socket_tx.as_ref() {
            if let Some(socket_tx) = socket_tx.upgrade() {
                socket_tx.send(data).unwrap();
                Ok(())
            } else {
                Err(anyhow::anyhow!("Socket tx is not upgradeable"))
            }
        } else {
            Err(anyhow::anyhow!("Socket no longer exists"))
        }
    }

    /// Establishes a TCP connection to the specified host and port.
    ///
    /// This function spawns a new Tokio task to handle the connection, including
    /// reading data from the socket, processing it with a VT parser, and sending
    /// outgoing data.
    ///
    /// If a previous connection managed by this `Connection` instance exists, it will
    /// be signaled to disconnect.
    ///
    /// # Panics
    ///
    /// This function can panic under the following conditions:
    /// - If sending initial messages (like "Connecting to...") to the session runtime fails (channel closed).
    /// - If `stream.set_nodelay(true)` fails on the newly connected TCP stream.
    /// - If sending the `UpdateWriteToSocketTx` action to the session runtime fails (channel closed).
    /// - If writing the `send_on_connect` data to the TCP stream fails.
    pub fn connect(&mut self, host: &str, port: u16) {
        let addr = format!("{host}:{port}");
        let runtime_tx = self.runtime_tx.clone();
        let (tx, mut disconnect_rx) = oneshot::channel();

        if let Some(disconnect) = self.disconnect.take() {
            // This will error if the channel is already closed, which is fine
            disconnect.send(()).ok();
        }

        self.disconnect = Some(tx);

        self.socket_tx = Arc::new(RwLock::new(None));
        let socket_tx = self.socket_tx.clone();

        let on_connect = self.on_connect.take();

        tokio::spawn(async move {
            let mut vt_parser = VTParser::new();
            let mut vt_processor = VtProcessor::new(runtime_tx.clone());
            let (write_to_socket_tx, mut write_to_socket_rx) =
                tokio::sync::mpsc::unbounded_channel::<Arc<String>>();

            runtime_tx
                .send(RuntimeAction::Echo(Arc::new(format!(
                    "Connecting to {addr}..."
                ))))
                .unwrap();
            trace!("Connecting to {addr}...");

            match TcpStream::connect(addr).await {
                Ok(mut stream) => {
                    runtime_tx
                        .send(RuntimeAction::Echo(Arc::new(format!("Connected."))))
                        .unwrap();
                    stream.set_nodelay(true).unwrap();
                    trace!("Connected");

                    if let Some(on_connect) = on_connect {
                        on_connect();
                    }

                    socket_tx
                        .write()
                        .unwrap()
                        .replace(write_to_socket_tx.downgrade());

                    runtime_tx
                        .send(RuntimeAction::Connected)
                        .unwrap();

                    loop {
                        select! {
                            Ok(ready) = stream.ready(Interest::READABLE) => {
                                if ready.is_readable() {
                                    let mut data: Vec<u8> = Vec::with_capacity(65536);

                                    match stream.try_read_buf(&mut data) {
                                        Ok(n) => {
                                            if n == 0 {
                                                break;
                                            }

                                            for b in &data {
                                                if *b != b'\n' && *b != b'\r' {
                                                    vt_processor.push_raw_incoming_byte(*b);
                                                }
                                                vt_parser.parse_byte(*b, &mut vt_processor);
                                            }

                                            vt_processor.notify_end_of_buffer();
                                        }
                                        Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
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
                    runtime_tx
                        .send(RuntimeAction::Connected)
                        .map(|()| {
                            runtime_tx
                                .send(RuntimeAction::Echo(Arc::new("Connection lost".to_string())))
                                .ok();
                        })
                        .ok();
                }
                _ => {
                    runtime_tx
                        .send(RuntimeAction::Echo(Arc::new(
                            "Connection failed".to_string(),
                        )))
                        .map_err(|_| {
                            warn!("Error notifying runtime of connection failure; ignoring");
                        })
                        .ok();
                }
            }
            trace!("Connection cleaning up");
            socket_tx.write().unwrap().take();
        });
    }

    pub fn disconnect(&mut self) {
        if let Some(disconnect) = self.disconnect.take() {
            disconnect.send(()).ok();
        }
    }

    pub fn on_connect(&mut self, on_connect: impl FnOnce() -> () + Send + 'static) {
        self.on_connect = Some(Box::new(on_connect));
    }
}
