use serde::{Deserialize, Serialize};
use tokio::sync::watch;

/// 会话状态。Idle 是逻辑值（尚未 start），不会出现在运行中的 watch 通道里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SessionStatus {
    Idle,
    Starting,
    Running { uptime_secs: u64 },
    Reconnecting { attempt: u32 },
    Failed { reason: String },
    Stopped,
}

impl SessionStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped | Self::Failed { .. })
    }
}

pub type StatusTx = watch::Sender<SessionStatus>;
pub type StatusRx = watch::Receiver<SessionStatus>;

pub fn status_channel() -> (StatusTx, StatusRx) {
    watch::channel(SessionStatus::Idle)
}
