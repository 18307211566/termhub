use termhub_core::{DriverEvent, UpstreamDriver};
use tokio::sync::mpsc;

struct Dummy;

#[async_trait::async_trait]
impl UpstreamDriver for Dummy {
    async fn run(
        &mut self,
        _input_rx: mpsc::Receiver<bytes::Bytes>,
        evt_tx: mpsc::Sender<DriverEvent>,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> Result<(), termhub_core::DriverError> {
        evt_tx.send(DriverEvent::Connected).await.ok();
        Ok(())
    }
}

#[tokio::test]
async fn dummy_driver_emits_connected() {
    let (_input_tx, input_rx) = mpsc::channel(8);
    let (evt_tx, mut evt_rx) = mpsc::channel(8);
    let cancel = tokio_util::sync::CancellationToken::new();
    let mut d = Dummy;
    d.run(input_rx, evt_tx, cancel).await.unwrap();
    let evt = evt_rx.recv().await.expect("expected event");
    assert!(matches!(evt, DriverEvent::Connected));
}
