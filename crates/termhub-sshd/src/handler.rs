use std::sync::Arc;

use bytes::Bytes;
use russh::server::{Auth, Handle, Handler, Msg, Session};
use russh::{ChannelId, CryptoVec};
use termhub_core::{ClientRecord, SessionEntry, SessionMgr};
use std::sync::atomic::Ordering;
use tokio::task::JoinHandle;

pub struct ClientHandler {
    mgr: Arc<SessionMgr>,
    max_clients: usize,
    peer: String,
    user: Option<String>,
    entry: Option<SessionEntry>,
    forward_task: Option<JoinHandle<()>>,
    client_rec: Option<Arc<ClientRecord>>,
}

impl ClientHandler {
    pub fn new(mgr: Arc<SessionMgr>, max_clients: usize, peer: String) -> Self {
        Self {
            mgr,
            max_clients,
            peer,
            user: None,
            entry: None,
            forward_task: None,
            client_rec: None,
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
        let remote = self.peer.clone();
        let rec = entry.clients.attach(remote).await;
        self.client_rec = Some(rec.clone());

        let mut rx = entry.hub.subscribe_output();
        let handle: Handle = session.handle();
        let kick = rec.kick.clone();

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = kick.cancelled() => {
                        let _ = handle
                            .data(
                                channel,
                                CryptoVec::from_slice(b"*** termhub: kicked by host\r\n"),
                            )
                            .await;
                        let _ = handle.close(channel).await;
                        break;
                    }
                    msg = rx.recv() => match msg {
                        Ok(b) => {
                            rec.bytes_out
                                .fetch_add(b.len() as u64, Ordering::Relaxed);
                            if handle
                                .data(channel, CryptoVec::from_slice(&b))
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
                    },
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
            if let Some(rec) = self.client_rec.as_ref() {
                rec.bytes_in
                    .fetch_add(data.len() as u64, Ordering::Relaxed);
            }
        }
        Ok(())
    }

    async fn channel_close(
        &mut self,
        _channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if let Some(task) = self.forward_task.take() {
            task.abort();
        }
        if let (Some(rec), Some(entry)) = (self.client_rec.take(), self.entry.as_ref()) {
            entry.clients.detach(rec.id).await;
        }
        let _ = session;
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
