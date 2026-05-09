use bytes::Bytes;
use termhub_core::{DriverEvent, UpstreamDriver};
use termhub_drivers::raw_tcp::RawTcpDriver;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn raw_tcp_full_duplex_via_loopback_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut s, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 16];
        let n = s.read(&mut buf).await.unwrap();
        let mut out = Vec::new();
        out.extend_from_slice(b"echo:");
        out.extend_from_slice(&buf[..n]);
        s.write_all(&out).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    });

    let (in_tx, in_rx) = mpsc::channel(8);
    let (evt_tx, mut evt_rx) = mpsc::channel(8);
    let cancel = CancellationToken::new();

    let cancel2 = cancel.clone();
    let h = tokio::spawn(async move {
        let mut d = RawTcpDriver {
            host: addr.ip().to_string(),
            port: addr.port(),
        };
        d.run(in_rx, evt_tx, cancel2).await
    });

    matches!(evt_rx.recv().await.unwrap(), DriverEvent::Connected);
    in_tx.send(Bytes::from_static(b"abc")).await.unwrap();
    let evt = evt_rx.recv().await.unwrap();
    let DriverEvent::Output(b) = evt else {
        panic!("expected Output");
    };
    assert_eq!(&b[..], b"echo:abc");

    cancel.cancel();
    let _ = h.await;
    let _ = server.await;
}
