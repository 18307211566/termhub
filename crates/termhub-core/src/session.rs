use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Stub,
}

pub type StatusTx = mpsc::Sender<SessionStatus>;
pub type StatusRx = mpsc::Receiver<SessionStatus>;
