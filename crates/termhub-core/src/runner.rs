use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::backoff::Backoff;
use crate::driver::{DriverError, DriverEvent, UpstreamDriver};
use crate::hub::Hub;
use crate::session::{SessionStatus, StatusTx};

pub struct RunnerConfig {
    pub auto_reconnect: bool,
    pub max_attempts: Option<u32>,
}

pub type DriverFactory = Box<dyn Fn() -> Box<dyn UpstreamDriver> + Send + Sync + 'static>;

pub fn spawn_runner(
    factory: DriverFactory,
    hub: Hub,
    up_tx: mpsc::Sender<Bytes>,
    up_rx: mpsc::Receiver<Bytes>,
    status: StatusTx,
    cancel: CancellationToken,
    cfg: RunnerConfig,
) -> JoinHandle<()> {
    let up_rx_shared = Arc::new(tokio::sync::Mutex::new(up_rx));
    tokio::spawn(async move {
        run_loop(
            factory,
            hub,
            up_tx,
            up_rx_shared,
            status,
            cancel,
            cfg,
        )
        .await;
    })
}

async fn run_loop(
    factory: DriverFactory,
    hub: Hub,
    up_tx: mpsc::Sender<Bytes>,
    up_rx_shared: Arc<tokio::sync::Mutex<mpsc::Receiver<Bytes>>>,
    status: StatusTx,
    cancel: CancellationToken,
    cfg: RunnerConfig,
) {
    let hub_handle = hub.handle();
    let _ = status.send(SessionStatus::Starting);
    let start_time = Instant::now();
    let backoff = Arc::new(std::sync::Mutex::new(Backoff::default()));

    // 定期更新 Running 状态的运行时间
    let uptime_status = status.clone();
    let uptime_cancel = cancel.clone();
    let uptime_start = start_time;
    let uptime_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = uptime_cancel.cancelled() => break,
                _ = interval.tick() => {
                    let _ = uptime_status.send(SessionStatus::Running {
                        uptime_secs: uptime_start.elapsed().as_secs(),
                    });
                }
            }
        }
    });

    loop {
        if cancel.is_cancelled() {
            uptime_task.abort();
            let _ = status.send(SessionStatus::Stopped);
            break;
        }

        let (drv_in_tx, drv_in_rx) = mpsc::channel::<Bytes>(256);
        let (evt_tx, mut evt_rx) = mpsc::channel::<DriverEvent>(256);

        let backoff_evt = Arc::clone(&backoff);
        let evt_pump = {
            let up_tx2 = up_tx.clone();
            let h = hub_handle.clone();
            tokio::spawn(async move {
                while let Some(e) = evt_rx.recv().await {
                    match e {
                        DriverEvent::Connected => {
                            backoff_evt.lock().unwrap().reset();
                        }
                        DriverEvent::Output(b) => {
                            let _ = up_tx2.send(b).await;
                        }
                        DriverEvent::Notice(s) => {
                            let line = format!("\x1b[33m*** termhub: {s}\x1b[0m\r\n");
                            h.broadcast_system(Bytes::copy_from_slice(line.as_bytes()));
                        }
                    }
                }
            })
        };

        let bridge_cancel = CancellationToken::new();
        let bridge = {
            let up_rx_shared = Arc::clone(&up_rx_shared);
            let drv_in_tx = drv_in_tx.clone();
            let bc = bridge_cancel.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = bc.cancelled() => break,
                        maybe = async {
                            let mut g = up_rx_shared.lock().await;
                            g.recv().await
                        } => {
                            match maybe {
                                None => break,
                                Some(b) => {
                                    if drv_in_tx.send(b).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            })
        };

        let cancel_drv = cancel.clone();
        let mut drv = factory();
        let drv_join = tokio::spawn(async move {
            drv.run(drv_in_rx, evt_tx, cancel_drv).await
        });

        let _ = status.send(SessionStatus::Running {
            uptime_secs: start_time.elapsed().as_secs(),
        });

        let driver_res = drv_join
            .await
            .unwrap_or_else(|e| Err(DriverError::IoLost(format!("driver join: {e}"))));

        bridge_cancel.cancel();
        let _ = bridge.await;
        evt_pump.abort();

        match driver_res {
            Ok(()) => {
                uptime_task.abort();
                let _ = status.send(SessionStatus::Stopped);
                break;
            }
            Err(DriverError::Fatal(reason)) => {
                uptime_task.abort();
                let _ = status.send(SessionStatus::Failed { reason });
                break;
            }
            Err(DriverError::IoLost(reason)) => {
                if !cfg.auto_reconnect {
                    uptime_task.abort();
                    let _ = status.send(SessionStatus::Stopped);
                    let _ = reason;
                    break;
                }
                hub_handle.broadcast_system(Bytes::from_static(
                    b"\x1b[33m*** termhub: upstream lost, reconnecting...\x1b[0m\r\n",
                ));
                let (delay, attempt) = {
                    let mut g = backoff.lock().unwrap();
                    let d = g.next_delay();
                    let a = g.attempt();
                    (d, a)
                };
                let _ = status.send(SessionStatus::Reconnecting { attempt });
                if let Some(max) = cfg.max_attempts {
                    if attempt > max {
                        uptime_task.abort();
                        let _ = status.send(SessionStatus::Failed {
                            reason: format!("max retries exceeded ({reason})"),
                        });
                        break;
                    }
                }
                tokio::select! {
                    _ = cancel.cancelled() => {
                        uptime_task.abort();
                        let _ = status.send(SessionStatus::Stopped);
                        break;
                    }
                    _ = tokio::time::sleep(delay) => {}
                }
            }
        }
    }
}
