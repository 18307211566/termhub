use std::sync::Arc;

use russh::client;
use termhub_core::{DriverEvent, Hub, SessionMgr, UpstreamDriver};
use termhub_drivers::loopback::LoopbackDriver;
use termhub_sshd::{start, ServerConfig};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
async fn e2e_loopback_echo() {
    let mgr = Arc::new(SessionMgr::new());
    let (hub, up_tx, up_rx) = Hub::new(1024, 256);
    let (status_tx, status_rx) = termhub_core::session::status_channel();
    let cancel = CancellationToken::new();
    mgr.register(
        "echo".to_string(),
        "pass".to_string(),
        hub.handle(),
        status_rx,
        cancel.clone(),
    )
    .await
    .unwrap();

    let (evt_tx, mut evt_rx) = mpsc::channel(64);
    tokio::spawn(async move {
        while let Some(evt) = evt_rx.recv().await {
            if let DriverEvent::Output(bytes) = evt {
                let _ = up_tx.send(bytes).await;
            }
        }
    });

    let driver_cancel = cancel.clone();
    tokio::spawn(async move {
        let mut driver = LoopbackDriver::default();
        let _ = driver.run(up_rx, evt_tx, driver_cancel).await;
    });
    let _ = status_tx.send(termhub_core::SessionStatus::Running { uptime_secs: 0 });

    let host_key = russh_keys::key::KeyPair::generate_ed25519().unwrap();
    let sshd_cancel = CancellationToken::new();
    let (sshd_handle, addr) = start(
        ServerConfig {
            listen: "127.0.0.1:0".to_string(),
            host_keys: vec![host_key],
            max_clients_per_session: 16,
        },
        mgr.clone(),
        sshd_cancel.clone(),
    )
    .await
    .unwrap();

    let client_cfg = Arc::new(client::Config::default());
    let mut session = client::connect(client_cfg, addr, ClientHandler)
        .await
        .unwrap();
    assert!(session.authenticate_password("echo", "pass").await.unwrap());

    let mut channel = session.channel_open_session().await.unwrap();
    channel.request_shell(true).await.unwrap();
    channel.data(&b"hi"[..]).await.unwrap();

    let mut got = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    while got.len() < 2 && tokio::time::Instant::now() < deadline {
        if let Some(msg) = channel.wait().await {
            if let russh::ChannelMsg::Data { ref data } = msg {
                got.extend_from_slice(data);
            }
        }
    }

    assert!(got.windows(2).any(|window| window == b"hi"), "got = {got:?}");

    cancel.cancel();
    sshd_cancel.cancel();
    let _ = sshd_handle.await;
}
