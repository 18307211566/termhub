use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use bytes::Bytes;
use termhub_core::{DriverError, DriverEvent, Hub, SessionStatus, UpstreamDriver};
use termhub_core::runner::{spawn_runner, RunnerConfig};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct AlwaysIoLost {
    counter: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl UpstreamDriver for AlwaysIoLost {
    async fn run(
        &mut self,
        _input_rx: mpsc::Receiver<Bytes>,
        evt_tx: mpsc::Sender<DriverEvent>,
        _cancel: CancellationToken,
    ) -> Result<(), DriverError> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        let _ = evt_tx.send(DriverEvent::Connected).await;
        Err(DriverError::IoLost("forced".into()))
    }
}

struct AlwaysFatal;

#[async_trait::async_trait]
impl UpstreamDriver for AlwaysFatal {
    async fn run(
        &mut self,
        _i: mpsc::Receiver<Bytes>,
        _e: mpsc::Sender<DriverEvent>,
        _c: CancellationToken,
    ) -> Result<(), DriverError> {
        Err(DriverError::Fatal("nope".into()))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn iolost_with_reconnect_eventually_runs_again() {
    let counter = Arc::new(AtomicU32::new(0));
    let counter2 = counter.clone();
    let factory = move || -> Box<dyn UpstreamDriver> {
        Box::new(AlwaysIoLost {
            counter: counter2.clone(),
        })
    };
    let (hub, up_tx, up_rx) = Hub::new(1024, 256);
    let (st, sr) = termhub_core::session::status_channel();
    let cancel = CancellationToken::new();

    let cfg = RunnerConfig {
        auto_reconnect: true,
        max_attempts: Some(3),
    };
    let h = spawn_runner(Box::new(factory), hub, up_tx, up_rx, st, cancel.clone(), cfg);

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
        if matches!(
            *sr.borrow(),
            SessionStatus::Stopped | SessionStatus::Failed { .. }
        ) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(counter.load(Ordering::SeqCst) >= 2, "driver was retried");
    cancel.cancel();
    h.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fatal_does_not_retry() {
    let factory = || -> Box<dyn UpstreamDriver> { Box::new(AlwaysFatal) };
    let (hub, up_tx, up_rx) = Hub::new(1024, 256);
    let (st, sr) = termhub_core::session::status_channel();
    let cancel = CancellationToken::new();
    let cfg = RunnerConfig {
        auto_reconnect: true,
        max_attempts: None,
    };
    let h = spawn_runner(Box::new(factory), hub, up_tx, up_rx, st, cancel.clone(), cfg);

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut saw_failed = false;
    while tokio::time::Instant::now() < deadline {
        if matches!(*sr.borrow(), SessionStatus::Failed { .. }) {
            saw_failed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(saw_failed);
    cancel.cancel();
    h.await.unwrap();
}
