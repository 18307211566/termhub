use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use termhub_core::{
    start_session, ClientRecordSnapshot, RunnerConfig, SessionConfig, SessionStatus,
    UpstreamSpec,
};
use termhub_drivers::factory::create_driver;

use crate::state::{AppState, RunnerEntry};

#[derive(Clone, Serialize)]
pub struct StatusEvent {
    pub name: String,
    pub status: SessionStatus,
}

pub fn spawn_session_status_listener(app: AppHandle, name: String, mut rx: termhub_core::StatusRx) {
    tokio::spawn(async move {
        loop {
            let status = rx.borrow().clone();
            let _ = app.emit(
                "session:status",
                StatusEvent {
                    name: name.clone(),
                    status,
                },
            );
            if rx.changed().await.is_err() {
                break;
            }
        }
    });
}

pub async fn persist_now(state: &AppState) -> anyhow::Result<()> {
    let runners = state.runners.read().await;
    let listen = state.server_listen_addr.read().await.to_string();
    let cfg = termhub_core::Config {
        server: termhub_core::ServerConfig {
            listen,
            host_key_path: "host_key".into(),
            max_clients_per_session: state.max_clients_per_session,
        },
        ui: termhub_core::UiConfig::default(),
        sessions: runners.values().map(|r| r.config.clone()).collect(),
    };
    termhub_core::save(&state.config_dir, &cfg)
}

#[derive(Serialize, Clone)]
pub struct SessionView {
    pub name: String,
    pub status: SessionStatus,
    pub upstream_kind: String,
    pub upstream_summary: String,
    pub auto_reconnect: bool,
    pub client_count: usize,
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionView>, String> {
    let runners = state.runners.read().await;
    let mut out = Vec::new();
    for entry in state.mgr.list().await {
        let key = entry.name.to_ascii_lowercase();
        let status = entry.status.borrow().clone();
        let (kind, summary, auto_rc) = match runners.get(&key) {
            Some(r) => (
                upstream_kind_label(&r.config.upstream),
                upstream_summary(&r.config.upstream),
                r.config.auto_reconnect,
            ),
            None => ("unknown".to_string(), String::new(), true),
        };
        let client_count = entry.clients.list().await.len();
        out.push(SessionView {
            name: entry.name.clone(),
            status,
            upstream_kind: kind,
            upstream_summary: summary,
            auto_reconnect: auto_rc,
            client_count,
        });
    }
    Ok(out)
}

fn upstream_kind_label(s: &UpstreamSpec) -> String {
    match s {
        UpstreamSpec::Loopback => "loopback",
        UpstreamSpec::Serial { .. } => "serial",
        UpstreamSpec::Ssh { .. } => "ssh",
        UpstreamSpec::Telnet { .. } => "telnet",
        UpstreamSpec::RawTcp { .. } => "raw_tcp",
        UpstreamSpec::LocalShell { .. } => "local_shell",
    }
    .to_string()
}

fn upstream_summary(s: &UpstreamSpec) -> String {
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
    }
}

#[derive(Deserialize)]
pub struct CreateSessionArgs {
    pub config: SessionConfig,
}

#[tauri::command]
pub async fn create_session(
    args: CreateSessionArgs,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let cfg = args.config;
    let name = cfg.name.clone();
    let password = cfg.password.clone();
    let auto_rc = cfg.auto_reconnect;
    let spec_for_factory = cfg.upstream.clone();
    let factory: termhub_core::runner::DriverFactory =
        Box::new(move || create_driver(&spec_for_factory).expect("driver factory"));
    let started = start_session(
        state.mgr.clone(),
        name.clone(),
        password.clone(),
        factory,
        RunnerConfig {
            auto_reconnect: auto_rc,
            max_attempts: None,
        },
    )
    .await
    .map_err(|e| e.to_string())?;

    spawn_session_status_listener(app, name.clone(), started.status_rx.clone());

    let cfg_for_runner = SessionConfig {
        name: name.clone(),
        password,
        auto_reconnect: auto_rc,
        pty_override: cfg.pty_override,
        upstream: cfg.upstream,
    };
    state.runners.write().await.insert(
        name.to_ascii_lowercase(),
        RunnerEntry {
            started,
            config: cfg_for_runner,
        },
    );

    let _ = persist_now(&state).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn stop_session(name: String, state: State<'_, AppState>) -> Result<(), String> {
    let key = name.to_ascii_lowercase();
    if let Some(r) = state.runners.write().await.remove(&key) {
        r.started.cancel.cancel();
        let _ = r.started.handle.await;
    }
    state.mgr.unregister(&name).await;
    let _ = persist_now(&state).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_session(name: String, state: State<'_, AppState>) -> Result<(), String> {
    stop_session(name, state).await
}

#[derive(Serialize)]
pub struct ClientView {
    pub id: u64,
    pub remote: String,
    pub connected_at_secs: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[tauri::command]
pub async fn list_clients(name: String, state: State<'_, AppState>) -> Result<Vec<ClientView>, String> {
    let entry = state.mgr.get(&name).await.ok_or_else(|| "no such session".to_string())?;
    let v = entry.clients.list().await;
    Ok(v
        .into_iter()
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

#[tauri::command]
pub async fn kick_client(
    session: String,
    client_id: u64,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let entry = state
        .mgr
        .get(&session)
        .await
        .ok_or_else(|| "no such session".to_string())?;
    Ok(entry.clients.kick(client_id).await)
}

#[derive(Serialize)]
pub struct ServerInfo {
    pub listen: String,
    pub host_key_fpr: String,
}

#[tauri::command]
pub async fn get_server_info(state: State<'_, AppState>) -> Result<ServerInfo, String> {
    Ok(ServerInfo {
        listen: state.server_listen_addr.read().await.to_string(),
        host_key_fpr: state.host_key_fpr.read().await.clone(),
    })
}
