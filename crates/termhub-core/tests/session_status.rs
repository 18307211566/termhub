use termhub_core::SessionStatus;

#[test]
fn status_is_terminal_classifies_correctly() {
    assert!(!SessionStatus::Running { uptime_secs: 0 }.is_terminal());
    assert!(!SessionStatus::Reconnecting { attempt: 1 }.is_terminal());
    assert!(SessionStatus::Stopped.is_terminal());
    assert!(SessionStatus::Failed { reason: "x".into() }.is_terminal());
}

#[tokio::test]
async fn status_watch_propagates_changes() {
    let (tx, mut rx) = termhub_core::session::status_channel();
    tx.send(SessionStatus::Starting).unwrap();
    rx.changed().await.unwrap();
    assert!(matches!(*rx.borrow(), SessionStatus::Starting));
    tx.send(SessionStatus::Running { uptime_secs: 0 }).unwrap();
    rx.changed().await.unwrap();
    assert!(matches!(*rx.borrow(), SessionStatus::Running { .. }));
}
