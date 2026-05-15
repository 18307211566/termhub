use std::fs;

use termhub_core::persist::{config_dir_for_test, load_or_default, save};
use termhub_core::{Config, ServerConfig, SessionConfig, UiConfig, UpstreamSpec};

#[test]
fn save_then_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Config {
        server: ServerConfig {
            listen: "127.0.0.1:2222".into(),
            host_key_path: "host_key".into(),
            max_clients_per_session: 16,
            api_listen: "127.0.0.1:2223".into(),
        },
        ui: UiConfig::default(),
        sessions: vec![SessionConfig {
            name: "x".into(),
            password: "p".into(),
            ssh_user: "u".into(),
            listen: "0.0.0.0:2222".into(),
            auto_reconnect: true,
            launch_on_startup: true,
            pty_override: None,
            upstream: UpstreamSpec::RawTcp {
                host: "h".into(),
                port: 1,
            },
        }],
    };
    save(dir.path(), &cfg).unwrap();
    let loaded = load_or_default(dir.path()).unwrap();
    assert_eq!(cfg, loaded);
    let _ = config_dir_for_test;
}

#[test]
fn load_missing_returns_default() {
    let dir = tempfile::tempdir().unwrap();
    fs::remove_dir_all(dir.path()).ok();
    let loaded = load_or_default(dir.path()).unwrap();
    assert_eq!(loaded.sessions.len(), 0);
}
