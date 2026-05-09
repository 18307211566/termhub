use bytes::{Bytes, BytesMut};
use termhub_core::{DriverError, DriverEvent, UpstreamDriver};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};
use tokio_util::sync::CancellationToken;

use crate::eol::{transform, EolMode};

#[derive(Debug, Clone)]
pub struct SerialParams {
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,
    pub flow: FlowControl,
}

pub fn parse_serial_params(
    data_bits: u8,
    parity: &str,
    stop_bits: u8,
    flow: &str,
) -> anyhow::Result<SerialParams> {
    let data_bits = match data_bits {
        5 => DataBits::Five,
        6 => DataBits::Six,
        7 => DataBits::Seven,
        8 => DataBits::Eight,
        n => anyhow::bail!("invalid data_bits: {n}"),
    };
    let parity = match parity {
        "none" => Parity::None,
        "even" => Parity::Even,
        "odd" => Parity::Odd,
        s => anyhow::bail!("invalid parity: {s}"),
    };
    let stop_bits = match stop_bits {
        1 => StopBits::One,
        2 => StopBits::Two,
        n => anyhow::bail!("invalid stop_bits: {n}"),
    };
    let flow = match flow {
        "none" => FlowControl::None,
        "hardware" => FlowControl::Hardware,
        "software" => FlowControl::Software,
        s => anyhow::bail!("invalid flow: {s}"),
    };
    Ok(SerialParams {
        data_bits,
        parity,
        stop_bits,
        flow,
    })
}

pub struct SerialDriver {
    pub port: String,
    pub baud: u32,
    pub params: SerialParams,
    pub input_eol: EolMode,
    pub output_eol: EolMode,
}

#[async_trait::async_trait]
impl UpstreamDriver for SerialDriver {
    async fn run(
        &mut self,
        mut input_rx: mpsc::Receiver<Bytes>,
        evt_tx: mpsc::Sender<DriverEvent>,
        cancel: CancellationToken,
    ) -> Result<(), DriverError> {
        let stream = tokio_serial::new(&self.port, self.baud)
            .data_bits(self.params.data_bits)
            .parity(self.params.parity)
            .stop_bits(self.params.stop_bits)
            .flow_control(self.params.flow)
            .open_native_async()
            .map_err(|e| classify_open_error(e, &self.port))?;
        let (mut rd, mut wr) = tokio::io::split(stream);

        evt_tx
            .send(DriverEvent::Connected)
            .await
            .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;

        let mut buf = BytesMut::with_capacity(4096);

        loop {
            buf.resize(4096, 0);
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                n = rd.read(&mut buf) => {
                    let n = n.map_err(|e| DriverError::IoLost(format!("serial read: {e}")))?;
                    if n == 0 {
                        return Err(DriverError::IoLost("serial EOF".into()));
                    }
                    let chunk = buf.split_to(n).freeze();
                    let chunk = transform(self.output_eol, chunk);
                    evt_tx
                        .send(DriverEvent::Output(chunk))
                        .await
                        .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;
                }
                maybe = input_rx.recv() => {
                    match maybe {
                        None => return Ok(()),
                        Some(b) => {
                            let b = transform(self.input_eol, b);
                            wr.write_all(&b)
                                .await
                                .map_err(|e| DriverError::IoLost(format!("serial write: {e}")))?;
                        }
                    }
                }
            }
        }
    }
}

fn classify_open_error(e: tokio_serial::Error, port: &str) -> DriverError {
    use tokio_serial::ErrorKind::*;
    match e.kind {
        NoDevice => DriverError::Fatal(format!("no such serial device: {port}")),
        InvalidInput => DriverError::Fatal("invalid serial parameters".to_string()),
        Io(_) => DriverError::IoLost(format!("serial open io: {e}")),
        Unknown => DriverError::IoLost(format!("serial open: {e}")),
    }
}
