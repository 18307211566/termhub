//! Placeholder for throughput smoke (multi-client heavy traffic). Run manually with `--ignored`
//! when russh server harness is wired.

#[tokio::test]
#[ignore]
async fn five_clients_one_gb_placeholder() {
    // Future: spawn russh server, 5 clients each push ~200 MiB, verify reconnect/backpressure.
}
