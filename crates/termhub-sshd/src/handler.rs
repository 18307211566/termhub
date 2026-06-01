use std::sync::Arc;

use bytes::Bytes;
use russh::server::{Auth, Handle, Handler, Msg, Session};
use russh::{ChannelId, CryptoVec};
use std::sync::atomic::Ordering;
use termhub_core::{ClientRecord, SessionEntry, SessionMgr};
use tokio::task::JoinHandle;

pub struct ClientHandler {
    mgr: Arc<SessionMgr>,
    max_clients: usize,
    peer: String,
    user: Option<String>,
    entry: Option<SessionEntry>,
    forward_task: Option<JoinHandle<()>>,
    client_rec: Option<Arc<ClientRecord>>,
    is_exec: bool,
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
            is_exec: false,
        }
    }

    /// Common logic for shell_request and exec_request:
    /// attach client, start output-forward task.
    async fn start_shell_or_exec(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
        initial_input: Option<Bytes>,
        is_exec: bool,
    ) {
        let entry = match self.entry.clone() {
            Some(entry) => entry,
            None => {
                session.close(channel);
                return;
            }
        };

        if entry.hub.subscriber_count() >= self.max_clients {
            session.data(
                channel,
                CryptoVec::from_slice(b"*** termhub: session is full\r\n"),
            );
            session.close(channel);
            return;
        }

        session.channel_success(channel);
        let remote = self.peer.clone();
        let rec = entry.clients.attach(remote).await;
        self.client_rec = Some(rec.clone());
        self.is_exec = is_exec;

        // If there's initial input (from exec_request), write it to upstream immediately.
        if let Some(b) = initial_input {
            let _ = entry.hub.send_input(b.clone()).await;
            rec.bytes_in.fetch_add(b.len() as u64, Ordering::Relaxed);
        }

        let mut rx = entry.hub.subscribe_output();
        let handle: Handle = session.handle();
        let kick = rec.kick.clone();

        // For exec channels: if upstream is silent for 3s, assume command finished.
        let silence_timeout = if is_exec {
            std::time::Duration::from_secs(3)
        } else {
            std::time::Duration::from_secs(365 * 24 * 60 * 60)
        };
        let mut sleep = Box::pin(tokio::time::sleep(silence_timeout));

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = kick.cancelled() => {
                        if is_exec {
                            let _ = handle.exit_status_request(channel, 0).await;
                            let _ = handle.close(channel).await;
                        } else {
                            let _ = handle
                                .data(
                                    channel,
                                    CryptoVec::from_slice(b"*** termhub: kicked by host\r\n"),
                                )
                                .await;
                            let _ = handle.close(channel).await;
                        }
                        break;
                    }
                    _ = &mut sleep => {
                        if is_exec {
                            let _ = handle.exit_status_request(channel, 0).await;
                            let _ = handle.close(channel).await;
                        }
                        break;
                    }
                    msg = rx.recv() => match msg {
                        Ok(b) => {
                            if is_exec {
                                sleep.as_mut().reset(tokio::time::Instant::now() + silence_timeout);
                            }
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
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            if is_exec {
                                let _ = handle.exit_status_request(channel, 0).await;
                                let _ = handle.close(channel).await;
                            }
                            break;
                        }
                    },
                }
            }
        });
        self.forward_task = Some(task);
    }
}

#[async_trait::async_trait]
impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        tracing::debug!(%user, "ssh auth attempt");
        match self.mgr.authenticate(user, password).await {
            Some(entry) => {
                tracing::info!(%user, "ssh auth accepted");
                self.user = Some(user.to_string());
                self.entry = Some(entry);
                Ok(Auth::Accept)
            }
            None => {
                tracing::warn!(%user, "ssh auth rejected: wrong user or password");
                Ok(Auth::Reject {
                    proceed_with_methods: None,
                })
            }
        }
    }

    async fn channel_open_session(
        &mut self,
        _channel: russh::Channel<Msg>,
        _session: &mut Session,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(russh::Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel);
        Ok(())
    }

    async fn env_request(
        &mut self,
        _channel: ChannelId,
        _variable_name: &str,
        _variable_value: &str,
        _session: &mut Session,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.start_shell_or_exec(channel, session, None, false)
            .await;
        Ok(())
    }

    async fn exec_request(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        let initial = if data.is_empty() {
            None
        } else {
            let mut v = data.to_vec();
            if !v.ends_with(b"\n") {
                if !v.ends_with(b"\r") {
                    v.push(b'\r');
                }
                v.push(b'\n');
            }
            Some(Bytes::from(v))
        };
        self.start_shell_or_exec(channel, session, initial, true)
            .await;
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
                rec.bytes_in.fetch_add(data.len() as u64, Ordering::Relaxed);
            }
        }
        Ok(())
    }

    async fn channel_eof(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.is_exec {
            // For exec channels, client EOF means no more input.
            // Signal exit and close so the ssh client can terminate cleanly.
            if let Some(task) = self.forward_task.take() {
                task.abort();
            }
            if let (Some(rec), Some(entry)) = (self.client_rec.take(), self.entry.as_ref()) {
                entry.clients.detach(rec.id).await;
            }
            session.exit_status_request(channel, 0);
            session.close(channel);
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
