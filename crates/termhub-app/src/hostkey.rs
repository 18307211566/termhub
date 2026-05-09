use std::fs;
use std::path::Path;

use base64::Engine as _;
use russh_keys::key::KeyPair;
use sha2::{Digest, Sha256};

pub fn load_or_create(path: &Path) -> anyhow::Result<KeyPair> {
    if path.exists() {
        let pem = fs::read_to_string(path)?;
        let kp = russh_keys::decode_secret_key(&pem, None)?;
        return Ok(kp);
    }
    let kp = KeyPair::generate_ed25519().ok_or_else(|| anyhow::anyhow!("gen ed25519 failed"))?;
    let mut buf = Vec::new();
    russh_keys::encode_pkcs8_pem(&kp, &mut buf)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, buf)?;
    Ok(kp)
}

/// UI 展示用指纹（对 PKCS#8 PEM 做 SHA256，前缀 SHA256:）。
pub fn fingerprint(kp: &KeyPair) -> anyhow::Result<String> {
    let mut buf = Vec::new();
    russh_keys::encode_pkcs8_pem(kp, &mut buf)?;
    let mut h = Sha256::new();
    h.update(&buf);
    let d = h.finalize();
    let b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(d);
    Ok(format!("SHA256:{b64}"))
}
