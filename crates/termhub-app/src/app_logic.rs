use std::sync::Arc;
use std::time::SystemTime;

use serde::Serialize;
use termhub_core::{
    start_session, ClientRecordSnapshot, RunnerConfig, SessionConfig, SessionStatus,
    UpstreamSpec,
};
use termhub_drivers::factory::create_driver;
use termhub_drivers::http_proxy::run_http_proxy;
use tokio_util::sync::CancellationToken;

use crate::state::{AppState, RunnerEntry};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

pub enum AppError {
    NotFound(String),
    Internal(anyhow::Error),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::NotFound(m) => write!(f, "{m}"),
            AppError::Internal(e) => write!(f, "{e}"),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e)
    }
}

// ---------------------------------------------------------------------------
// View types (shared between Tauri IPC and HTTP API)
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
pub struct SessionView {
    pub name: String,
    pub ssh_user: String,
    pub listen: String,
    pub status: SessionStatus,
    pub upstream_kind: String,
    pub upstream_summary: String,
    pub upstream: UpstreamSpec,
    pub auto_reconnect: bool,
    pub client_count: usize,
}

#[derive(Serialize)]
pub struct ClientView {
    pub id: u64,
    pub remote: String,
    pub connected_at_secs: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub host_key_fpr: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn upstream_kind_label(s: &UpstreamSpec) -> String {
    match s {
        UpstreamSpec::Loopback => "loopback",
        UpstreamSpec::Serial { .. } => "serial",
        UpstreamSpec::Ssh { .. } => "ssh",
        UpstreamSpec::Telnet { .. } => "telnet",
        UpstreamSpec::RawTcp { .. } => "raw_tcp",
        UpstreamSpec::LocalShell { .. } => "local_shell",
        UpstreamSpec::HttpProxy { .. } => "http_proxy",
    }
    .to_string()
}

pub fn upstream_summary(s: &UpstreamSpec) -> String {
    match s {
        UpstreamSpec::Loopback => "internal loopback".into(),
        UpstreamSpec::Serial {
            port,
            baud,
            data_bits,
            parity,
            stop_bits,
            flow,
            ..
        } => {
            let p = match parity.as_str() {
                "none" => "N",
                "even" => "E",
                "odd" => "O",
                _ => "?",
            };
            format!("{port} @ {baud} {data_bits}{p}{stop_bits} {flow}")
        }
        UpstreamSpec::Ssh {
            host, port, user, ..
        } => format!("{user}@{host}:{port}"),
        UpstreamSpec::Telnet { host, port } => format!("{host}:{port} (telnet)"),
        UpstreamSpec::RawTcp { host, port } => format!("{host}:{port} (raw)"),
        UpstreamSpec::LocalShell { command, .. } => command.clone(),
        UpstreamSpec::HttpProxy { listen, target } => format!("{listen} -> {target}"),
    }
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

pub async fn persist_now(state: &AppState) -> anyhow::Result<()> {
    let runners = state.runners.read().await;
    let api_listen = state.api_listen.read().await.clone();
    let cfg = termhub_core::Config {
        server: termhub_core::ServerConfig {
            listen: "0.0.0.0:2222".into(),
            host_key_path: "host_key".into(),
            max_clients_per_session: state.max_clients_per_session,
            api_listen,
        },
        ui: termhub_core::UiConfig::default(),
        sessions: runners.values().map(|r| r.config.clone()).collect(),
    };
    termhub_core::save(&state.config_dir, &cfg)
}

// ---------------------------------------------------------------------------
// Business logic
// ---------------------------------------------------------------------------

pub async fn list_sessions(state: &AppState) -> Result<Vec<SessionView>, AppError> {
    let runners = state.runners.read().await;
    let mut out = Vec::new();

    for (_key, entry) in runners.iter() {
        let mgr_entry = match &entry.session_mgr {
            Some(mgr) => mgr.get(&entry.config.ssh_user).await,
            None => None,
        };
        let status = match &mgr_entry {
            Some(e) => e.status.borrow().clone(),
            None => {
                // 不在 mgr 中：已停止或 HTTP 代理
                if entry.proxy_cancel.is_some() {
                    SessionStatus::Running { uptime_secs: 0 }
                } else {
                    SessionStatus::Stopped
                }
            }
        };
        let client_count = match &mgr_entry {
            Some(e) => e.clients.list().await.len(),
            None => 0,
        };
        out.push(SessionView {
            name: entry.config.name.clone(),
            ssh_user: entry.config.ssh_user.clone(),
            listen: entry.config.listen.clone(),
            status,
            upstream_kind: upstream_kind_label(&entry.config.upstream),
            upstream_summary: upstream_summary(&entry.config.upstream),
            upstream: entry.config.upstream.clone(),
            auto_reconnect: entry.config.auto_reconnect,
            client_count,
        });
    }

    Ok(out)
}

/// Returns `(session_name, StatusRx)` so the caller can spawn a status listener.
pub async fn create_session(
    state: &AppState,
    cfg: SessionConfig,
) -> Result<(String, termhub_core::StatusRx), AppError> {
    let name = cfg.name.clone();

    // HTTP 代理模式：不经过 Hub，直接启动 TCP 代理
    if let UpstreamSpec::HttpProxy { listen, target } = &cfg.upstream {
        let cancel = CancellationToken::new();
        let listen = listen.clone();
        let target = target.clone();
        let proxy_cancel = cancel.clone();
        tokio::spawn(async move {
            if let Err(e) = run_http_proxy(&listen, &target, cancel).await {
                tracing::error!(%listen, %target, "http proxy error: {e}");
            }
        });

        // 为 HTTP 代理创建一个虚拟的 status channel
        let (status_tx, status_rx) = termhub_core::session::status_channel();
        let _ = status_tx; // 代理不需要更新状态

        state.runners.write().await.insert(
            name.to_ascii_lowercase(),
            RunnerEntry {
                started: None,
                config: cfg,
                proxy_cancel: Some(proxy_cancel),
                sshd_cancel: None,
                sshd_handle: None,
                session_mgr: None,
            },
        );

        persist_now(state).await?;
        return Ok((name, status_rx));
    }

    // 常规会话模式：通过 Hub + UpstreamDriver + 独立 SSH server
    let password = cfg.password.clone();
    let ssh_user = cfg.ssh_user.clone();
    let listen = cfg.listen.clone();
    let auto_rc = cfg.auto_reconnect;
    let pty = cfg.pty_override.clone();
    let upstream = cfg.upstream.clone();
    let spec_for_factory = cfg.upstream.clone();
    let factory: termhub_core::runner::DriverFactory =
        Box::new(move || create_driver(&spec_for_factory).expect("driver factory"));

    // 1. 创建独立的 SessionMgr（每会话一个）
    let session_mgr = Arc::new(termhub_core::SessionMgr::new());
    let ssh_user_for_auth = cfg.ssh_user.clone();
    let started = start_session(
        session_mgr.clone(),
        name.clone(),
        ssh_user_for_auth,
        password.clone(),
        factory,
        RunnerConfig {
            auto_reconnect: auto_rc,
            max_attempts: None,
        },
    )
    .await?;

    // 2. 为该会话启动独立的 SSH server
    let sshd_cancel = CancellationToken::new();
    let sshd_cfg = termhub_sshd::ServerConfig {
        listen: listen.clone(),
        host_keys: state.host_keys.clone(),
        max_clients_per_session: state.max_clients_per_session,
    };
    let (sshd_handle, _sshd_addr) = termhub_sshd::start(
        sshd_cfg,
        session_mgr.clone(),
        sshd_cancel.clone(),
    )
    .await
    .map_err(|e| AppError::Internal(e))?;

    let status_rx = started.status_rx.clone();
    let cfg_for_runner = SessionConfig {
        name: name.clone(),
        password: cfg.password,
        ssh_user,
        listen,
        auto_reconnect: auto_rc,
        pty_override: pty,
        upstream,
    };
    state.runners.write().await.insert(
        name.to_ascii_lowercase(),
        RunnerEntry {
            started: Some(started),
            config: cfg_for_runner,
            proxy_cancel: None,
            sshd_cancel: Some(sshd_cancel),
            sshd_handle: Some(sshd_handle),
            session_mgr: Some(session_mgr),
        },
    );

    persist_now(state).await?;
    Ok((name, status_rx))
}

pub async fn stop_session(state: &AppState, name: &str) -> Result<(), AppError> {
    let key = name.to_ascii_lowercase();
    // 1. 触发取消
    {
        let runners = state.runners.read().await;
        if let Some(r) = runners.get(&key) {
            if let Some(ref started) = r.started {
                started.cancel.cancel();
            }
            if let Some(ref proxy_cancel) = r.proxy_cancel {
                proxy_cancel.cancel();
            }
            if let Some(ref sshd_cancel) = r.sshd_cancel {
                sshd_cancel.cancel();
            }
        }
    }
    // 2. 等待任务结束并清空 started
    let (runner_handle, sshd_handle) = {
        let mut runners = state.runners.write().await;
        if let Some(r) = runners.get_mut(&key) {
            let rh = r.started.take().map(|s| s.handle);
            let sh = r.sshd_handle.take();
            r.proxy_cancel = None;
            r.sshd_cancel = None;
            (rh, sh)
        } else {
            (None, None)
        }
    };
    if let Some(h) = runner_handle {
        let _ = h.await;
    }
    if let Some(h) = sshd_handle {
        let _ = h.await;
    }
    persist_now(state).await?;
    Ok(())
}

pub async fn delete_session(state: &AppState, name: &str) -> Result<(), AppError> {
    stop_session(state, name).await?;
    let key = name.to_ascii_lowercase();
    state.runners.write().await.remove(&key);
    persist_now(state).await?;
    Ok(())
}

pub async fn restart_session(state: &AppState, name: &str) -> Result<termhub_core::StatusRx, AppError> {
    let key = name.to_ascii_lowercase();
    let cfg = {
        let runners = state.runners.read().await;
        runners.get(&key).map(|r| r.config.clone())
    };
    let Some(cfg) = cfg else {
        return Err(AppError::NotFound("no such session".into()));
    };
    stop_session(state, name).await?;
    let (_, status_rx) = create_session(state, cfg).await?;
    Ok(status_rx)
}

pub async fn update_session(
    state: &AppState,
    old_name: &str,
    cfg: SessionConfig,
) -> Result<termhub_core::StatusRx, AppError> {
    stop_session(state, old_name).await?;
    let (_, status_rx) = create_session(state, cfg).await?;
    Ok(status_rx)
}

pub async fn list_clients(state: &AppState, name: &str) -> Result<Vec<ClientView>, AppError> {
    let key = name.to_ascii_lowercase();
    let runners = state.runners.read().await;
    let runner = match runners.get(&key) {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };
    let entry = match &runner.session_mgr {
        Some(mgr) => mgr.get(&runner.config.ssh_user).await,
        None => return Ok(Vec::new()),
    };
    let v = match entry {
        Some(e) => e.clients.list().await,
        None => return Ok(Vec::new()),
    };
    Ok(v.into_iter()
        .map(|s: ClientRecordSnapshot| ClientView {
            id: s.id,
            remote: s.remote,
            connected_at_secs: s
                .connected_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            bytes_in: s.bytes_in,
            bytes_out: s.bytes_out,
        })
        .collect())
}

pub async fn kick_client(
    state: &AppState,
    session: &str,
    client_id: u64,
) -> Result<bool, AppError> {
    let key = session.to_ascii_lowercase();
    let runners = state.runners.read().await;
    let runner = match runners.get(&key) {
        Some(r) => r,
        None => return Ok(false),
    };
    let entry = match &runner.session_mgr {
        Some(mgr) => mgr.get(&runner.config.ssh_user).await,
        None => return Ok(false),
    };
    match entry {
        Some(e) => Ok(e.clients.kick(client_id).await),
        None => Ok(false),
    }
}

pub async fn get_server_info(state: &AppState) -> Result<ServerInfo, AppError> {
    Ok(ServerInfo {
        host_key_fpr: state.host_key_fpr.clone(),
    })
}

pub fn list_serial_ports() -> Result<Vec<String>, AppError> {
    serialport::available_ports()
        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
        .map_err(|e| AppError::Internal(e.into()))
}

