use bytes::{Bytes, BytesMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EolMode {
    AsIs,
    /// 单 `\r` → `\n`，`\r\n` 保持
    Lf,
    /// 单 `\r` → `\r\n`，单 `\n` → `\r\n`，`\r\n` 保持
    Crlf,
    /// 单 `\n` → `\r`，`\r\n` 保持
    Cr,
}

impl EolMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "lf" | "\n" => Self::Lf,
            "crlf" | "\r\n" => Self::Crlf,
            "cr" | "\r" => Self::Cr,
            _ => Self::AsIs,
        }
    }
}

pub fn transform(mode: EolMode, input: Bytes) -> Bytes {
    if matches!(mode, EolMode::AsIs) || input.is_empty() {
        return input;
    }
    let mut out = BytesMut::with_capacity(input.len() + 8);
    let bytes = &input[..];
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match (mode, b) {
            (EolMode::Lf, b'\r') => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    out.extend_from_slice(b"\r\n");
                    i += 2;
                    continue;
                }
                out.extend_from_slice(b"\n");
            }
            (EolMode::Crlf, b'\r') => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    out.extend_from_slice(b"\r\n");
                    i += 2;
                    continue;
                }
                out.extend_from_slice(b"\r\n");
            }
            (EolMode::Crlf, b'\n') => {
                out.extend_from_slice(b"\r\n");
            }
            (EolMode::Cr, b'\n') => {
                out.extend_from_slice(b"\r");
            }
            _ => out.extend_from_slice(&[b]),
        }
        i += 1;
    }
    out.freeze()
}
