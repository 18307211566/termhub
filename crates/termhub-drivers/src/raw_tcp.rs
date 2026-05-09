use bytes::Bytes;
use termhub_core::{DriverError, DriverEvent, UpstreamDriver};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct RawTcpDriver {
    pub host: String,
    pub port: u16,
}

#[async_trait::async_trait]
impl UpstreamDriver for RawTcpDriver {
    async fn run(
        &mut self,
        mut input_rx: mpsc::Receiver<Bytes>,
        evt_tx: mpsc::Sender<DriverEvent>,
        cancel: CancellationToken,
    ) -> Result<(), DriverError> {
        let stream = TcpStream::connect((self.host.as_str(), self.port))
            .await
            .map_err(|e| DriverError::IoLost(format!("tcp connect: {e}")))?;
        let _ = stream.set_nodelay(true);
        let (mut rd, mut wr) = stream.into_split();
        evt_tx
            .send(DriverEvent::Connected)
            .await
            .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;

        let mut buf = vec![0u8; 4096];
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                n = rd.read(&mut buf) => {
                    let n = n.map_err(|e| DriverError::IoLost(format!("tcp read: {e}")))?;
                    if n == 0 {
                        return Err(DriverError::IoLost("tcp EOF".into()));
                    }
                    evt_tx
                        .send(DriverEvent::Output(Bytes::copy_from_slice(&buf[..n])))
                        .await
                        .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;
                }
                maybe = input_rx.recv() => {
                    match maybe {
                        None => return Ok(()),
                        Some(b) => {
                            wr.write_all(&b)
                                .await
                                .map_err(|e| DriverError::IoLost(format!("tcp write: {e}")))?;
                        }
                    }
                }
            }
        }
    }
}
