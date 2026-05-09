use bytes::Bytes;
use termhub_core::{DriverError, DriverEvent, UpstreamDriver};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// 测试用 driver：把所有下联输入直接当作上联输出回灌。
/// 用于 M1 端到端跑通，以及任何不接真实硬件的集成测试。
#[derive(Default)]
pub struct LoopbackDriver;

#[async_trait::async_trait]
impl UpstreamDriver for LoopbackDriver {
    async fn run(
        &mut self,
        mut input_rx: mpsc::Receiver<Bytes>,
        evt_tx: mpsc::Sender<DriverEvent>,
        cancel: CancellationToken,
    ) -> Result<(), DriverError> {
        evt_tx
            .send(DriverEvent::Connected)
            .await
            .map_err(|e| DriverError::Fatal(format!("evt_tx closed: {e}")))?;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    return Ok(());
                }
                maybe = input_rx.recv() => {
                    match maybe {
                        None => return Ok(()),
                        Some(b) => {
                            evt_tx
                                .send(DriverEvent::Output(b))
                                .await
                                .map_err(|e| DriverError::Fatal(format!("evt_tx closed: {e}")))?;
                        }
                    }
                }
            }
        }
    }
}
