#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::sync::Arc;

use bytes::Bytes;
use termhub_core::{DriverEvent, Hub, SessionMgr, UpstreamDriver};
use termhub_drivers::loopback::LoopbackDriver;
use termhub_sshd::{start as sshd_start, ServerConfig as SshdConfig};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tauri::async_runtime::set(tokio::runtime::Handle::current());

    FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    tracing::info!("termhub starting");

    let mgr = Arc::new(SessionMgr::new());

    let (hub, up_tx, up_rx) = Hub::new(1024, 256);
    let (st, sr) = termhub_core::session::status_channel();
    let cancel = CancellationToken::new();
    mgr.register(
        "echo".to_string(),
        "pass".to_string(),
        hub.handle(),
        sr,
        cancel.clone(),
    )
    .await?;
    spawn_loopback(up_tx, up_rx, cancel.clone());
    let _ = st.send(termhub_core::SessionStatus::Running { uptime_secs: 0 });

    let host_key = russh_keys::key::KeyPair::generate_ed25519().expect("ed25519 host key");
    let (_sshd, addr) = sshd_start(
        SshdConfig {
            listen: "0.0.0.0:2222".to_string(),
            host_key,
            max_clients_per_session: 16,
        },
        mgr.clone(),
    )
    .await?;
    tracing::info!(
        "sshd listening on {addr}, try: ssh echo@127.0.0.1 -p {} (password: pass)",
        addr.port()
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .run(tauri::generate_context!())
        .expect("tauri run");

    Ok(())
}

fn spawn_loopback(
    up_tx: mpsc::Sender<Bytes>,
    up_rx: mpsc::Receiver<Bytes>,
    cancel: CancellationToken,
) {
    let (evt_tx, mut evt_rx) = mpsc::channel::<DriverEvent>(64);
    tokio::spawn(async move {
        while let Some(e) = evt_rx.recv().await {
            if let DriverEvent::Output(b) = e {
                let _ = up_tx.send(b).await;
            }
        }
    });
    tokio::spawn(async move {
        let mut d = LoopbackDriver::default();
        let _ = d.run(up_rx, evt_tx, cancel).await;
    });
}
