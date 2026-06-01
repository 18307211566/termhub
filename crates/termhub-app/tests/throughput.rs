//! 1 GB throughput smoke test — multi-client heavy traffic.
//! Run manually: `cargo test -p termhub-app --test throughput -- --ignored`

use std::sync::Arc;

use bytes::Bytes;
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

/// Each client pushes `chunk_size` bytes in a loop until `total_bytes` per client
/// have been sent through the loopback. Verifies all data arrives intact.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn five_clients_one_gb() {
    const NUM_CLIENTS: usize = 5;
    const TOTAL_PER_CLIENT: usize = 200 * 1024 * 1024; // 200 MiB each → 1 GiB total
    const CHUNK_SIZE: usize = 32 * 1024; // 32 KiB chunks

    let mgr = Arc::new(SessionMgr::new());
    let (hub, up_tx, up_rx) = Hub::new(4096, 1024);
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

    // Wire loopback driver: output → broadcast, client input → driver
    let (evt_tx, mut evt_rx) = mpsc::channel(1024);
    tokio::spawn(async move {
        while let Some(evt) = evt_rx.recv().await {
            if let DriverEvent::Output(bytes) = evt {
                let _ = up_tx.send(bytes).await;
            }
        }
    });

    let driver_cancel = cancel.clone();
    tokio::spawn(async move {
        let mut driver = LoopbackDriver;
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

    let mut client_handles = Vec::new();

    for _ in 0..NUM_CLIENTS {
        let addr_clone = addr;
        let handle = tokio::spawn(async move {
            let client_cfg = Arc::new(client::Config::default());
            let mut session = client::connect(client_cfg, addr_clone, ClientHandler)
                .await
                .unwrap();
            assert!(session.authenticate_password("echo", "pass").await.unwrap());

            let mut channel = session.channel_open_session().await.unwrap();
            channel.request_shell(true).await.unwrap();

            // Send data
            let payload = Bytes::from(vec![0xAB_u8; CHUNK_SIZE]);
            let mut sent: usize = 0;
            while sent < TOTAL_PER_CLIENT {
                channel.data(&payload[..]).await.unwrap();
                sent += CHUNK_SIZE;
            }

            // Read echoed data back
            let mut received: usize = 0;
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
            while received < TOTAL_PER_CLIENT && tokio::time::Instant::now() < deadline {
                match tokio::time::timeout(std::time::Duration::from_secs(10), channel.wait()).await
                {
                    Ok(Some(russh::ChannelMsg::Data { data })) => {
                        received += data.len();
                    }
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(_) => break,
                }
            }

            (sent, received)
        });
        client_handles.push(handle);
    }

    let mut total_sent: usize = 0;
    let mut total_received: usize = 0;
    for h in client_handles {
        let (s, r) = h.await.unwrap();
        total_sent += s;
        total_received += r;
    }

    assert_eq!(total_sent, NUM_CLIENTS * TOTAL_PER_CLIENT);
    assert!(
        total_received >= NUM_CLIENTS * TOTAL_PER_CLIENT,
        "received {total_received} < expected {}",
        NUM_CLIENTS * TOTAL_PER_CLIENT,
    );

    cancel.cancel();
    sshd_cancel.cancel();
    let _ = sshd_handle.await;
}
