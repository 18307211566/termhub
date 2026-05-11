use axum::extract::{Path, State as AxumState};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Serialize;
use termhub_core::SessionConfig;

use crate::app_logic::{self, AppError};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// JSON envelope
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn ok<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        ok: true,
        data: Some(data),
        error: None,
    })
}

fn err(status: StatusCode, msg: String) -> (StatusCode, Json<ApiResponse<()>>) {
    (
        status,
        Json(ApiResponse {
            ok: false,
            data: None,
            error: Some(msg),
        }),
    )
}

fn map_err(e: AppError) -> (StatusCode, Json<ApiResponse<()>>) {
    match e {
        AppError::NotFound(m) => err(StatusCode::NOT_FOUND, m),
        AppError::Internal(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn list_sessions(
    AxumState(state): AxumState<AppState>,
) -> Result<Json<ApiResponse<Vec<app_logic::SessionView>>>, (StatusCode, Json<ApiResponse<()>>)> {
    app_logic::list_sessions(&state).await.map(ok).map_err(map_err)
}

async fn create_session(
    AxumState(state): AxumState<AppState>,
    Json(config): Json<SessionConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    let (name, _status_rx) = app_logic::create_session(&state, config)
        .await
        .map_err(map_err)?;
    // The status_rx is dropped here; the runner task keeps running.
    // CLI clients poll GET /api/sessions for status.
    tracing::info!(%name, "session created via API");
    Ok(ok(()))
}

async fn stop_session(
    AxumState(state): AxumState<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    app_logic::stop_session(&state, &name)
        .await
        .map(ok)
        .map_err(map_err)
}

async fn restart_session(
    AxumState(state): AxumState<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    let _status_rx = app_logic::restart_session(&state, &name)
        .await
        .map_err(map_err)?;
    Ok(ok(()))
}

async fn update_session(
    AxumState(state): AxumState<AppState>,
    Path(name): Path<String>,
    Json(config): Json<SessionConfig>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    let _status_rx = app_logic::update_session(&state, &name, config)
        .await
        .map_err(map_err)?;
    Ok(ok(()))
}

async fn delete_session(
    AxumState(state): AxumState<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    app_logic::delete_session(&state, &name)
        .await
        .map(ok)
        .map_err(map_err)
}

async fn list_clients(
    AxumState(state): AxumState<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<Vec<app_logic::ClientView>>>, (StatusCode, Json<ApiResponse<()>>)> {
    app_logic::list_clients(&state, &name)
        .await
        .map(ok)
        .map_err(map_err)
}

async fn kick_client(
    AxumState(state): AxumState<AppState>,
    Path((session, id)): Path<(String, u64)>,
) -> Result<Json<ApiResponse<bool>>, (StatusCode, Json<ApiResponse<()>>)> {
    app_logic::kick_client(&state, &session, id)
        .await
        .map(ok)
        .map_err(map_err)
}

async fn get_server_info(
    AxumState(state): AxumState<AppState>,
) -> Result<Json<ApiResponse<app_logic::ServerInfo>>, (StatusCode, Json<ApiResponse<()>>)> {
    app_logic::get_server_info(&state)
        .await
        .map(ok)
        .map_err(map_err)
}

async fn set_listen_addr(
    AxumState(state): AxumState<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<()>>, (StatusCode, Json<ApiResponse<()>>)> {
    let addr = body
        .get("addr")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err(StatusCode::BAD_REQUEST, "missing 'addr' field".into()))?;
    app_logic::set_listen_addr(&state, addr)
        .await
        .map(ok)
        .map_err(map_err)
}

async fn list_serial_ports() -> Result<Json<ApiResponse<Vec<String>>>, (StatusCode, Json<ApiResponse<()>>)> {
    app_logic::list_serial_ports().map(ok).map_err(map_err)
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn api_router(state: AppState) -> Router {
    Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/sessions/:name",
            put(update_session).delete(delete_session),
        )
        .route("/api/sessions/:name/stop", post(stop_session))
        .route("/api/sessions/:name/restart", post(restart_session))
        .route("/api/sessions/:name/clients", get(list_clients))
        .route(
            "/api/sessions/:name/clients/:id/kick",
            post(kick_client),
        )
        .route("/api/server", get(get_server_info))
        .route("/api/server/listen", put(set_listen_addr))
        .route("/api/serial-ports", get(list_serial_ports))
        .with_state(state)
}
