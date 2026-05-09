use termhub_core::UpstreamSpec;
use termhub_drivers::factory::create_driver;

#[test]
fn factory_accepts_loopback() {
    assert!(create_driver(&UpstreamSpec::Loopback).is_ok());
}

#[test]
fn factory_creates_serial_driver() {
    let spec = UpstreamSpec::Serial {
        port: "COM1".into(),
        baud: 115200,
        data_bits: 8,
        parity: "none".into(),
        stop_bits: 1,
        flow: "none".into(),
        input_eol: "as_is".into(),
        output_eol: "as_is".into(),
    };
    assert!(create_driver(&spec).is_ok());
}

#[test]
fn factory_creates_all_other_kinds() {
    let kinds = vec![
        UpstreamSpec::Ssh {
            host: "h".into(),
            port: 22,
            user: "u".into(),
            password: "p".into(),
        },
        UpstreamSpec::Telnet {
            host: "h".into(),
            port: 23,
        },
        UpstreamSpec::RawTcp {
            host: "h".into(),
            port: 1,
        },
        UpstreamSpec::LocalShell {
            command: "cmd.exe".into(),
            args: vec![],
        },
    ];
    for s in kinds {
        assert!(create_driver(&s).is_ok());
    }
}

#[test]
fn factory_rejects_invalid_serial_params() {
    let spec = UpstreamSpec::Serial {
        port: "COM1".into(),
        baud: 115200,
        data_bits: 9,
        parity: "none".into(),
        stop_bits: 1,
        flow: "none".into(),
        input_eol: "as_is".into(),
        output_eol: "as_is".into(),
    };
    assert!(create_driver(&spec).is_err());
}
