use bytes::{Bytes, BytesMut};
use termhub_core::{DriverError, DriverEvent, UpstreamDriver};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const IAC: u8 = 0xFF;
const DONT: u8 = 0xFE;
const DO: u8 = 0xFD;
const WONT: u8 = 0xFC;
const WILL: u8 = 0xFB;
const SB: u8 = 0xFA;
const SE: u8 = 0xF0;

/// 处理输入流首部一个 IAC 命令块。
pub fn process_iac_block(input: &[u8]) -> (usize, Vec<u8>) {
    if input.len() < 2 || input[0] != IAC {
        return (0, vec![]);
    }
    match input[1] {
        IAC => (2, vec![]),
        DO => simple3(input, WONT),
        DONT => simple3(input, WONT),
        WILL => simple3(input, DONT),
        WONT => simple3(input, DONT),
        SB => {
            let mut i = 2;
            while i + 1 < input.len() {
                if input[i] == IAC && input[i + 1] == SE {
                    return (i + 2, vec![]);
                }
                i += 1;
            }
            (0, vec![])
        }
        _ => (2, vec![]),
    }
}

fn simple3(input: &[u8], reply_verb: u8) -> (usize, Vec<u8>) {
    if input.len() < 3 {
        return (0, vec![]);
    }
    (3, vec![IAC, reply_verb, input[2]])
}

pub struct TelnetDriver {
    pub host: String,
    pub port: u16,
}

#[async_trait::async_trait]
impl UpstreamDriver for TelnetDriver {
    async fn run(
        &mut self,
        mut input_rx: mpsc::Receiver<Bytes>,
        evt_tx: mpsc::Sender<DriverEvent>,
        cancel: CancellationToken,
    ) -> Result<(), DriverError> {
        let stream = TcpStream::connect((self.host.as_str(), self.port))
            .await
            .map_err(|e| DriverError::IoLost(format!("telnet connect: {e}")))?;
        let _ = stream.set_nodelay(true);
        let (mut rd, mut wr) = stream.into_split();

        evt_tx
            .send(DriverEvent::Connected)
            .await
            .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;

        let mut tail = BytesMut::new();
        let mut read_buf = vec![0u8; 4096];

        loop {
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                n = rd.read(&mut read_buf) => {
                    let n = n.map_err(|e| DriverError::IoLost(format!("telnet read: {e}")))?;
                    if n == 0 {
                        return Err(DriverError::IoLost("telnet EOF".into()));
                    }
                    tail.extend_from_slice(&read_buf[..n]);
                    let mut data_out = BytesMut::new();
                    let mut i = 0;
                    while i < tail.len() {
                        if tail[i] == IAC {
                            let (consumed, reply) = process_iac_block(&tail[i..]);
                            if consumed == 0 {
                                break;
                            }
                            if !reply.is_empty() {
                                wr.write_all(&reply).await.map_err(|e| {
                                    DriverError::IoLost(format!("telnet write: {e}"))
                                })?;
                            }
                            if consumed == 2 && tail[i + 1] == IAC {
                                data_out.extend_from_slice(&[0xFF]);
                            }
                            i += consumed;
                        } else {
                            data_out.extend_from_slice(&[tail[i]]);
                            i += 1;
                        }
                    }
                    let leftover = tail.split_off(i);
                    tail = leftover;
                    if !data_out.is_empty() {
                        evt_tx
                            .send(DriverEvent::Output(data_out.freeze()))
                            .await
                            .map_err(|e| DriverError::Fatal(format!("evt closed: {e}")))?;
                    }
                }
                maybe = input_rx.recv() => {
                    match maybe {
                        None => return Ok(()),
                        Some(b) => {
                            let mut escaped: Vec<u8> = Vec::with_capacity(b.len());
                            for byte in &b[..] {
                                if *byte == IAC {
                                    escaped.extend_from_slice(&[IAC, IAC]);
                                } else {
                                    escaped.push(*byte);
                                }
                            }
                            wr.write_all(&escaped).await.map_err(|e| {
                                DriverError::IoLost(format!("telnet write: {e}"))
                            })?;
                        }
                    }
                }
            }
        }
    }
}
