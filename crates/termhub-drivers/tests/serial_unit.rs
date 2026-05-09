use termhub_drivers::serial::parse_serial_params;

#[test]
fn parse_defaults_serial_params() {
    let p = parse_serial_params(8, "none", 1, "none").unwrap();
    assert_eq!(p.data_bits, tokio_serial::DataBits::Eight);
    assert!(matches!(p.parity, tokio_serial::Parity::None));
    assert!(matches!(p.stop_bits, tokio_serial::StopBits::One));
    assert!(matches!(p.flow, tokio_serial::FlowControl::None));
}

#[test]
fn parse_seven_even_two_hardware() {
    let p = parse_serial_params(7, "even", 2, "hardware").unwrap();
    assert!(matches!(p.data_bits, tokio_serial::DataBits::Seven));
    assert!(matches!(p.parity, tokio_serial::Parity::Even));
    assert!(matches!(p.stop_bits, tokio_serial::StopBits::Two));
    assert!(matches!(p.flow, tokio_serial::FlowControl::Hardware));
}

#[test]
fn parse_invalid_data_bits_errors() {
    assert!(parse_serial_params(9, "none", 1, "none").is_err());
}
