use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("stub driver error")]
    Stub,
}

#[derive(Debug, Clone)]
pub enum DriverEvent {
    Stub,
}

#[async_trait]
pub trait UpstreamDriver: Send {
    async fn run(&mut self) -> Result<(), DriverError>;
}
