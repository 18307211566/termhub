use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use russh_keys::key::KeyPair;
use termhub_core::{SessionConfig, SessionMgr, StartedSession};

use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct RunnerEntry {
    /// 常规会话使用 StartedSession；HTTP 代理会话为 None（不经过 Hub）
    pub started: Option<StartedSession>,
    pub config: SessionConfig,
    /// HTTP 代理取消令牌（仅 HttpProxy 类型使用）
    pub proxy_cancel: Option<CancellationToken>,
    /// 该会话的 SSH server 取消令牌
    pub sshd_cancel: Option<CancellationToken>,
    /// 该会话的 SSH server 任务句柄
    pub sshd_handle: Option<JoinHandle<()>>,
    /// 该会话的 SessionMgr（用于查询下联客户端）
    pub session_mgr: Option<Arc<SessionMgr>>,
}

#[derive(Clone)]
pub struct AppState {
    pub runners: Arc<RwLock<HashMap<String, RunnerEntry>>>,
    /// 全局 host keys，所有会话的 SSH server 共用
    pub host_keys: Vec<KeyPair>,
    pub host_key_fpr: String,
    pub config_dir: PathBuf,
    pub max_clients_per_session: usize,
    /// HTTP API listen address (for CLI).
    pub api_listen: Arc<RwLock<String>>,
}

impl AppState {
    pub fn new(
        host_keys: Vec<KeyPair>,
        host_key_fpr: String,
        config_dir: PathBuf,
        max_clients_per_session: usize,
        api_listen: String,
    ) -> Self {
        Self {
            runners: Arc::new(RwLock::new(HashMap::new())),
            host_keys,
            host_key_fpr,
            config_dir,
            max_clients_per_session,
            api_listen: Arc::new(RwLock::new(api_listen)),
        }
    }
}
