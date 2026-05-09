use termhub_core::secrets::{decrypt, encrypt};

#[test]
fn encrypt_then_decrypt_roundtrips_ascii() {
    let original = "secretPass!#42";
    let blob = encrypt(original).unwrap();
    let back = decrypt(&blob).unwrap();
    assert_eq!(back, original);
}

#[test]
fn empty_string_roundtrips() {
    let blob = encrypt("").unwrap();
    assert_eq!(decrypt(&blob).unwrap(), "");
}
