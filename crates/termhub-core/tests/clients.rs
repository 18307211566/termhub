use termhub_core::clients::ClientRegistry;

#[tokio::test]
async fn attach_assigns_unique_ids_and_lists() {
    let r = ClientRegistry::default();
    let a = r.attach("1.1.1.1:1".into()).await;
    let b = r.attach("1.1.1.1:2".into()).await;
    assert_ne!(a.id, b.id);
    let v = r.list().await;
    assert_eq!(v.len(), 2);
}

#[tokio::test]
async fn detach_removes_record() {
    let r = ClientRegistry::default();
    let a = r.attach("x".into()).await;
    r.detach(a.id).await;
    assert_eq!(r.list().await.len(), 0);
}

#[tokio::test]
async fn kick_triggers_token() {
    let r = ClientRegistry::default();
    let a = r.attach("x".into()).await;
    let tok = a.kick.clone();
    r.kick(a.id).await;
    assert!(tok.is_cancelled());
}
