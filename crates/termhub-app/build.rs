//! Windows：除 tauri-build 默认嵌入的图标外，再写入 ID 为 1 的 ICON。
//! Explorer 按数值最小的图标组选 exe 图标；仅 32512 时可能仍显示默认程序图标（见 tauri-winres 说明）。

fn escape_rc_string(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\"\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}

fn strip_verbatim_windows_prefix(path: String) -> String {
    const UNC_PREFIX: &str = r"\\?\UNC\";
    if let Some(rest) = path.strip_prefix(UNC_PREFIX) {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    path
}

fn main() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let icon_path = manifest_dir.join("icons").join("icon.ico");
    println!("cargo:rerun-if-changed={}", icon_path.display());

    let icon_abs = std::fs::canonicalize(&icon_path).unwrap_or(icon_path);
    // canonicalize() 在 Windows 上常得到 \\?\...，RC/链接阶段易导致图标无法正确嵌入；与 tauri 使用的普通绝对路径保持一致。
    let path_for_rc = strip_verbatim_windows_prefix(icon_abs.display().to_string());
    let escaped = escape_rc_string(&path_for_rc);

    let windows = tauri_build::WindowsAttributes::new()
        .append_rc_content(format!("1 ICON \"{escaped}\"\n"));

    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
