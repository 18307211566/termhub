use std::sync::{Arc, Mutex};

use bytes::Bytes;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use termhub_core::{DriverError, DriverEvent, UpstreamDriver};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct LocalShellDriver {
    pub command: String,
    pub args: Vec<String>,
    pub cols: u16,
    pub rows: u16,
}

#[async_trait::async_trait]
impl UpstreamDriver for LocalShellDriver {
    async fn run(
        &mut self,
        mut input_rx: mpsc::Receiver<Bytes>,
        evt_tx: mpsc::Sender<DriverEvent>,
        cancel: CancellationToken,
    ) -> Result<(), DriverError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: self.rows,
                cols: self.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| DriverError::Fatal(format!("openpty: {e}")))?;

        let mut cmd = CommandBuilder::new(&self.command);
        for a in &self.args {
            cmd.arg(a);
        }
        let mut child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| DriverError::Fatal(format!("spawn {}: {e}", self.command)))?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| DriverError::Fatal(format!("clone reader: {e}")))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| DriverError::Fatal(format!("take writer: {e}")))?;
        let writer = Arc::new(Mutex::new(writer));

        evt_tx
            .send(DriverEvent::Connected)
            .await
            .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;

        let evt_tx_r = evt_tx.clone();
        let read_handle = tokio::task::spawn_blocking(move || {
            let mut buf = [0u8; 4096];
            loop {
                use std::io::Read;
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if evt_tx_r
                            .blocking_send(DriverEvent::Output(Bytes::copy_from_slice(&buf[..n])))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    let _ = child.kill();
                    break;
                }
                maybe = input_rx.recv() => {
                    match maybe {
                        None => break,
                        Some(b) => {
                            let w = Arc::clone(&writer);
                            let res = tokio::task::spawn_blocking(move || {
                                use std::io::Write;
                                w.lock().unwrap().write_all(&b)
                            })
                            .await;
                            if matches!(res, Err(_) | Ok(Err(_))) {
                                let _ =
                                    tokio::time::timeout(std::time::Duration::from_secs(5), read_handle).await;
                                return Err(DriverError::IoLost("pty write failed".into()));
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            let _ =
                                tokio::time::timeout(std::time::Duration::from_secs(5), read_handle).await;
                            return Err(DriverError::IoLost("local shell exited".into()));
                        }
                        Ok(None) => {}
                        Err(_) => {}
                    }
                }
            }
        }

        let _ = child.kill();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), read_handle).await;
        Ok(())
    }
}
