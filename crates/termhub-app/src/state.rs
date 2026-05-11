use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use russh_keys::key::KeyPair;
use termhub_core::{SessionConfig, SessionMgr, StartedSession};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct RunnerEntry {
    pub started: StartedSession,
    pub config: SessionConfig,
}

/// Tracks the running sshd accept loop so it can be cancelled and rebound.
pub struct SshdState {
    pub cancel: CancellationToken,
    pub handle: JoinHandle<()>,
    pub host_keys: Vec<KeyPair>,
}

#[derive(Clone)]
pub struct AppState {
    pub mgr: Arc<SessionMgr>,
    pub runners: Arc<RwLock<HashMap<String, RunnerEntry>>>,
    pub server_listen_addr: Arc<RwLock<SocketAddr>>,
    pub host_key_fpr: Arc<RwLock<String>>,
    pub config_dir: PathBuf,
    pub max_clients_per_session: usize,
    /// Running sshd handle — guarded by an async lock for hot-rebind.
    pub sshd: Arc<RwLock<SshdState>>,
    /// HTTP API listen address (for CLI).
    pub api_listen: Arc<RwLock<String>>,
}

impl AppState {
    pub fn new(
        mgr: Arc<SessionMgr>,
        listen: SocketAddr,
        fpr: String,
        config_dir: PathBuf,
        max_clients_per_session: usize,
        sshd_cancel: CancellationToken,
        sshd_handle: JoinHandle<()>,
        host_keys: Vec<KeyPair>,
        api_listen: String,
    ) -> Self {
        Self {
            mgr,
            runners: Arc::new(RwLock::new(HashMap::new())),
            server_listen_addr: Arc::new(RwLock::new(listen)),
            host_key_fpr: Arc::new(RwLock::new(fpr)),
            config_dir,
            max_clients_per_session,
            sshd: Arc::new(RwLock::new(SshdState {
                cancel: sshd_cancel,
                handle: sshd_handle,
                host_keys,
            })),
            api_listen: Arc::new(RwLock::new(api_listen)),
        }
    }
}
