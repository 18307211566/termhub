use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::app_logic;
use crate::state::AppState;

#[tauri::command]
pub async fn set_listen_addr(addr: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    app_logic::set_listen_addr(&state, &addr)
        .await
        .map_err(|e| e.to_string())
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
