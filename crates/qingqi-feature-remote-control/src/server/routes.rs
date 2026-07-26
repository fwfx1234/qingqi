use axum::{
    Extension, Json,
    extract::{Path, Query},
    http::StatusCode,
    response::IntoResponse,
};

use crate::protocol::requests::{
    AuthPairRequest, AuthVerifyRequest, LaunchAppRequest, ProcessListQuery, RestartRequest,
    SearchAppsQuery, ShutdownRequest, SleepRequest,
};
use crate::protocol::responses::{ApiResponse, ForegroundResponse, PairResponse};
use crate::server::AppState;

pub mod auth {
    use super::*;

    pub async fn pair(
        Extension(state): Extension<AppState>,
        Json(req): Json<AuthPairRequest>,
    ) -> impl IntoResponse {
        if state.service.verify_pin(&req.pin) {
            let token = state.token_store.create_token("mobile", 30 * 24 * 3600);
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            // 获取本机 MAC 地址，用于 Wake-on-LAN
            let mac_address = state.system_service.get_mac_address();
            (
                StatusCode::OK,
                Json(ApiResponse::success(PairResponse {
                    token,
                    expires_at: now + 30 * 24 * 3600,
                    mac_address,
                })),
            )
        } else {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<PairResponse>::error(
                    "INVALID_PIN",
                    "Invalid or expired PIN",
                )),
            )
        }
    }

    pub async fn verify(
        Extension(state): Extension<AppState>,
        Json(req): Json<AuthVerifyRequest>,
    ) -> impl IntoResponse {
        if state.token_store.validate(&req.token) {
            (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            )
        } else {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "INVALID_TOKEN",
                    "Token is invalid or expired",
                )),
            )
        }
    }

    pub async fn revoke(
        Extension(state): Extension<AppState>,
        headers: axum::http::HeaderMap,
    ) -> impl IntoResponse {
        if let Some(auth) = headers.get("authorization") {
            if let Ok(auth_str) = auth.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    state.token_store.revoke(token);
                    return (
                        StatusCode::OK,
                        Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
                    );
                }
            }
        }
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                "MISSING_AUTH",
                "Missing authorization header",
            )),
        )
    }
}

pub mod system {
    use super::*;

    pub async fn status(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let status = state.system_service.get_status();
        (StatusCode::OK, Json(ApiResponse::success(status)))
    }

    /// 获取本机 MAC 地址（用于 Wake-on-LAN 魔法封唤醒）
    pub async fn mac_address(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let mac = state.system_service.get_mac_address();
        #[derive(serde::Serialize)]
        struct MacResponse {
            mac_address: Option<String>,
        }
        (StatusCode::OK, Json(ApiResponse::success(MacResponse { mac_address: mac })))
    }

    pub async fn shutdown(
        Extension(state): Extension<AppState>,
        Json(req): Json<ShutdownRequest>,
    ) -> impl IntoResponse {
        match state.system_service.shutdown(req.force, req.delay_secs) {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "SHUTDOWN_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    pub async fn sleep(
        Extension(state): Extension<AppState>,
        Json(req): Json<SleepRequest>,
    ) -> impl IntoResponse {
        match state.system_service.sleep(req.hibernate) {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "SLEEP_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    pub async fn restart(
        Extension(state): Extension<AppState>,
        Json(req): Json<RestartRequest>,
    ) -> impl IntoResponse {
        match state.system_service.restart(req.force) {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "RESTART_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    pub async fn logoff(Extension(state): Extension<AppState>) -> impl IntoResponse {
        match state.system_service.logoff() {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "LOGOFF_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    pub async fn lock(Extension(state): Extension<AppState>) -> impl IntoResponse {
        match state.system_service.lock() {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "LOCK_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }
}

pub mod process {
    use super::*;

    pub async fn list(
        Extension(state): Extension<AppState>,
        Query(query): Query<ProcessListQuery>,
    ) -> impl IntoResponse {
        let mut pm = state.process_manager.lock().unwrap();
        let result = pm.list_processes(query.search.as_deref(), query.page, query.page_size);
        (StatusCode::OK, Json(ApiResponse::success(result)))
    }

    pub async fn foreground(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let pm = state.process_manager.lock().unwrap();
        match pm.get_foreground() {
            Ok(info) => (StatusCode::OK, Json(ApiResponse::success(info))),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<ForegroundResponse>::error(
                    "FOREGROUND_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    pub async fn kill(
        Extension(state): Extension<AppState>,
        Path(pid): Path<u32>,
    ) -> impl IntoResponse {
        let pm = state.process_manager.lock().unwrap();
        match pm.kill_process(pid) {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "KILL_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    pub async fn suspend(
        Extension(state): Extension<AppState>,
        Path(pid): Path<u32>,
    ) -> impl IntoResponse {
        let pm = state.process_manager.lock().unwrap();
        match pm.suspend_process(pid) {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "SUSPEND_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    pub async fn resume(
        Extension(state): Extension<AppState>,
        Path(pid): Path<u32>,
    ) -> impl IntoResponse {
        let pm = state.process_manager.lock().unwrap();
        match pm.resume_process(pid) {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "RESUME_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }
}

pub mod app {
    use super::*;

    pub async fn launch(
        Extension(_state): Extension<AppState>,
        Json(req): Json<LaunchAppRequest>,
    ) -> impl IntoResponse {
        match crate::platform::launch_app(&req.path, &req.args) {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "LAUNCH_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    pub async fn search(
        Extension(_state): Extension<AppState>,
        Query(query): Query<SearchAppsQuery>,
    ) -> impl IntoResponse {
        let apps = crate::platform::search_installed_apps(&query.query);
        (
            StatusCode::OK,
            Json(ApiResponse::success(
                crate::protocol::responses::SearchAppsResponse { apps },
            )),
        )
    }
}
