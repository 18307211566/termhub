use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub sessions: Vec<SessionConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 例 "0.0.0.0:2222" 或 "127.0.0.1:2222"
    pub listen: String,
    /// 相对 %APPDATA%\termhub 的 host key 路径
    #[serde(default = "default_hostkey")]
    pub host_key_path: String,
    #[serde(default = "default_max_clients")]
    pub max_clients_per_session: usize,
    /// HTTP API 监听地址（供 CLI 使用）
    #[serde(default = "default_api_listen")]
    pub api_listen: String,
}

fn default_hostkey() -> String {
    "host_key".into()
}
fn default_max_clients() -> usize {
    16
}
fn default_api_listen() -> String {
    "127.0.0.1:2223".into()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UiConfig {
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub auto_start: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConfig {
    pub name: String,
    /// 运行期为明文；持久化时由 config 层在序列化前加密为 base64 字符串
    pub password: String,
    #[serde(default = "yes")]
    pub auto_reconnect: bool,
    #[serde(default)]
    pub pty_override: Option<PtySize>,
    pub upstream: UpstreamSpec,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtySize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpstreamSpec {
    /// 内置环回（测试 / 默认 demo）
    Loopback,
    Serial {
        port: String,
        baud: u32,
        #[serde(default = "default_8")]
        data_bits: u8,
        #[serde(default = "default_parity")]
        parity: String,
        #[serde(default = "default_1")]
        stop_bits: u8,
        #[serde(default = "default_flow")]
        flow: String,
        #[serde(default = "default_eol")]
        input_eol: String,
        #[serde(default = "default_eol")]
        output_eol: String,
    },
    Ssh {
        host: String,
        port: u16,
        user: String,
        password: String,
    },
    Telnet {
        host: String,
        port: u16,
    },
    RawTcp {
        host: String,
        port: u16,
    },
    LocalShell {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// HTTP 反向代理：监听 listen，将 HTTP 请求转发到 target
    HttpProxy {
        listen: String,
        target: String,
    },
}

fn default_8() -> u8 {
    8
}
fn default_1() -> u8 {
    1
}
fn default_parity() -> String {
    "none".into()
}
fn default_flow() -> String {
    "none".into()
}
fn default_eol() -> String {
    "as_is".into()
}
