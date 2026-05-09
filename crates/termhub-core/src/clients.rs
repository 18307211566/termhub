use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub struct ClientRecord {
    pub id: u64,
    pub remote: String,
    pub connected_at: SystemTime,
    pub bytes_in: AtomicU64,
    pub bytes_out: AtomicU64,
    pub kick: CancellationToken,
}

#[derive(Default, Clone)]
pub struct ClientRegistry {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    map: HashMap<u64, Arc<ClientRecord>>,
}

impl ClientRegistry {
    pub async fn attach(&self, remote: String) -> Arc<ClientRecord> {
        let mut g = self.inner.write().await;
        g.next_id += 1;
        let id = g.next_id;
        let rec = Arc::new(ClientRecord {
            id,
            remote,
            connected_at: SystemTime::now(),
            bytes_in: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            kick: CancellationToken::new(),
        });
        g.map.insert(id, rec.clone());
        rec
    }

    pub async fn detach(&self, id: u64) {
        self.inner.write().await.map.remove(&id);
    }

    pub async fn kick(&self, id: u64) -> bool {
        if let Some(rec) = self.inner.read().await.map.get(&id).cloned() {
            rec.kick.cancel();
            true
        } else {
            false
        }
    }

    pub async fn list(&self) -> Vec<ClientRecordSnapshot> {
        self.inner
            .read()
            .await
            .map
            .values()
            .map(|r| ClientRecordSnapshot {
                id: r.id,
                remote: r.remote.clone(),
                connected_at: r.connected_at,
                bytes_in: r.bytes_in.load(Ordering::Relaxed),
                bytes_out: r.bytes_out.load(Ordering::Relaxed),
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ClientRecordSnapshot {
    pub id: u64,
    pub remote: String,
    pub connected_at: SystemTime,
    pub bytes_in: u64,
    pub bytes_out: u64,
}
