use termhub_drivers::ssh_client::SshClientDriver;

#[test]
fn ssh_client_driver_can_be_constructed() {
    let _ = SshClientDriver {
        host: "h".into(),
        port: 22,
        user: "u".into(),
        password: "p".into(),
    };
}
