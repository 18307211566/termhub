use termhub_core::{Config, UpstreamSpec};

#[test]
fn parse_full_example_config() {
    let toml = r#"
[server]
listen = "0.0.0.0:2222"
host_key_path = "host_key"
max_clients_per_session = 16

[ui]
start_minimized = false
auto_start = false

[[sessions]]
name = "com3"
password = "secret"
auto_reconnect = true
[sessions.pty_override]
cols = 80
rows = 24
[sessions.upstream]
type = "serial"
port = "COM3"
baud = 115200
data_bits = 8
parity = "none"
stop_bits = 1
flow = "none"
input_eol = "as_is"
output_eol = "as_is"

[[sessions]]
name = "router"
password = "p"
auto_reconnect = true
[sessions.upstream]
type = "ssh"
host = "192.168.1.1"
port = 22
user = "admin"
password = "p2"

[[sessions]]
name = "shell"
password = "p"
auto_reconnect = false
[sessions.upstream]
type = "local_shell"
command = "powershell.exe"

[[sessions]]
name = "telnet1"
password = "p"
auto_reconnect = true
[sessions.upstream]
type = "telnet"
host = "1.2.3.4"
port = 23

[[sessions]]
name = "tcp1"
password = "p"
auto_reconnect = true
[sessions.upstream]
type = "raw_tcp"
host = "1.2.3.4"
port = 9000
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    assert_eq!(cfg.server.listen, "0.0.0.0:2222");
    assert_eq!(cfg.sessions.len(), 5);
    assert!(matches!(cfg.sessions[0].upstream, UpstreamSpec::Serial { .. }));
    assert!(matches!(cfg.sessions[1].upstream, UpstreamSpec::Ssh { .. }));
    assert!(matches!(
        cfg.sessions[2].upstream,
        UpstreamSpec::LocalShell { .. }
    ));
    assert!(matches!(cfg.sessions[3].upstream, UpstreamSpec::Telnet { .. }));
    assert!(matches!(cfg.sessions[4].upstream, UpstreamSpec::RawTcp { .. }));
}

#[test]
fn serialize_roundtrip() {
    let toml = r#"
[server]
listen = "127.0.0.1:2222"
host_key_path = "host_key"
max_clients_per_session = 4

[ui]
start_minimized = true
auto_start = false

[[sessions]]
name = "a"
password = "p"
auto_reconnect = true
[sessions.upstream]
type = "raw_tcp"
host = "h"
port = 1
"#;
    let cfg: Config = toml::from_str(toml).unwrap();
    let s = toml::to_string(&cfg).unwrap();
    let cfg2: Config = toml::from_str(&s).unwrap();
    assert_eq!(cfg, cfg2);
}
