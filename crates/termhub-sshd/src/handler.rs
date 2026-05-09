use std::sync::Arc;

use bytes::Bytes;
use russh::server::{Auth, Handle, Handler, Msg, Session};
use russh::{ChannelId, CryptoVec};
use termhub_core::{SessionEntry, SessionMgr};
use tokio::task::JoinHandle;

pub struct ClientHandler {
    mgr: Arc<SessionMgr>,
    max_clients: usize,
    user: Option<String>,
    entry: Option<SessionEntry>,
    forward_task: Option<JoinHandle<()>>,
}

impl ClientHandler {
    pub fn new(mgr: Arc<SessionMgr>, max_clients: usize) -> Self {
        Self {
            mgr,
            max_clients,
            user: None,
            entry: None,
            forward_task: None,
        }
    }
}

#[async_trait::async_trait]
impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        match self.mgr.authenticate(user, password).await {
            Some(entry) => {
                self.user = Some(user.to_string());
                self.entry = Some(entry);
                Ok(Auth::Accept)
            }
            None => Ok(Auth::Reject {
                proceed_with_methods: None,
            }),
        }
    }

    async fn channel_open_session(
        &mut self,
        _channel: russh::Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let entry = match self.entry.clone() {
            Some(entry) => entry,
            None => {
                session.close(channel);
                return Ok(());
            }
        };

        if entry.hub.subscriber_count() >= self.max_clients {
            session.data(
                channel,
                CryptoVec::from_slice(b"*** termhub: session is full\r\n"),
            );
            session.close(channel);
            return Ok(());
        }

        session.channel_success(channel);
        let mut rx = entry.hub.subscribe_output();
        let handle: Handle = session.handle();

        let task = tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(bytes) => {
                        if handle
                            .data(channel, CryptoVec::from_slice(&bytes))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let _ = handle
                            .data(
                                channel,
                                CryptoVec::from_slice(
                                    b"\x1b[33m*** termhub: client lagged, output truncated\x1b[0m\r\n",
                                ),
                            )
                            .await;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        self.forward_task = Some(task);

        Ok(())
    }

    async fn data(
        &mut self,
        _channel: ChannelId,
        data: &[u8],
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(entry) = self.entry.as_ref() {
            let _ = entry.hub.send_input(Bytes::copy_from_slice(data)).await;
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        _channel: ChannelId,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(task) = self.forward_task.take() {
            task.abort();
        }
        Ok(())
    }
}

impl Drop for ClientHandler {
    fn drop(&mut self) {
        if let Some(task) = self.forward_task.take() {
            task.abort();
        }
    }
}
