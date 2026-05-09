use std::io::Cursor;
use std::sync::Arc;

use bytes::Bytes;
use russh::client;
use russh::ChannelMsg;
use russh_keys::key::PublicKey;
use termhub_core::{DriverError, DriverEvent, UpstreamDriver};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct SshClientDriver {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

struct AcceptAnyKey;

#[async_trait::async_trait]
impl client::Handler for AcceptAnyKey {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _k: &PublicKey) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[async_trait::async_trait]
impl UpstreamDriver for SshClientDriver {
    async fn run(
        &mut self,
        mut input_rx: mpsc::Receiver<Bytes>,
        evt_tx: mpsc::Sender<DriverEvent>,
        cancel: CancellationToken,
    ) -> Result<(), DriverError> {
        let cfg = Arc::new(client::Config::default());
        let addr = (self.host.as_str(), self.port);
        let mut session = client::connect(cfg, addr, AcceptAnyKey)
            .await
            .map_err(|e| DriverError::IoLost(format!("ssh connect: {e}")))?;
        let ok = session
            .authenticate_password(&self.user, &self.password)
            .await
            .map_err(|e| DriverError::IoLost(format!("ssh auth io: {e}")))?;
        if !ok {
            return Err(DriverError::Fatal("ssh auth rejected".into()));
        }
        let mut ch = session
            .channel_open_session()
            .await
            .map_err(|e| DriverError::IoLost(format!("open session: {e}")))?;
        ch.request_pty(false, "xterm-256color", 80, 24, 0, 0, &[])
            .await
            .map_err(|e| DriverError::IoLost(format!("pty: {e}")))?;
        ch.request_shell(false)
            .await
            .map_err(|e| DriverError::IoLost(format!("shell: {e}")))?;

        evt_tx
            .send(DriverEvent::Connected)
            .await
            .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                msg = ch.wait() => {
                    match msg {
                        None => return Err(DriverError::IoLost("ssh channel closed".into())),
                        Some(ChannelMsg::Data { data }) => {
                            evt_tx
                                .send(DriverEvent::Output(Bytes::copy_from_slice(&data)))
                                .await
                                .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;
                        }
                        Some(ChannelMsg::ExtendedData { data, .. }) => {
                            evt_tx
                                .send(DriverEvent::Output(Bytes::copy_from_slice(&data)))
                                .await
                                .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;
                        }
                        Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) => {
                            return Err(DriverError::IoLost("ssh remote closed".into()));
                        }
                        _ => {}
                    }
                }
                maybe = input_rx.recv() => {
                    match maybe {
                        None => return Ok(()),
                        Some(b) => {
                            ch.data(Cursor::new(b.as_ref()))
                                .await
                                .map_err(|e| DriverError::IoLost(format!("ssh write: {e}")))?;
                        }
                    }
                }
            }
        }
    }
}
