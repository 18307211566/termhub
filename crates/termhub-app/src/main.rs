#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod api_server;
mod app_logic;
mod commands;
mod commands_settings;
mod hostkey;
mod state;

use std::collections::HashMap;
use std::sync::Arc;

use tauri::Manager;

use commands::{
    create_session, delete_session, get_server_info, kick_client, list_clients, list_serial_ports,
    list_sessions, restart_session, spawn_session_status_listener, stop_session, update_session,
};
use commands_settings::{get_autostart, set_autostart, set_listen_addr};
use state::{AppState, RunnerEntry};
use termhub_core::{
    start_session, RunnerConfig, SessionConfig, SessionMgr, UpstreamSpec,
};
use termhub_drivers::factory::create_driver;
use termhub_sshd::{start as sshd_start, ServerConfig as SshdConfig};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tauri::async_runtime::set(tokio::runtime::Handle::current());

    let cfg_dir = termhub_core::default_config_dir()?;
    std::fs::create_dir_all(&cfg_dir)?;

    let log_dir = cfg_dir.join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let log_file = tracing_appender::rolling::daily(&log_dir, "termhub.log");
    let (non_blocking, log_guard) = tracing_appender::non_blocking(log_file);
    FmtSubscriber::builder()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(non_blocking)
        .init();

    std::panic::set_hook(Box::new(|info| {
        let bt = std::backtrace::Backtrace::force_capture();
        tracing::error!("PANIC: {info}\n{bt}");
    }));

    let stored = termhub_core::load_or_default(&cfg_dir)?;

    let host_key_path = cfg_dir.join(&stored.server.host_key_path);
    let host_keys = hostkey::load_or_create(&host_key_path)?;
    let fpr = hostkey::fingerprint(&host_keys[0])?;
    tracing::info!(fingerprint=%fpr, "host keys ready ({} keys)", host_keys.len());

    let mgr = Arc::new(SessionMgr::new());

    let mut runners_map: HashMap<String, RunnerEntry> = HashMap::new();

    let mut sessions_to_start = stored.sessions.clone();
    if !sessions_to_start
        .iter()
        .any(|s| s.name.eq_ignore_ascii_case("echo"))
    {
        sessions_to_start.insert(
            0,
            SessionConfig {
                name: "echo".into(),
                password: "pass".into(),
                auto_reconnect: false,
                pty_override: None,
                upstream: UpstreamSpec::Loopback,
            },
        );
    }

    for s in sessions_to_start {
        let spec = s.upstream.clone();
        let factory: termhub_core::runner::DriverFactory =
            Box::new(move || create_driver(&spec).expect("driver factory"));
        let started = start_session(
            mgr.clone(),
            s.name.clone(),
            s.password.clone(),
            factory,
            RunnerConfig {
                auto_reconnect: s.auto_reconnect,
                max_attempts: None,
            },
        )
        .await?;
        runners_map.insert(
            s.name.to_ascii_lowercase(),
            RunnerEntry {
                started,
                config: s,
            },
        );
    }

    if let Ok(port) = std::env::var("TERMHUB_DEV_SERIAL") {
        let baud: u32 = std::env::var("TERMHUB_DEV_BAUD")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(115200);
        let spec = UpstreamSpec::Serial {
            port,
            baud,
            data_bits: 8,
            parity: "none".into(),
            stop_bits: 1,
            flow: "none".into(),
            input_eol: "as_is".into(),
            output_eol: "as_is".into(),
        };
        let cfg = SessionConfig {
            name: "com".into(),
            password: "pass".into(),
            auto_reconnect: true,
            pty_override: None,
            upstream: spec.clone(),
        };
        let spec2 = spec.clone();
        let factory: termhub_core::runner::DriverFactory =
            Box::new(move || create_driver(&spec2).expect("driver factory"));
        let started = start_session(
            mgr.clone(),
            cfg.name.clone(),
            cfg.password.clone(),
            factory,
            RunnerConfig {
                auto_reconnect: true,
                max_attempts: None,
            },
        )
        .await?;
        runners_map.insert(cfg.name.to_ascii_lowercase(), RunnerEntry { started, config: cfg });
    }

    let sshd_cancel = tokio_util::sync::CancellationToken::new();
    let host_keys_clone = host_keys.clone();
    let (sshd_handle, addr) = sshd_start(
        SshdConfig {
            listen: stored.server.listen.clone(),
            host_keys: host_keys_clone,
            max_clients_per_session: stored.server.max_clients_per_session,
        },
        mgr.clone(),
        sshd_cancel.clone(),
    )
    .await?;

    let app_state = AppState::new(
        mgr.clone(),
        addr,
        fpr,
        cfg_dir,
        stored.server.max_clients_per_session,
        sshd_cancel,
        sshd_handle,
        host_keys,
        stored.server.api_listen.clone(),
    );
    {
        let mut w = app_state.runners.write().await;
        for (k, v) in runners_map {
            w.insert(k, v);
        }
    }

    let first_listen = app_state.server_listen_addr.read().await.to_string();
    tracing::info!(
        "sshd on {first_listen}, example: ssh echo@127.0.0.1 -p {} (password: pass)",
        addr.port()
    );

    // Start HTTP API server for CLI access
    {
        let api_state = app_state.clone();
        let api_listen = stored.server.api_listen.clone();
        tokio::spawn(async move {
            match tokio::net::TcpListener::bind(&api_listen).await {
                Ok(listener) => {
                    tracing::info!("HTTP API listening on {api_listen}");
                    if let Err(e) = axum::serve(listener, api_server::api_router(api_state)).await {
                        tracing::error!("HTTP API server error: {e}");
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to bind HTTP API on {api_listen}: {e}");
                }
            }
        });
    }

    let runners_for_status = app_state.runners.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            create_session,
            stop_session,
            delete_session,
            restart_session,
            update_session,
            list_clients,
            kick_client,
            list_serial_ports,
            get_server_info,
            set_listen_addr,
            get_autostart,
            set_autostart,
        ])
        .setup(move |app| {
            let app_h = app.handle().clone();
            let runners_arc = runners_for_status.clone();
            tauri::async_runtime::spawn(async move {
                let snapshot = runners_arc.read().await;
                for (_k, entry) in snapshot.iter() {
                    spawn_session_status_listener(
                        app_h.clone(),
                        entry.config.name.clone(),
                        entry.started.status_rx.clone(),
                    );
                }
            });

            let show_i = tauri::menu::MenuItem::with_id(app, "show", "打开主窗口", true, None::<&str>)?;
            let quit_i = tauri::menu::MenuItem::with_id(app, "quit", "完全退出", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&show_i, &quit_i])?;
            let icon = app.default_window_icon().cloned().ok_or_else(|| {
                anyhow::anyhow!("missing default window icon")
            })?;
            let _tray = tauri::tray::TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .on_menu_event(move |app, e| {
                    match e.id.as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|win, evt| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = evt {
                api.prevent_close();
                let _ = win.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("tauri run");

    drop(log_guard);
    Ok(())
}
