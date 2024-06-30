use std::sync::Arc;

use tokio::{
    io::{self, AsyncWriteExt, Interest},
    net::TcpStream,
    select,
    sync::{mpsc::UnboundedSender, oneshot},
};
use vt_processor::VtProcessor;
use vtparse::VTParser;

use crate::{
    script_runtime::{RuntimeAction, ScriptRuntime},
    trigger::TriggerManager,
};

pub mod vt_processor;
pub struct Connection {
    trigger_manager: Arc<TriggerManager>,
    disconnect: Option<oneshot::Sender<()>>,
    script_action_tx: UnboundedSender<RuntimeAction>,
}

impl Connection {
    pub fn new(trigger_manager: Arc<TriggerManager>, script_runtime: Arc<ScriptRuntime>) -> Self {
        Self {
            trigger_manager,
            disconnect: None,
            script_action_tx: script_runtime.tx(),
        }
    }

    pub fn connect(&mut self, host: &str, port: u16) {
        let addr = format!("{host}:{port}");
        let arc_trigger_manager = self.trigger_manager.clone();
        let script_action_tx = self.script_action_tx.clone();
        let (tx, mut disconnect_rx) = oneshot::channel();

        if let Some(disconnect) = self.disconnect.take() {
            // This will error if the channel is already closed, which is fine
            disconnect.send(()).ok();
        }

        self.disconnect = Some(tx);

        crate::TOKIO.spawn(async move {
            let mut vt_parser = VTParser::new();
            let mut vt_processor = VtProcessor::new(arc_trigger_manager);
            let (write_to_socket_tx, mut write_to_socket_rx) = tokio::sync::mpsc::unbounded_channel::<Arc<String>>();

            script_action_tx.send(RuntimeAction::Echo(Arc::new(format!("\r\nConnecting to {addr}...")))).unwrap();
            trace!("Connecting to {addr}...");

            match TcpStream::connect(addr).await {
                Ok(mut stream) => {
                    stream.set_nodelay(true).unwrap();
                    trace!("Connected");
                    script_action_tx.send(RuntimeAction::UpdateWriteToSocketTx(Some(write_to_socket_tx))).unwrap();

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
                    script_action_tx.send(RuntimeAction::UpdateWriteToSocketTx(None)).map(|_| {
                        script_action_tx.send(RuntimeAction::Echo(Arc::new(format!("\r\nConnection lost")))).ok();
                    }).ok();
                }
                _ => {
                    script_action_tx.send(RuntimeAction::Echo(Arc::new(format!("\r\nConnection failed")))).map_err(|_| {
                        warn!("Error notifying runtime of connection failure; ignoring");
                    }).ok();
                }
            }
            trace!("Connection cleaning up");
        });
    }
}
