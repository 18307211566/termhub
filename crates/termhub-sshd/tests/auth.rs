use std::sync::Arc;

use russh::client;
use termhub_core::{Hub, SessionMgr};
use termhub_sshd::{start, ServerConfig};

struct ClientHandler;

#[async_trait::async_trait]
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn password_auth_accept_or_reject() {
    let mgr = Arc::new(SessionMgr::new());
    let (hub, _u, _v) = Hub::new(1024, 256);
    let (_st, sr) = termhub_core::session::status_channel();
    let cancel = tokio_util::sync::CancellationToken::new();
    mgr.register(
        "echo".to_string(),
        "secret".to_string(),
        hub.handle(),
        sr,
        cancel,
    )
    .await
    .unwrap();

    let host_key = russh_keys::key::KeyPair::generate_ed25519().unwrap();
    let cfg = ServerConfig {
        listen: "127.0.0.1:0".to_string(),
        host_key,
        max_clients_per_session: 16,
    };
    let (handle, addr) = start(cfg, mgr.clone()).await.unwrap();

    let cc = Arc::new(client::Config::default());
    let mut s = client::connect(cc.clone(), addr, ClientHandler).await.unwrap();
    assert!(s.authenticate_password("echo", "secret").await.unwrap());

    let mut s2 = client::connect(cc, addr, ClientHandler).await.unwrap();
    assert!(!s2.authenticate_password("echo", "wrong").await.unwrap());

    handle.abort();
}
