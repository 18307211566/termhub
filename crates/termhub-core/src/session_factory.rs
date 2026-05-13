use std::sync::Arc;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::hub::Hub;
use crate::runner::{spawn_runner, DriverFactory, RunnerConfig};
use crate::session::{status_channel, StatusRx};
use crate::session_mgr::SessionMgr;

pub struct StartedSession {
    pub status_rx: StatusRx,
    pub cancel: CancellationToken,
    pub handle: JoinHandle<()>,
}

pub async fn start_session(
    mgr: Arc<SessionMgr>,
    _name: String,
    ssh_user: String,
    password: String,
    factory: DriverFactory,
    cfg: RunnerConfig,
) -> anyhow::Result<StartedSession> {
    let (hub, up_tx, up_rx) = Hub::new(1024, 256);
    let (status_tx, status_rx) = status_channel();
    let cancel = CancellationToken::new();

    mgr.register(
        ssh_user,
        password,
        hub.handle(),
        status_rx.clone(),
        cancel.clone(),
    )
    .await?;

    let handle = spawn_runner(factory, hub, up_tx, up_rx, status_tx, cancel.clone(), cfg);

    Ok(StartedSession {
        status_rx,
        cancel,
        handle,
    })
}
