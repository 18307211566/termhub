use std::fs;
use std::path::Path;

use base64::Engine as _;
use russh_keys::key::KeyPair;
use sha2::{Digest, Sha256};

/// Load or generate Ed25519 + RSA host keys for maximum client compatibility.
pub fn load_or_create(path: &Path) -> anyhow::Result<Vec<KeyPair>> {
    let ed25519_path = path.with_extension("ed25519");
    let rsa_path = path.with_extension("rsa");

    let ed25519 = if ed25519_path.exists() {
        let pem = fs::read_to_string(&ed25519_path)?;
        russh_keys::decode_secret_key(&pem, None)?
    } else {
        let kp =
            KeyPair::generate_ed25519().ok_or_else(|| anyhow::anyhow!("gen ed25519 failed"))?;
        let mut buf = Vec::new();
        russh_keys::encode_pkcs8_pem(&kp, &mut buf)?;
        if let Some(parent) = ed25519_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&ed25519_path, buf)?;
        kp
    };

    let rsa = if rsa_path.exists() {
        let pem = fs::read_to_string(&rsa_path)?;
        russh_keys::decode_secret_key(&pem, None)?
    } else {
        let kp = KeyPair::generate_rsa(3072, russh_keys::key::SignatureHash::SHA2_256)
            .ok_or_else(|| anyhow::anyhow!("gen rsa failed"))?;
        let mut buf = Vec::new();
        russh_keys::encode_pkcs8_pem(&kp, &mut buf)?;
        if let Some(parent) = rsa_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&rsa_path, buf)?;
        kp
    };

    Ok(vec![ed25519, rsa])
}

/// UI 展示用指纹（对 Ed25519 PKCS#8 PEM 做 SHA256，前缀 SHA256:）。
pub fn fingerprint(kp: &KeyPair) -> anyhow::Result<String> {
    let mut buf = Vec::new();
    russh_keys::encode_pkcs8_pem(kp, &mut buf)?;
    let mut h = Sha256::new();
    h.update(&buf);
    let d = h.finalize();
    let b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(d);
    Ok(format!("SHA256:{b64}"))
}
