#![cfg(target_os = "windows")]

use termhub_core::{DriverEvent, UpstreamDriver};
use termhub_drivers::local_shell::LocalShellDriver;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cmd_echo_runs_and_outputs() {
    let (in_tx, in_rx) = mpsc::channel(8);
    let (evt_tx, mut evt_rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();

    let cancel2 = cancel.clone();
    let h = tokio::spawn(async move {
        let mut d = LocalShellDriver {
            command: "cmd.exe".into(),
            args: vec!["/c".into(), "echo hello".into()],
            cols: 80,
            rows: 24,
        };
        d.run(in_rx, evt_tx, cancel2).await
    });

    matches!(evt_rx.recv().await.unwrap(), DriverEvent::Connected);

    let mut got = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(500), evt_rx.recv()).await {
            Ok(Some(DriverEvent::Output(b))) => got.extend_from_slice(&b),
            Ok(Some(DriverEvent::Connected)) | Ok(Some(DriverEvent::Notice(_))) => {}
            Ok(None) => break,
            Err(_) => {}
        }
        if got.windows(5).any(|w| w == b"hello") {
            break;
        }
    }
    drop(in_tx);
    cancel.cancel();
    let _ = h.await;

    assert!(
        got.windows(5).any(|w| w == b"hello"),
        "got = {:?}",
        String::from_utf8_lossy(&got)
    );
}
