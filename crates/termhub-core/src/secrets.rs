use base64::{engine::general_purpose::STANDARD, Engine as _};

#[cfg(target_os = "windows")]
pub fn encrypt(plain: &str) -> anyhow::Result<String> {
    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};

    let mut input = plain.as_bytes().to_vec();
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    unsafe {
        let ok = CryptProtectData(
            &in_blob,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            &mut out_blob,
        );
        if ok == 0 {
            anyhow::bail!("CryptProtectData failed");
        }
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let s = STANDARD.encode(slice);
        LocalFree(out_blob.pbData as HLOCAL);
        Ok(s)
    }
}

#[cfg(target_os = "windows")]
pub fn decrypt(blob_b64: &str) -> anyhow::Result<String> {
    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    use windows_sys::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    let mut bytes = STANDARD.decode(blob_b64)?;
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    unsafe {
        let ok = CryptUnprotectData(
            &in_blob,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            &mut out_blob,
        );
        if ok == 0 {
            anyhow::bail!("CryptUnprotectData failed");
        }
        let slice = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize);
        let s = String::from_utf8(slice.to_vec())?;
        LocalFree(out_blob.pbData as HLOCAL);
        Ok(s)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn encrypt(plain: &str) -> anyhow::Result<String> {
    Ok(format!("plain:{}", STANDARD.encode(plain.as_bytes())))
}

#[cfg(not(target_os = "windows"))]
pub fn decrypt(blob_b64: &str) -> anyhow::Result<String> {
    let raw = blob_b64.strip_prefix("plain:").unwrap_or(blob_b64);
    let v = STANDARD.decode(raw)?;
    Ok(String::from_utf8(v)?)
}
