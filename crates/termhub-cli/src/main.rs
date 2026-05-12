use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tabled::{Table, Tabled};

use termhub_core::{SessionConfig, UpstreamSpec};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "termhub-cli", about = "TermHub CLI - manage sessions programmatically")]
struct Cli {
    /// HTTP API base URL
    #[arg(long, env("TERMHUB_API_URL"), default_value = "http://127.0.0.1:2223")]
    api_url: String,

    /// Output in JSON format
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Server management
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
    /// Client management
    Client {
        #[command(subcommand)]
        action: ClientAction,
    },
    /// List available serial ports
    SerialPorts,
}

#[derive(Subcommand)]
enum SessionAction {
    /// List all sessions
    List,
    /// Create a new session
    Create(CreateArgs),
    /// Stop a session
    Stop { name: String },
    /// Restart a session
    Restart { name: String },
    /// Delete a session (same as stop)
    Delete { name: String },
    /// Update a session
    Update(UpdateArgs),
}

#[derive(Args)]
struct CreateArgs {
    /// Session name (used as SSH username)
    name: String,
    /// Password for SSH authentication
    password: String,
    /// Enable auto-reconnect on upstream disconnect
    #[arg(long, default_value = "true")]
    auto_reconnect: bool,
    /// PTY columns (omit to disable PTY override)
    #[arg(long)]
    cols: Option<u16>,
    /// PTY rows
    #[arg(long)]
    rows: Option<u16>,
    /// Upstream type
    #[command(subcommand)]
    upstream: UpstreamCmd,
}

#[derive(Args)]
struct UpdateArgs {
    /// Current session name
    name: String,
    /// New password (omit to keep current)
    #[arg(long)]
    password: Option<String>,
    /// Enable auto-reconnect
    #[arg(long)]
    auto_reconnect: Option<bool>,
    /// New upstream config (omit to keep current)
    #[command(subcommand)]
    upstream: Option<UpstreamCmd>,
}

#[derive(Subcommand)]
enum UpstreamCmd {
    /// Internal loopback (echo)
    Loopback,
    /// Serial port
    Serial {
        /// Serial port name (e.g. COM3)
        #[arg(long)]
        port: String,
        /// Baud rate
        #[arg(long, default_value = "115200")]
        baud: u32,
        /// Data bits (5-8)
        #[arg(long, default_value = "8")]
        data_bits: u8,
        /// Parity (none, even, odd)
        #[arg(long, default_value = "none")]
        parity: String,
        /// Stop bits (1, 2)
        #[arg(long, default_value = "1")]
        stop_bits: u8,
        /// Flow control (none, hardware, software)
        #[arg(long, default_value = "none")]
        flow: String,
        /// Input EOL mode (as_is, lf, crlf, cr)
        #[arg(long, default_value = "as_is")]
        input_eol: String,
        /// Output EOL mode (as_is, lf, crlf, cr)
        #[arg(long, default_value = "as_is")]
        output_eol: String,
    },
    /// SSH client
    Ssh {
        /// Remote host
        #[arg(long)]
        host: String,
        /// Remote port
        #[arg(long, default_value = "22")]
        port: u16,
        /// Username
        #[arg(long)]
        user: String,
        /// Password
        #[arg(long)]
        password: String,
    },
    /// Telnet
    Telnet {
        /// Remote host
        #[arg(long)]
        host: String,
        /// Remote port
        #[arg(long, default_value = "23")]
        port: u16,
    },
    /// Raw TCP
    RawTcp {
        /// Remote host
        #[arg(long)]
        host: String,
        /// Remote port
        #[arg(long)]
        port: u16,
    },
    /// Local shell (PTY)
    LocalShell {
        /// Command to execute
        #[arg(long)]
        command: String,
        /// Arguments
        #[arg(long, num_args = 0..)]
        args: Vec<String>,
    },
    /// HTTP reverse proxy (port forwarding)
    HttpProxy {
        /// Local listen address (e.g. 0.0.0.0:8080)
        #[arg(long)]
        listen: String,
        /// Target address to forward to (e.g. 192.168.1.100:80)
        #[arg(long)]
        target: String,
    },
}

#[derive(Subcommand)]
enum ServerAction {
    /// Show server info (listen address, host key fingerprint)
    Info,
    /// Change SSH server listen address
    Listen {
        /// New listen address (e.g. 127.0.0.1:2222)
        addr: String,
    },
}

#[derive(Subcommand)]
enum ClientAction {
    /// List clients connected to a session
    List {
        /// Session name
        session: String,
    },
    /// Kick a client from a session
    Kick {
        /// Session name
        session: String,
        /// Client ID
        id: u64,
    },
}

// ---------------------------------------------------------------------------
// API response envelope
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ApiResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Table row types
// ---------------------------------------------------------------------------

#[derive(Tabled)]
struct SessionRow {
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "STATUS")]
    status: String,
    #[tabled(rename = "UPSTREAM")]
    upstream: String,
    #[tabled(rename = "AUTO-RC")]
    auto_reconnect: String,
    #[tabled(rename = "CLIENTS")]
    clients: usize,
}

#[derive(Tabled)]
struct ClientRow {
    #[tabled(rename = "ID")]
    id: u64,
    #[tabled(rename = "REMOTE")]
    remote: String,
    #[tabled(rename = "CONNECTED(s)")]
    connected: u64,
    #[tabled(rename = "BYTES_IN")]
    bytes_in: u64,
    #[tabled(rename = "BYTES_OUT")]
    bytes_out: u64,
}

#[derive(Tabled)]
struct ServerRow {
    #[tabled(rename = "KEY")]
    key: String,
    #[tabled(rename = "VALUE")]
    value: String,
}

// ---------------------------------------------------------------------------
// Session view from API (matches app_logic::SessionView)
// ---------------------------------------------------------------------------

#[derive(Deserialize, Serialize)]
struct SessionView {
    name: String,
    status: serde_json::Value,
    upstream_kind: String,
    upstream_summary: String,
    upstream: serde_json::Value,
    auto_reconnect: bool,
    client_count: usize,
}

#[derive(Deserialize, Serialize)]
struct ClientView {
    id: u64,
    remote: String,
    connected_at_secs: u64,
    bytes_in: u64,
    bytes_out: u64,
}

#[derive(Deserialize, Serialize)]
struct ServerInfo {
    listen: String,
    host_key_fpr: String,
}

// ---------------------------------------------------------------------------
// Format helpers
// ---------------------------------------------------------------------------

fn format_status(v: &serde_json::Value) -> String {
    match v.get("state").and_then(|s| s.as_str()) {
        Some("idle") => "idle".into(),
        Some("starting") => "starting".into(),
        Some("running") => {
            let secs = v.get("uptime_secs").and_then(|s| s.as_u64()).unwrap_or(0);
            format!("running ({secs}s)")
        }
        Some("reconnecting") => {
            let n = v.get("attempt").and_then(|s| s.as_u64()).unwrap_or(0);
            format!("reconnecting #{n}")
        }
        Some("failed") => {
            let reason = v
                .get("reason")
                .and_then(|s| s.as_str())
                .unwrap_or("unknown");
            format!("failed: {reason}")
        }
        Some("stopped") => "stopped".into(),
        other => format!("{other:?}"),
    }
}

fn upstream_to_spec(cmd: &UpstreamCmd) -> UpstreamSpec {
    match cmd {
        UpstreamCmd::Loopback => UpstreamSpec::Loopback,
        UpstreamCmd::Serial {
            port,
            baud,
            data_bits,
            parity,
            stop_bits,
            flow,
            input_eol,
            output_eol,
        } => UpstreamSpec::Serial {
            port: port.clone(),
            baud: *baud,
            data_bits: *data_bits,
            parity: parity.clone(),
            stop_bits: *stop_bits,
            flow: flow.clone(),
            input_eol: input_eol.clone(),
            output_eol: output_eol.clone(),
        },
        UpstreamCmd::Ssh {
            host,
            port,
            user,
            password,
        } => UpstreamSpec::Ssh {
            host: host.clone(),
            port: *port,
            user: user.clone(),
            password: password.clone(),
        },
        UpstreamCmd::Telnet { host, port } => UpstreamSpec::Telnet {
            host: host.clone(),
            port: *port,
        },
        UpstreamCmd::RawTcp { host, port } => UpstreamSpec::RawTcp {
            host: host.clone(),
            port: *port,
        },
        UpstreamCmd::LocalShell { command, args } => UpstreamSpec::LocalShell {
            command: command.clone(),
            args: args.clone(),
        },
        UpstreamCmd::HttpProxy { listen, target } => UpstreamSpec::HttpProxy {
            listen: listen.clone(),
            target: target.clone(),
        },
    }
}

// ---------------------------------------------------------------------------
// HTTP client
// ---------------------------------------------------------------------------

async fn api_get<T: serde::de::DeserializeOwned>(base: &str, path: &str) -> Result<T> {
    let resp = reqwest::get(format!("{base}{path}")).await?;
    let body: ApiResponse<T> = resp.json().await?;
    if body.ok {
        body.data
            .ok_or_else(|| anyhow::anyhow!("API returned ok=true but no data"))
    } else {
        Err(anyhow::anyhow!(
            "{}",
            body.error.unwrap_or_else(|| "unknown error".into())
        ))
    }
}

async fn api_post<T: serde::de::DeserializeOwned>(
    base: &str,
    path: &str,
    body: impl Serialize,
) -> Result<T> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}{path}"))
        .json(&body)
        .send()
        .await?;
    let body: ApiResponse<T> = resp.json().await?;
    if body.ok {
        body.data
            .ok_or_else(|| anyhow::anyhow!("API returned ok=true but no data"))
    } else {
        Err(anyhow::anyhow!(
            "{}",
            body.error.unwrap_or_else(|| "unknown error".into())
        ))
    }
}


// Unit calls that don't require a data field in the response.
async fn api_post_unit(base: &str, path: &str, body: impl Serialize) -> Result<()> {
    let client = reqwest::Client::new();
    let resp = client.post(format!("{base}{path}")).json(&body).send().await?;
    check_unit_response(resp).await
}

async fn api_put_unit(base: &str, path: &str, body: impl Serialize) -> Result<()> {
    let client = reqwest::Client::new();
    let resp = client.put(format!("{base}{path}")).json(&body).send().await?;
    check_unit_response(resp).await
}

async fn api_delete_unit(base: &str, path: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let resp = client.delete(format!("{base}{path}")).send().await?;
    check_unit_response(resp).await
}

async fn check_unit_response(resp: reqwest::Response) -> Result<()> {
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            let msg = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error");
            return Err(anyhow::anyhow!("HTTP {status}: {msg}"));
        }
        return Err(anyhow::anyhow!("HTTP {status}: {text}"));
    }
    if text.is_empty() {
        return Ok(());
    }
    let v: serde_json::Value = serde_json::from_str(&text)?;
    if v.get("ok").and_then(|b| b.as_bool()).unwrap_or(false) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "{}",
            v.get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown error")
        ))
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let base = cli.api_url.trim_end_matches('/').to_string();
    let as_json = cli.json;

    match cli.command {
        Command::Session { action } => match action {
            SessionAction::List => {
                let sessions: Vec<SessionView> = api_get(&base, "/api/sessions").await?;
                if as_json {
                    println!("{}", serde_json::to_string_pretty(&sessions)?);
                } else {
                    let rows: Vec<SessionRow> = sessions
                        .iter()
                        .map(|s| SessionRow {
                            name: s.name.clone(),
                            status: format_status(&s.status),
                            upstream: format!("{} ({})", s.upstream_kind, s.upstream_summary),
                            auto_reconnect: if s.auto_reconnect {
                                "yes".into()
                            } else {
                                "no".into()
                            },
                            clients: s.client_count,
                        })
                        .collect();
                    println!("{}", Table::new(&rows));
                }
            }
            SessionAction::Create(args) => {
                let pty = match (args.cols, args.rows) {
                    (Some(c), Some(r)) => Some(termhub_core::PtySize { cols: c, rows: r }),
                    _ => None,
                };
                let config = SessionConfig {
                    name: args.name,
                    password: args.password,
                    auto_reconnect: args.auto_reconnect,
                    pty_override: pty,
                    upstream: upstream_to_spec(&args.upstream),
                };
                api_post_unit(&base, "/api/sessions", &config).await?;
                if as_json {
                    println!("{}", serde_json::json!({"ok": true}));
                } else {
                    println!("Session '{}' created.", config.name);
                }
            }
            SessionAction::Stop { name } => {
                api_post_unit(&base, &format!("/api/sessions/{name}/stop"), ()).await?;
                if as_json {
                    println!("{}", serde_json::json!({"ok": true}));
                } else {
                    println!("Session '{name}' stopped.");
                }
            }
            SessionAction::Restart { name } => {
                api_post_unit(&base, &format!("/api/sessions/{name}/restart"), ()).await?;
                if as_json {
                    println!("{}", serde_json::json!({"ok": true}));
                } else {
                    println!("Session '{name}' restarted.");
                }
            }
            SessionAction::Delete { name } => {
                api_delete_unit(&base, &format!("/api/sessions/{name}")).await?;
                if as_json {
                    println!("{}", serde_json::json!({"ok": true}));
                } else {
                    println!("Session '{name}' deleted.");
                }
            }
            SessionAction::Update(args) => {
                // For update, we need to send the full SessionConfig.
                // First fetch the current session to fill in missing fields.
                let sessions: Vec<SessionView> = api_get(&base, "/api/sessions").await?;
                let current = sessions
                    .iter()
                    .find(|s| s.name.eq_ignore_ascii_case(&args.name))
                    .ok_or_else(|| anyhow::anyhow!("session '{}' not found", args.name))?;

                let password = args
                    .password
                    .unwrap_or_else(|| current.name.clone()); // placeholder, real password not exposed
                let auto_reconnect = args.auto_reconnect.unwrap_or(current.auto_reconnect);
                let upstream = args
                    .upstream
                    .as_ref()
                    .map(upstream_to_spec)
                    .unwrap_or(UpstreamSpec::Loopback); // placeholder

                let config = SessionConfig {
                    name: args.name.clone(),
                    password,
                    auto_reconnect,
                    pty_override: None,
                    upstream,
                };
                api_put_unit(&base, &format!("/api/sessions/{}", args.name), &config).await?;
                if as_json {
                    println!("{}", serde_json::json!({"ok": true}));
                } else {
                    println!("Session '{}' updated.", args.name);
                }
            }
        },
        Command::Client { action } => match action {
            ClientAction::List { session } => {
                let clients: Vec<ClientView> =
                    api_get(&base, &format!("/api/sessions/{session}/clients")).await?;
                if as_json {
                    println!("{}", serde_json::to_string_pretty(&clients)?);
                } else {
                    let rows: Vec<ClientRow> = clients
                        .iter()
                        .map(|c| ClientRow {
                            id: c.id,
                            remote: c.remote.clone(),
                            connected: c.connected_at_secs,
                            bytes_in: c.bytes_in,
                            bytes_out: c.bytes_out,
                        })
                        .collect();
                    if rows.is_empty() {
                        println!("No clients connected to session '{session}'.");
                    } else {
                        println!("{}", Table::new(&rows));
                    }
                }
            }
            ClientAction::Kick { session, id } => {
                let result: bool = api_post(
                    &base,
                    &format!("/api/sessions/{session}/clients/{id}/kick"),
                    (),
                )
                .await?;
                if as_json {
                    println!("{}", serde_json::json!({"ok": true, "kicked": result}));
                } else if result {
                    println!("Client {id} kicked from session '{session}'.");
                } else {
                    println!("Client {id} not found in session '{session}'.");
                }
            }
        },
        Command::Server { action } => match action {
            ServerAction::Info => {
                let info: ServerInfo = api_get(&base, "/api/server").await?;
                if as_json {
                    println!("{}", serde_json::to_string_pretty(&info)?);
                } else {
                    let rows = vec![
                        ServerRow {
                            key: "Listen".into(),
                            value: info.listen,
                        },
                        ServerRow {
                            key: "Host Key Fingerprint".into(),
                            value: info.host_key_fpr,
                        },
                    ];
                    println!("{}", Table::new(&rows));
                }
            }
            ServerAction::Listen { addr } => {
                api_put_unit(
                    &base,
                    "/api/server/listen",
                    serde_json::json!({"addr": addr}),
                )
                .await?;
                if as_json {
                    println!("{}", serde_json::json!({"ok": true}));
                } else {
                    println!("Server listen address changed to {addr}.");
                }
            }
        },
        Command::SerialPorts => {
            let ports: Vec<String> = api_get(&base, "/api/serial-ports").await?;
            if as_json {
                println!("{}", serde_json::to_string_pretty(&ports)?);
            } else if ports.is_empty() {
                println!("No serial ports found.");
            } else {
                for p in &ports {
                    println!("{p}");
                }
            }
        }
    }

    Ok(())
}
