use std::borrow::Cow;
use std::net::SocketAddr;
use std::sync::Arc;

use russh::server::{Config as RConfig, Server as RServer};
use russh_keys::key::KeyPair;
use termhub_core::SessionMgr;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::handler::ClientHandler;

pub struct ServerConfig {
    pub listen: String,
    pub host_keys: Vec<KeyPair>,
    pub max_clients_per_session: usize,
}

struct Inner {
    mgr: Arc<SessionMgr>,
    max_clients: usize,
}

impl RServer for Inner {
    type Handler = ClientHandler;

    fn new_client(&mut self, peer_addr: Option<SocketAddr>) -> Self::Handler {
        let peer = peer_addr
            .map(|p| p.to_string())
            .unwrap_or_else(|| "?".into());
        ClientHandler::new(Arc::clone(&self.mgr), self.max_clients, peer)
    }
}

/// Start sshd accept loop with a cancellation token.
/// Returns the task handle and the actual bound address.
/// When `cancel` is fired the accept loop exits (existing client sessions are unaffected).
pub async fn start(
    cfg: ServerConfig,
    mgr: Arc<SessionMgr>,
    cancel: CancellationToken,
) -> anyhow::Result<(JoinHandle<()>, SocketAddr)> {
    let listener = TcpListener::bind(&cfg.listen).await?;
    let addr = listener.local_addr()?;

    let default = RConfig::default();
    let russh_cfg = RConfig {
        keys: cfg.host_keys,
        preferred: russh::Preferred {
            kex: Cow::Borrowed(
                &[
                    russh::kex::CURVE25519,
                    russh::kex::CURVE25519_PRE_RFC_8731,
                    russh::kex::ECDH_SHA2_NISTP256,
                    russh::kex::DH_G16_SHA512,
                    russh::kex::DH_G14_SHA256,
                    russh::kex::DH_G14_SHA1,
                    russh::kex::EXTENSION_SUPPORT_AS_CLIENT,
                    russh::kex::EXTENSION_SUPPORT_AS_SERVER,
                    russh::kex::EXTENSION_OPENSSH_STRICT_KEX_AS_CLIENT,
                    russh::kex::EXTENSION_OPENSSH_STRICT_KEX_AS_SERVER,
                ]
            ),
            key: Cow::Borrowed(
                &[
                    russh_keys::key::ED25519,
                    russh_keys::key::ECDSA_SHA2_NISTP256,
                    russh_keys::key::ECDSA_SHA2_NISTP521,
                    russh_keys::key::RSA_SHA2_512,
                    russh_keys::key::RSA_SHA2_256,
                    russh_keys::key::SSH_RSA,
                ]
            ),
            cipher: default.preferred.cipher.clone(),
            mac: default.preferred.mac.clone(),
            compression: default.preferred.compression.clone(),
        },
        ..default
    };
    let russh_cfg = Arc::new(russh_cfg);

    let mut server = Inner {
        mgr,
        max_clients: cfg.max_clients_per_session,
    };

    let handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                accept_result = listener.accept() => {
                    let (stream, peer) = match accept_result {
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
            }
        }
    });

    Ok((handle, addr))
}
