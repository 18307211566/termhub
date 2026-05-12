use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

/// HTTP 反向代理：在 `listen` 地址上监听 TCP 连接，
/// 将每个连接的数据双向转发到 `target` 地址。
///
/// 适用于 HTTP 文件下载转发等场景——本质上是 TCP 层面的字节透传，
/// 不解析 HTTP 协议，因此兼容任意基于 TCP 的协议。
pub async fn run_http_proxy(
    listen: &str,
    target: &str,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(listen).await?;
    tracing::info!(%listen, %target, "http proxy listening");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!(%listen, "http proxy cancelled");
                return Ok(());
            }
            accept = listener.accept() => {
                let (client, remote) = accept?;
                tracing::debug!(%remote, "http proxy: new connection");
                let target = target.to_string();
                tokio::spawn(async move {
                    if let Err(e) = proxy_connection(client, &target).await {
                        tracing::debug!(%remote, %target, "http proxy connection error: {e}");
                    }
                });
            }
        }
    }
}

async fn proxy_connection(mut client: TcpStream, target: &str) -> anyhow::Result<()> {
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
