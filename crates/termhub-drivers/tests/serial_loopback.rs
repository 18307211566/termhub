use std::env;

use bytes::Bytes;
use termhub_core::{DriverEvent, UpstreamDriver};
use termhub_drivers::eol::EolMode;
use termhub_drivers::serial::{parse_serial_params, SerialDriver};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// 通过环境变量 TERMHUB_TEST_COM_A / TERMHUB_TEST_COM_B 注入虚拟串口对
#[ignore]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn serial_echo_via_paired_ports() {
    let port_a = env::var("TERMHUB_TEST_COM_A").expect("set TERMHUB_TEST_COM_A");
    let port_b = env::var("TERMHUB_TEST_COM_B").expect("set TERMHUB_TEST_COM_B");
    let params = parse_serial_params(8, "none", 1, "none").unwrap();

    // driver 端 = COM_A
    let mut drv = SerialDriver {
        port: port_a,
        baud: 115200,
        params: params.clone(),
        input_eol: EolMode::AsIs,
        output_eol: EolMode::AsIs,
    };
    let (in_tx, in_rx) = mpsc::channel(8);
    let (evt_tx, mut evt_rx) = mpsc::channel(8);
    let cancel = CancellationToken::new();
    let cancel2 = cancel.clone();
    let h = tokio::spawn(async move { drv.run(in_rx, evt_tx, cancel2).await });

    matches!(evt_rx.recv().await.unwrap(), DriverEvent::Connected);

    // 用 serialport 同步 API 在 COM_B 上读写：写 "hi"，应当能在 driver 看到
    let mut peer = serialport::new(&port_b, 115200)
        .timeout(std::time::Duration::from_secs(2))
        .open()
        .unwrap();
    use std::io::Write;
    peer.write_all(b"hi").unwrap();

    let evt = evt_rx.recv().await.unwrap();
    match evt {
        DriverEvent::Output(b) => assert_eq!(&b[..], b"hi"),
        other => panic!("expected Output, got {other:?}"),
    }

    // 反向：driver 输入 "ok"，COM_B 应当收到
    in_tx.send(Bytes::from_static(b"ok")).await.unwrap();
    let mut buf = [0u8; 16];
    use std::io::Read;
    let n = peer.read(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"ok");

    cancel.cancel();
    h.await.unwrap().unwrap();
}
