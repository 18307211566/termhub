use termhub_drivers::telnet::process_iac_block;

#[test]
fn iac_will_echo_responds_dont() {
    let (consumed, reply) = process_iac_block(&[0xFF, 0xFB, 0x01, b'a']);
    assert_eq!(consumed, 3);
    assert_eq!(reply, vec![0xFF, 0xFE, 0x01]);
}

#[test]
fn iac_do_naws_responds_wont() {
    let (consumed, reply) = process_iac_block(&[0xFF, 0xFD, 0x1F]);
    assert_eq!(consumed, 3);
    assert_eq!(reply, vec![0xFF, 0xFC, 0x1F]);
}

#[test]
fn standalone_iac_iac_is_data_byte_0xff() {
    let (consumed, reply) = process_iac_block(&[0xFF, 0xFF, b'x']);
    assert_eq!(consumed, 2);
    assert!(reply.is_empty());
}

#[test]
fn incomplete_iac_returns_zero_consumed() {
    let (consumed, reply) = process_iac_block(&[0xFF]);
    assert_eq!(consumed, 0);
    assert!(reply.is_empty());
}
