use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::state::AppState;

#[tauri::command]
pub async fn set_listen_addr(addr: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let _parsed: std::net::SocketAddr = addr.parse().map_err(|e: std::net::AddrParseError| e.to_string())?;

    // 1. Cancel the old accept loop
    {
        let sshd = state.sshd.read().await;
        sshd.cancel.cancel();
    }

    // 2. Await the old handle
    let old_handle = {
        let mut sshd = state.sshd.write().await;
        let handle = std::mem::replace(&mut sshd.handle, tokio::spawn(async {}));
        handle
    };
    let _ = old_handle.await;

    // 3. Start a new accept loop on the new address
    let new_cancel = tokio_util::sync::CancellationToken::new();
    let host_keys_clone = {
        let sshd = state.sshd.read().await;
        sshd.host_keys.clone()
    };

    let cfg = termhub_sshd::ServerConfig {
        listen: addr.clone(),
        host_keys: host_keys_clone,
        max_clients_per_session: state.max_clients_per_session,
    };
    let (new_handle, new_addr) = termhub_sshd::start(cfg, state.mgr.clone(), new_cancel.clone())
        .await
        .map_err(|e| e.to_string())?;

    // 4. Store the new state
    {
        let mut sshd = state.sshd.write().await;
        sshd.cancel = new_cancel;
        sshd.handle = new_handle;
    }

    // 5. Update the listen address shown in UI
    *state.server_listen_addr.write().await = new_addr;

    tracing::info!(%new_addr, "sshd rebound");

    let _ = super::commands::persist_now(&state).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    Ok(app.autolaunch().is_enabled().unwrap_or(false))
}

#[tauri::command]
pub async fn set_autostart(enable: bool, app: AppHandle) -> Result<(), String> {
    if enable {
        app.autolaunch().enable().map_err(|e| e.to_string())?;
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}
