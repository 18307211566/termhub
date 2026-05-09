use termhub_core::{SessionMgr, SessionStatus};

#[tokio::test]
async fn register_and_lookup() {
    let mgr = SessionMgr::new();
    let (status_tx, status_rx) = termhub_core::session::status_channel();
    let cancel = tokio_util::sync::CancellationToken::new();
    let (hub, _up_tx, _up_rx) = termhub_core::Hub::new(1024, 256);
    mgr.register(
        "echo".to_string(),
        "pass".to_string(),
        hub.handle(),
        status_rx,
        cancel.clone(),
    )
    .await
    .unwrap();
    assert!(mgr.get("echo").await.is_some());
    assert!(mgr.get("Echo").await.is_some(), "case-insensitive lookup");
    let _ = status_tx;
    let _ = cancel;
}

#[tokio::test]
async fn duplicate_name_rejected() {
    let mgr = SessionMgr::new();
    let (_, status_rx) = termhub_core::session::status_channel();
    let cancel = tokio_util::sync::CancellationToken::new();
    let (hub, _u, _v) = termhub_core::Hub::new(1024, 256);
    mgr.register(
        "x".to_string(),
        "p".to_string(),
        hub.handle(),
        status_rx.clone(),
        cancel.clone(),
    )
    .await
    .unwrap();
    let (hub2, _u2, _v2) = termhub_core::Hub::new(1024, 256);
    let r = mgr
        .register(
            "X".to_string(),
            "p".to_string(),
            hub2.handle(),
            status_rx,
            cancel,
        )
        .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn auth_correct_password_returns_handle() {
    let mgr = SessionMgr::new();
    let (_, status_rx) = termhub_core::session::status_channel();
    let cancel = tokio_util::sync::CancellationToken::new();
    let (hub, _u, _v) = termhub_core::Hub::new(1024, 256);
    mgr.register(
        "echo".to_string(),
        "secret".to_string(),
        hub.handle(),
        status_rx,
        cancel,
    )
    .await
    .unwrap();
    let h = mgr.authenticate("echo", "secret").await;
    assert!(h.is_some());
    assert!(mgr.authenticate("echo", "wrong").await.is_none());
    assert!(mgr.authenticate("nope", "x").await.is_none());
    let _ = SessionStatus::Idle; // ensure import path
}
