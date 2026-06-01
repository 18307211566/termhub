use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use bytes::Bytes;
use tokio::sync::{broadcast, mpsc};

/// 订阅者 ID（每个下联 SSH 通道一份）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubscriberId(pub u64);

/// 一个会话的核心枢纽。每个 `Session` 拥有一个 `Hub`。
///
/// 数据流：
/// - 上联输出：`Hub` 拥有 `broadcast::Sender<Bytes>`；上层把 `up_tx` 交给 driver 用来推字节
/// - 下联输入：所有下联往 `mpsc::Sender<Bytes>` 写，driver 从 `up_rx` 读
pub struct Hub {
    out_tx: broadcast::Sender<Bytes>,
    in_tx: mpsc::Sender<Bytes>,
    next_id: Arc<AtomicU64>,
}

impl Hub {
    /// 新建 Hub。
    /// - `out_capacity`：broadcast 通道容量（消费不过来时丢老数据）
    /// - `in_capacity`：mpsc 通道容量
    ///
    /// 返回 `(Hub, up_tx, up_rx)`：driver 拿到 `up_tx` 写上联输出，从 `up_rx` 读下联输入
    pub fn new(
        out_capacity: usize,
        in_capacity: usize,
    ) -> (Self, mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) {
        let (out_tx, _out_rx) = broadcast::channel(out_capacity);
        let (in_tx, in_rx) = mpsc::channel(in_capacity);
        // 把"driver 用的 up_tx"做成一个独立的 sender：实际上我们让 driver 把 Bytes
        // 通过另一个 mpsc 提交给 hub 内部循环，hub 再 broadcast——但这样多一跳。
        // 简化：driver 直接持有 broadcast::Sender 的克隆，自己 send。
        // 因此 up_tx 的"实际类型"我们用一个本地 mpsc 包一层，让 hub 单独 spawn forward task。
        let driver_out_tx = make_forward(out_tx.clone());
        (
            Self {
                out_tx,
                in_tx,
                next_id: Arc::new(AtomicU64::new(0)),
            },
            driver_out_tx,
            in_rx,
        )
    }

    /// 给下联使用的轻量句柄；可任意 clone
    pub fn handle(&self) -> HubHandle {
        HubHandle {
            out_tx: self.out_tx.clone(),
            in_tx: self.in_tx.clone(),
            next_id: Arc::clone(&self.next_id),
        }
    }

    /// 关闭 Hub：所有 broadcast 订阅者收到 `Err(Closed)`，所有 mpsc 发送方收到 `SendError`
    pub fn shutdown(self) {
        drop(self.out_tx);
        drop(self.in_tx);
    }
}

/// `HubHandle`：给下联（SSH server channel）和系统消息发送者使用
#[derive(Clone)]
pub struct HubHandle {
    out_tx: broadcast::Sender<Bytes>,
    in_tx: mpsc::Sender<Bytes>,
    next_id: Arc<AtomicU64>,
}

impl HubHandle {
    pub fn alloc_id(&self) -> SubscriberId {
        SubscriberId(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    /// 订阅上联输出
    pub fn subscribe_output(&self) -> broadcast::Receiver<Bytes> {
        self.out_tx.subscribe()
    }

    /// 推一段字节到下联输入（被汇聚后送往上联）
    pub async fn send_input(&self, b: Bytes) -> Result<(), mpsc::error::SendError<Bytes>> {
        self.in_tx.send(b).await
    }

    /// 当前订阅者数量（用于 metrics）
    pub fn subscriber_count(&self) -> usize {
        self.out_tx.receiver_count()
    }

    /// 直接向所有订阅者广播一段字节（用于系统提示行）
    pub fn broadcast_system(&self, b: Bytes) {
        // broadcast 满会丢老数据，对系统行可接受
        let _ = self.out_tx.send(b);
    }
}

/// 把 driver 的 `mpsc::Sender<Bytes>` 包成一层：driver 调 send 后，
/// 内部 task 把数据 forward 进 broadcast。这样 driver 不需要直接持有 broadcast::Sender。
fn make_forward(out_tx: broadcast::Sender<Bytes>) -> mpsc::Sender<Bytes> {
    let (tx, mut rx) = mpsc::channel::<Bytes>(256);
    tokio::spawn(async move {
        while let Some(b) = rx.recv().await {
            // broadcast 满则丢老的；不会阻塞
            let _ = out_tx.send(b);
        }
    });
    tx
}
