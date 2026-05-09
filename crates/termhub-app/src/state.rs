use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use termhub_core::{SessionConfig, SessionMgr, StartedSession};
use tokio::sync::RwLock;

pub struct RunnerEntry {
    pub started: StartedSession,
    pub config: SessionConfig,
}

pub struct AppState {
    pub mgr: Arc<SessionMgr>,
    pub runners: Arc<RwLock<HashMap<String, RunnerEntry>>>,
    pub server_listen_addr: Arc<RwLock<SocketAddr>>,
    pub host_key_fpr: Arc<RwLock<String>>,
    pub config_dir: PathBuf,
    pub max_clients_per_session: usize,
}

impl AppState {
    pub fn new(
        mgr: Arc<SessionMgr>,
        listen: SocketAddr,
        fpr: String,
        config_dir: PathBuf,
        max_clients_per_session: usize,
    ) -> Self {
        Self {
            mgr,
            runners: Arc::new(RwLock::new(HashMap::new())),
            server_listen_addr: Arc::new(RwLock::new(listen)),
            host_key_fpr: Arc::new(RwLock::new(fpr)),
            config_dir,
            max_clients_per_session,
        }
    }
}
