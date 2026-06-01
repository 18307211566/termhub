use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio_util::sync::CancellationToken;

/// TCP 端口转发：在 `listen` 地址上监听 TCP 连接，
/// 将每个连接的数据双向转发到 `target` 地址。
pub async fn run_tcp_forward(
    listen: &str,
    target: &str,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(listen).await?;
    tracing::info!(%listen, %target, "tcp port forward listening");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!(%listen, "tcp port forward cancelled");
                return Ok(());
            }
            accept = listener.accept() => {
                let (client, remote) = accept?;
                tracing::debug!(%remote, "tcp forward: new connection");
                let target = target.to_string();
                tokio::spawn(async move {
                    if let Err(e) = proxy_tcp(client, &target).await {
                        tracing::debug!(%remote, %target, "tcp forward connection error: {e}");
                    }
                });
            }
        }
    }
}

async fn proxy_tcp(mut client: TcpStream, target: &str) -> anyhow::Result<()> {
    let mut backend = TcpStream::connect(target).await?;
    backend.set_nodelay(true)?;
    client.set_nodelay(true)?;

    let (mut client_rd, mut client_wr) = client.split();
    let (mut backend_rd, mut backend_wr) = backend.split();

    let client_to_backend = async {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = client_rd.read(&mut buf).await?;
            if n == 0 {
                return anyhow::Ok(());
            }
            backend_wr.write_all(&buf[..n]).await?;
        }
    };

    let backend_to_client = async {
        let mut buf = vec![0u8; 8192];
        loop {
            let n = backend_rd.read(&mut buf).await?;
            if n == 0 {
                return anyhow::Ok(());
            }
            client_wr.write_all(&buf[..n]).await?;
        }
    };

    tokio::select! {
        r = client_to_backend => { r?; }
        r = backend_to_client => { r?; }
    }

    Ok(())
}

/// UDP 端口转发：在 `listen` 地址上监听 UDP 数据包，
/// 将数据包转发到 `target` 地址，并将响应回传给发送方。
pub async fn run_udp_forward(
    listen: &str,
    target: &str,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let socket = std::sync::Arc::new(UdpSocket::bind(listen).await?);
    let target_addr = target.to_string();
    tracing::info!(%listen, %target, "udp port forward listening");

    let mut buf = vec![0u8; 65535];
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!(%listen, "udp port forward cancelled");
                return Ok(());
            }
            result = socket.recv_from(&mut buf) => {
                let (n, peer) = result?;
                let data = buf[..n].to_vec();
                let socket_clone = std::sync::Arc::clone(&socket);
                let target_addr = target_addr.clone();
                tokio::spawn(async move {
                    if let Err(e) = proxy_udp(socket_clone, peer, &target_addr, &data).await {
                        tracing::debug!(%peer, %target_addr, "udp forward error: {e}");
                    }
                });
            }
        }
    }
}

async fn proxy_udp(
    socket: std::sync::Arc<UdpSocket>,
    peer: std::net::SocketAddr,
    target: &str,
    data: &[u8],
) -> anyhow::Result<()> {
    // 用一个临时 socket 连接目标
    let backend = UdpSocket::bind("0.0.0.0:0").await?;
    backend.connect(target).await?;
    backend.send(data).await?;

    let mut buf = vec![0u8; 65535];
    // 等待响应，超时 30 秒
    let n = tokio::time::timeout(std::time::Duration::from_secs(30), backend.recv(&mut buf))
        .await
        .map_err(|_| anyhow::anyhow!("udp response timeout"))??;

    socket.send_to(&buf[..n], peer).await?;
    Ok(())
}

/// 兼容旧接口
pub async fn run_http_proxy(
    listen: &str,
    target: &str,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    run_tcp_forward(listen, target, cancel).await
}
