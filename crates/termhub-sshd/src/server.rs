use std::net::SocketAddr;
use std::sync::Arc;

use russh::server::{Config as RConfig, Server as RServer};
use russh_keys::key::KeyPair;
use termhub_core::SessionMgr;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use crate::handler::ClientHandler;

pub struct ServerConfig {
    pub listen: String,
    pub host_key: KeyPair,
    pub max_clients_per_session: usize,
}

struct Inner {
    mgr: Arc<SessionMgr>,
    max_clients: usize,
}

impl RServer for Inner {
    type Handler = ClientHandler;

    fn new_client(&mut self, _peer_addr: Option<SocketAddr>) -> Self::Handler {
        ClientHandler::new(Arc::clone(&self.mgr), self.max_clients)
    }
}

/// Start sshd in the background and return its task plus the bound address.
pub async fn start(
    cfg: ServerConfig,
    mgr: Arc<SessionMgr>,
) -> anyhow::Result<(JoinHandle<()>, SocketAddr)> {
    let listener = TcpListener::bind(&cfg.listen).await?;
    let addr = listener.local_addr()?;

    let mut russh_cfg = RConfig::default();
    russh_cfg.keys.push(cfg.host_key);
    let russh_cfg = Arc::new(russh_cfg);

    let mut server = Inner {
        mgr,
        max_clients: cfg.max_clients_per_session,
    };

    let handle = tokio::spawn(async move {
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(accepted) => accepted,
                Err(error) => {
                    tracing::warn!(?error, "ssh accept failed");
                    continue;
                }
            };

            let cfg = Arc::clone(&russh_cfg);
            let handler = server.new_client(Some(peer));
            tokio::spawn(async move {
                if let Err(error) = russh::server::run_stream(cfg, stream, handler).await {
                    tracing::debug!(?peer, ?error, "ssh client session ended");
                }
            });
        }
    });

    Ok((handle, addr))
}
