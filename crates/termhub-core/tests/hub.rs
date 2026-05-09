use bytes::Bytes;
use tokio::sync::broadcast::error::TryRecvError;
use termhub_core::Hub;

#[tokio::test]
async fn output_fanout_two_subscribers_each_receive_same_bytes() {
    let (hub, mut up_tx, _up_rx) = Hub::new(1024, 256);
    let h1 = hub.handle();
    let h2 = hub.handle();
    let mut s1 = h1.subscribe_output();
    let mut s2 = h2.subscribe_output();

    up_tx.send(Bytes::from_static(b"hello")).await.unwrap();

    let r1 = s1.recv().await.unwrap();
    let r2 = s2.recv().await.unwrap();
    assert_eq!(&r1[..], b"hello");
    assert_eq!(&r2[..], b"hello");
}

#[tokio::test]
async fn input_fanin_three_subs_arrive_in_order() {
    let (hub, _up_tx, mut up_rx) = Hub::new(1024, 256);
    let h1 = hub.handle();
    let h2 = hub.handle();
    let h3 = hub.handle();

    h1.send_input(Bytes::from_static(b"a")).await.unwrap();
    h2.send_input(Bytes::from_static(b"b")).await.unwrap();
    h3.send_input(Bytes::from_static(b"c")).await.unwrap();

    let mut got = Vec::new();
    for _ in 0..3 {
        got.push(up_rx.recv().await.unwrap());
    }
    let joined: Vec<u8> = got.into_iter().flatten().collect();
    assert_eq!(joined, b"abc");
}

#[tokio::test]
async fn slow_subscriber_lags_does_not_block_others() {
    // 一个订阅者完全不消费，broadcast 容量耗尽后该订阅者将丢老数据，
    // 但其他订阅者必须继续正常收
    let (hub, mut up_tx, _up_rx) = Hub::new(4, 256);
    let h1 = hub.handle();
    let h2 = hub.handle();
    let _slow = h1.subscribe_output();
    let mut fast = h2.subscribe_output();

    for i in 0..32u8 {
        up_tx.send(Bytes::copy_from_slice(&[i])).await.unwrap();
    }
    // fast 应当能收到 *某些* 数据；其中至少最后一个必然到（连续追写后 fast lag 可能也命中，
    // 但因为我们立即 recv 不止一次，最后一个数据点必然在）
    let mut last = 0u8;
    loop {
        match fast.try_recv() {
            Ok(b) => last = b[0],
            Err(TryRecvError::Lagged(_)) => continue,
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Closed) => panic!("broadcast closed unexpectedly"),
        }
    }
    assert_eq!(last, 31);
}
