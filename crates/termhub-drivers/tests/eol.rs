use bytes::Bytes;
use termhub_drivers::eol::{transform, EolMode};

#[test]
fn as_is_passes_through() {
    let v = transform(EolMode::AsIs, Bytes::from_static(b"a\rb\nc\r\nd"));
    assert_eq!(&v[..], b"a\rb\nc\r\nd");
}

#[test]
fn lf_replaces_lone_cr_with_lf() {
    let v = transform(EolMode::Lf, Bytes::from_static(b"a\rb\r\nc"));
    assert_eq!(&v[..], b"a\nb\r\nc");
}

#[test]
fn crlf_promotes_lone_cr_and_lone_lf_to_crlf() {
    let v = transform(EolMode::Crlf, Bytes::from_static(b"a\rb\nc\r\nd"));
    assert_eq!(&v[..], b"a\r\nb\r\nc\r\nd");
}

#[test]
fn empty_input_yields_empty_output() {
    let v = transform(EolMode::Crlf, Bytes::new());
    assert!(v.is_empty());
}
