use bytes::Bytes;
use termhub_core::{DriverEvent, UpstreamDriver};
use termhub_drivers::loopback::LoopbackDriver;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loopback_echoes_input_back() {
    let (in_tx, in_rx) = mpsc::channel(8);
    let (evt_tx, mut evt_rx) = mpsc::channel(8);
    let cancel = CancellationToken::new();

    let cancel2 = cancel.clone();
    let h = tokio::spawn(async move {
        let mut d = LoopbackDriver::default();
        d.run(in_rx, evt_tx, cancel2).await
    });

    // 应该先收到 Connected
    assert!(matches!(
        evt_rx.recv().await.unwrap(),
        DriverEvent::Connected
    ));

    in_tx.send(Bytes::from_static(b"hello")).await.unwrap();
    let out = evt_rx.recv().await.unwrap();
    match out {
        DriverEvent::Output(b) => assert_eq!(&b[..], b"hello"),
        other => panic!("expected Output, got {other:?}"),
    }

    cancel.cancel();
    h.await.unwrap().unwrap();
}
