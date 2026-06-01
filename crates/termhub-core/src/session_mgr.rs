use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::hub::HubHandle;
use crate::session::StatusRx;

/// 会话名规则：[a-zA-Z0-9._-]+，匹配后**统一转小写**作为 map key
fn normalize_name(s: &str) -> Option<String> {
    if s.is_empty() {
        return None;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return None;
    }
    Some(s.to_ascii_lowercase())
}

#[derive(Clone)]
pub struct SessionEntry {
    pub name: String,
    pub password: String,
    pub hub: HubHandle,
    pub status: StatusRx,
    pub cancel: CancellationToken,
    pub clients: crate::clients::ClientRegistry,
}

#[derive(Default)]
pub struct SessionMgr {
    inner: Arc<RwLock<HashMap<String, SessionEntry>>>,
}

impl SessionMgr {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(
        &self,
        name: String,
        password: String,
        hub: HubHandle,
        status: StatusRx,
        cancel: CancellationToken,
    ) -> anyhow::Result<()> {
        let key = normalize_name(&name).ok_or_else(|| anyhow::anyhow!("invalid session name"))?;
        let mut g = self.inner.write().await;
        if g.contains_key(&key) {
            anyhow::bail!("session already exists");
        }
        g.insert(
            key,
            SessionEntry {
                name,
                password,
                hub,
                status,
                cancel,
                clients: crate::clients::ClientRegistry::default(),
            },
        );
        Ok(())
    }

    pub async fn unregister(&self, name: &str) -> Option<SessionEntry> {
        let key = normalize_name(name)?;
        self.inner.write().await.remove(&key)
    }

    pub async fn get(&self, name: &str) -> Option<SessionEntry> {
        let key = normalize_name(name)?;
        self.inner.read().await.get(&key).cloned()
    }

    /// 用户名 + 密码鉴权，命中返回 SessionEntry（含 HubHandle）
    pub async fn authenticate(&self, name: &str, password: &str) -> Option<SessionEntry> {
        let entry = self.get(name).await?;
        if constant_time_eq(entry.password.as_bytes(), password.as_bytes()) {
            Some(entry)
        } else {
            None
        }
    }

    pub async fn list(&self) -> Vec<SessionEntry> {
        self.inner.read().await.values().cloned().collect()
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
