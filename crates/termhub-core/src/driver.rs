use bytes::Bytes;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// 上联驱动 trait。每种上联类型（Serial/Ssh/Telnet/RawTcp/LocalShell）实现这个 trait。
///
/// 调用约定：
/// - `run` 是阻塞协程，持续读上联输出 → `DriverEvent::Output`，并消费 `input_rx` 写回上联。
/// - 出现可恢复错误时返回 `Err(DriverError::IoLost(_))`，会话层将进入 `Reconnecting`。
/// - 出现配置错误（如认证失败、串口不存在）返回 `Err(DriverError::Fatal(_))`，会话层进入 `Failed`。
/// - `cancel` 被触发时应尽快返回 `Ok(())`。
#[async_trait::async_trait]
pub trait UpstreamDriver: Send {
    async fn run(
        &mut self,
        input_rx: mpsc::Receiver<Bytes>,
        evt_tx: mpsc::Sender<DriverEvent>,
        cancel: CancellationToken,
    ) -> Result<(), DriverError>;
}

/// driver 在生命周期内推给上层的事件。
#[derive(Debug, Clone)]
pub enum DriverEvent {
    /// 已与上联建立连接（首次或重连成功）
    Connected,
    /// 上联送来的字节
    Output(Bytes),
    /// 想给所有下联打的系统提示行（不带 ANSI 前缀，由上层加色）
    Notice(String),
}

#[derive(Debug, Error)]
pub enum DriverError {
    /// 配置错误或永久错误，不应重试
    #[error("fatal: {0}")]
    Fatal(String),
    /// 可恢复 IO 错误，应进入重连流程
    #[error("io lost: {0}")]
    IoLost(String),
}
