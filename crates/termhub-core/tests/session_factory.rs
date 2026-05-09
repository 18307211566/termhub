use std::sync::Arc;

use termhub_core::session_factory::start_session;
use termhub_core::{RunnerConfig, SessionMgr, SessionStatus, UpstreamDriver};
use tokio_util::sync::CancellationToken;

struct Stub;

#[async_trait::async_trait]
impl UpstreamDriver for Stub {
    async fn run(
        &mut self,
        _i: tokio::sync::mpsc::Receiver<bytes::Bytes>,
        _e: tokio::sync::mpsc::Sender<termhub_core::DriverEvent>,
        cancel: CancellationToken,
    ) -> Result<(), termhub_core::DriverError> {
        cancel.cancelled().await;
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn start_session_registers_and_reaches_running() {
    let mgr = Arc::new(SessionMgr::new());
    let started = start_session(
        mgr.clone(),
        "echo".to_string(),
        "pass".to_string(),
        Box::new(|| Box::new(Stub)),
        RunnerConfig {
            auto_reconnect: false,
            max_attempts: None,
        },
    )
    .await
    .unwrap();

    let mut status_rx = started.status_rx;
    for _ in 0..50 {
        if matches!(*status_rx.borrow(), SessionStatus::Running { .. }) {
            break;
        }
        let _ = status_rx.changed().await;
    }
    assert!(matches!(
        *status_rx.borrow(),
        SessionStatus::Running { .. }
    ));
    assert!(mgr.get("echo").await.is_some());

    started.cancel.cancel();
    let _ = started.handle.await;
}
