use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use termhub_core::SessionConfig;

use crate::app_logic;
use crate::state::AppState;

#[derive(Clone, Serialize)]
pub struct StatusEvent {
    pub name: String,
    pub status: termhub_core::SessionStatus,
}

pub fn spawn_session_status_listener(app: AppHandle, name: String, mut rx: termhub_core::StatusRx) {
    tokio::spawn(async move {
        loop {
            let status = rx.borrow().clone();
            let _ = app.emit(
                "session:status",
                StatusEvent {
                    name: name.clone(),
                    status,
                },
            );
            if rx.changed().await.is_err() {
                break;
            }
        }
    });
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<app_logic::SessionView>, String> {
    app_logic::list_sessions(&state).await.map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct CreateSessionArgs {
    pub config: SessionConfig,
}

#[tauri::command]
pub async fn create_session(
    args: CreateSessionArgs,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let name = args.config.name.clone();
    let (_name, status_rx) = app_logic::create_session(&state, args.config)
        .await
        .map_err(|e| e.to_string())?;
    spawn_session_status_listener(app, name, status_rx);
    Ok(())
}

#[tauri::command]
pub async fn stop_session(name: String, state: State<'_, AppState>) -> Result<(), String> {
    app_logic::stop_session(&state, &name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_session(name: String, state: State<'_, AppState>) -> Result<(), String> {
    app_logic::delete_session(&state, &name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restart_session(
    name: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let status_rx = app_logic::restart_session(&state, &name)
        .await
        .map_err(|e| e.to_string())?;
    spawn_session_status_listener(app, name, status_rx);
    Ok(())
}

#[derive(Deserialize)]
pub struct UpdateSessionArgs {
    pub name: String,
    pub config: SessionConfig,
}

#[tauri::command]
pub async fn update_session(
    args: UpdateSessionArgs,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    let new_name = args.config.name.clone();
    let status_rx = app_logic::update_session(&state, &args.name, args.config)
        .await
        .map_err(|e| e.to_string())?;
    spawn_session_status_listener(app, new_name, status_rx);
    Ok(())
}

#[tauri::command]
pub async fn list_clients(
    name: String,
    state: State<'_, AppState>,
) -> Result<Vec<app_logic::ClientView>, String> {
    app_logic::list_clients(&state, &name)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn kick_client(
    session: String,
    client_id: u64,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    app_logic::kick_client(&state, &session, client_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_server_info(
    state: State<'_, AppState>,
) -> Result<app_logic::ServerInfo, String> {
    app_logic::get_server_info(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_serial_ports() -> Result<Vec<String>, String> {
    app_logic::list_serial_ports().map_err(|e| e.to_string())
}
