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
use crate::protocol::responses::{ApiResponse, DeviceInfo, DeviceListResponse, EmptyResponse, ForegroundResponse, PairResponse};
use crate::server::AppState;

pub mod auth {
    use super::*;

    pub async fn pair(
        Extension(state): Extension<AppState>,
        Json(req): Json<AuthPairRequest>,
    ) -> impl IntoResponse {
        tracing::info!("[远程控制] 配对请求，PIN: {}", req.pin);
        if state.service.verify_pin(&req.pin) {
            let device_name = format!("手机-{}", &req.pin);
            // 创建永久 Token，永不过期
            let token = state.token_store.create_permanent_token(&device_name);
            let expires_at = i64::MAX;
            // Register the paired device in service state
            state
                .service
                .register_paired_device(device_name, token.clone(), expires_at);
            // 获取本机 MAC 地址，用于 Wake-on-LAN
            let mac_address = state.system_service.get_mac_address();
            (
                StatusCode::OK,
                Json(ApiResponse::success(PairResponse {
                    token,
                    expires_at,
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

    /// 列出所有已配对设备
    pub async fn list_devices(
        Extension(state): Extension<AppState>,
    ) -> impl IntoResponse {
        let tokens = state.token_store.list_active();
        let devices: Vec<DeviceInfo> = tokens
            .into_iter()
            .map(|(token, info)| DeviceInfo {
                device_name: info.device_name,
                created_at: info.created_at,
                expires_at: info.expires_at,
                permanent: info.permanent,
                token,
            })
            .collect();
        (
            StatusCode::OK,
            Json(ApiResponse::success(DeviceListResponse { devices })),
        )
    }

    /// 通过设备名称撤销配对
    pub async fn revoke_device(
        Extension(state): Extension<AppState>,
        Path(device_name): Path<String>,
    ) -> impl IntoResponse {
        let revoked = state.token_store.revoke_by_name(&device_name);
        if revoked {
            state.service.revoke_device(&device_name);
            (
                StatusCode::OK,
                Json(ApiResponse::success(EmptyResponse {})),
            )
        } else {
            (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<EmptyResponse>::error(
                    "DEVICE_NOT_FOUND",
                    "Device not found",
                )),
            )
        }
    }

    pub async fn verify(
        Extension(state): Extension<AppState>,
        Json(req): Json<AuthVerifyRequest>,
    ) -> impl IntoResponse {
        tracing::info!("[远程控制] 验证令牌请求");
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
        tracing::debug!("[远程控制] 获取系统状态");
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

pub mod scanner {
    use super::*;
    #[allow(unused_imports)]
    use crate::service::app_scanner::AppEntry;

    /// Scan all installed applications
    pub async fn scan_apps(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let apps = state.app_scanner.get_or_scan();
        (StatusCode::OK, Json(ApiResponse::success(apps)))
    }

    /// Rename an application
    pub async fn rename_app(
        Extension(state): Extension<AppState>,
        Json(req): Json<serde_json::Value>,
    ) -> impl IntoResponse {
        let original_name = req["original_name"].as_str().unwrap_or("");
        let new_name = req["new_name"].as_str().unwrap_or("");

        if original_name.is_empty() || new_name.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "INVALID_INPUT",
                    "original_name and new_name are required",
                )),
            );
        }

        state.app_scanner.rename(original_name, new_name);
        (
            StatusCode::OK,
            Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
        )
    }

    /// Launch an application by ID
    pub async fn launch_app(
        Extension(state): Extension<AppState>,
        Path(id): Path<String>,
    ) -> impl IntoResponse {
        let apps = state.app_scanner.get_or_scan();
        let app = apps.iter().find(|a| a.id == id);

        match app {
            Some(app) => {
                let exe_path = app.exe_path.clone();
                let args: Vec<String> = Vec::new();
                match crate::platform::launch_app(&exe_path, &args) {
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
            None => (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "APP_NOT_FOUND",
                    "Application not found",
                )),
            ),
        }
    }

    /// Refresh the app scan cache
    pub async fn refresh_apps(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let apps = state.app_scanner.scan();
        (StatusCode::OK, Json(ApiResponse::success(apps)))
    }
}

pub mod steam {
    use super::*;
    use crate::protocol::responses::{SteamGamesResponse, SteamRefreshResponse};

    /// 获取 Steam 游戏列表
    pub async fn list_games(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let games = state.app_scanner.get_steam_games();
        let libraries = state.app_scanner.get_steam_libraries();
        let steam_path = state.app_scanner.get_steam_path();
        let steam_installed = steam_path.is_some();

        (
            StatusCode::OK,
            Json(ApiResponse::success(SteamGamesResponse {
                steam_installed,
                steam_path,
                libraries,
                total: games.len(),
                games,
            })),
        )
    }

    /// 刷新 Steam 扫描
    pub async fn refresh(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let games = state.app_scanner.refresh_steam();
        (
            StatusCode::OK,
            Json(ApiResponse::success(SteamRefreshResponse {
                scanned: games.len(),
            })),
        )
    }

    /// 启动 Steam 游戏
    pub async fn launch_game(
        Extension(state): Extension<AppState>,
        Path(app_id): Path<u32>,
    ) -> impl IntoResponse {
        let games = state.app_scanner.get_steam_games();
        let game = games.iter().find(|g| g.app_id == app_id);

        match game {
            Some(_) => {
                let steam_url = format!("steam://rungameid/{}", app_id);
                match crate::platform::launch_app(&steam_url, &[]) {
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
            None => (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "GAME_NOT_FOUND",
                    "Steam game not found",
                )),
            ),
        }
    }
}

pub mod custom_dir {
    use super::*;
    use crate::custom_dir::{AddCustomDirRequest, UpdateCustomDirRequest, CustomDirManager, CustomDir};
    use crate::protocol::responses::CustomDirListResponse;

    /// 获取自定义目录列表
    pub async fn list(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let manager = state.app_scanner.custom_dir_manager().lock().unwrap();
        let dirs: Vec<CustomDir> = manager.list().iter().map(|d| (*d).clone()).collect();
        (
            StatusCode::OK,
            Json(ApiResponse::success(CustomDirListResponse { dirs })),
        )
    }

    /// 添加自定义目录
    pub async fn add(
        Extension(state): Extension<AppState>,
        Json(req): Json<AddCustomDirRequest>,
    ) -> impl IntoResponse {
        let manager = state.app_scanner.custom_dir_manager();
        let mut manager = manager.lock().unwrap();
        match manager.add(req) {
            Ok(dir) => (
                StatusCode::CREATED,
                Json(ApiResponse::success(dir)),
            ),
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<crate::custom_dir::CustomDir>::error(
                    "ADD_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    /// 更新自定义目录
    pub async fn update(
        Extension(state): Extension<AppState>,
        Path(id): Path<String>,
        Json(req): Json<UpdateCustomDirRequest>,
    ) -> impl IntoResponse {
        let manager = state.app_scanner.custom_dir_manager();
        let mut manager = manager.lock().unwrap();
        match manager.update(&id, req) {
            Ok(dir) => (
                StatusCode::OK,
                Json(ApiResponse::success(dir)),
            ),
            Err(e) => (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<crate::custom_dir::CustomDir>::error(
                    "UPDATE_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    /// 删除自定义目录
    pub async fn remove(
        Extension(state): Extension<AppState>,
        Path(id): Path<String>,
    ) -> impl IntoResponse {
        let manager = state.app_scanner.custom_dir_manager();
        let mut manager = manager.lock().unwrap();
        match manager.remove(&id) {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::NOT_FOUND,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "DELETE_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    /// 验证目录是否可访问
    pub async fn validate(Json(req): Json<serde_json::Value>) -> impl IntoResponse {
        let path = req["path"].as_str().unwrap_or("");
        if path.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<crate::custom_dir::DirValidationResult>::error(
                    "INVALID_INPUT",
                    "path is required",
                )),
            );
        }

        let result = CustomDirManager::validate(path);
        (
            StatusCode::OK,
            Json(ApiResponse::success(result)),
        )
    }
}

pub mod task {
    use super::*;

    /// List all running processes (task manager)
    pub async fn list_tasks(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let mut pm = state.process_manager.lock().unwrap();
        let result = pm.list_processes(None, 0, 200);
        (StatusCode::OK, Json(ApiResponse::success(result)))
    }

    /// Kill a process by PID
    pub async fn kill_task(
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

    /// Set process priority
    pub async fn set_priority(
        Extension(state): Extension<AppState>,
        Path(pid): Path<u32>,
        Json(req): Json<serde_json::Value>,
    ) -> impl IntoResponse {
        let priority = req["priority"].as_str().unwrap_or("normal");
        let pm = state.process_manager.lock().unwrap();
        match pm.set_process_priority(pid, priority) {
            Ok(()) => (
                StatusCode::OK,
                Json(ApiResponse::success(crate::protocol::responses::EmptyResponse {})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<crate::protocol::responses::EmptyResponse>::error(
                    "PRIORITY_FAILED",
                    &e.to_string(),
                )),
            ),
        }
    }

    /// Get system stats (CPU, memory, etc.)
    pub async fn system_stats(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let status = state.system_service.get_status();
        (StatusCode::OK, Json(ApiResponse::success(status)))
    }
}

pub mod web {
    use super::*;

    /// Serve the mobile-friendly web interface
    pub async fn index() -> impl IntoResponse {
        let html = include_str!("../assets/mobile.html");
        axum::response::Html(html)
    }

    /// Get apps formatted for mobile display
    pub async fn mobile_apps(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let apps = state.app_scanner.get_or_scan();
        let mobile_apps: Vec<MobileAppEntry> = apps
            .into_iter()
            .map(|a| MobileAppEntry {
                id: a.id,
                name: a.name,
                original_name: a.original_name,
                icon: a.icon_base64,
                category: a.category,
                source: a.source,
                exe_path: a.exe_path,
            })
            .collect();
        (StatusCode::OK, Json(ApiResponse::success(mobile_apps)))
    }

    /// Get tasks formatted for mobile display
    pub async fn mobile_tasks(Extension(state): Extension<AppState>) -> impl IntoResponse {
        let mut pm = state.process_manager.lock().unwrap();
        let result = pm.list_processes(None, 0, 200);
        (StatusCode::OK, Json(ApiResponse::success(result)))
    }

    #[derive(serde::Serialize)]
    struct MobileAppEntry {
        id: String,
        name: String,
        original_name: String,
        icon: Option<String>,
        category: String,
        source: String,
        exe_path: String,
    }
}
