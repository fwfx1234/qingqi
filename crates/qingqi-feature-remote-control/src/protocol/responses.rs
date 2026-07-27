use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(code: &str, message: &str) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

// === Auth responses ===

#[derive(Debug, Serialize)]
pub struct PairResponse {
    pub token: String,
    pub expires_at: i64,
    /// PC 的 MAC 地址，用于 Wake-on-LAN 魔法封唤醒
    pub mac_address: Option<String>,
}

// === System responses ===

#[derive(Debug, Serialize)]
pub struct SystemStatus {
    pub cpu_usage: f32,
    pub memory_total: u64,
    pub memory_used: u64,
    pub memory_percent: f32,
    pub uptime_seconds: u64,
}

// === Process responses ===

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub memory_bytes: u64,
    pub cpu_usage: f32,
    pub path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProcessListResponse {
    pub processes: Vec<ProcessInfo>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Serialize)]
pub struct ForegroundResponse {
    pub pid: u32,
    pub title: String,
    pub path: Option<String>,
    pub process_name: String,
}

// === App responses ===

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct SearchAppsResponse {
    pub apps: Vec<AppInfo>,
}

// === Device management responses ===

#[derive(Debug, Serialize)]
pub struct DeviceInfo {
    pub device_name: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub permanent: bool,
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct DeviceListResponse {
    pub devices: Vec<DeviceInfo>,
}

// === Steam responses ===

#[derive(Debug, Serialize)]
pub struct SteamGamesResponse {
    pub steam_installed: bool,
    pub steam_path: Option<String>,
    pub libraries: Vec<crate::steam::SteamLibrary>,
    pub games: Vec<crate::steam::SteamGame>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct SteamRefreshResponse {
    pub scanned: usize,
}

// === Custom directory responses ===

#[derive(Debug, Serialize)]
pub struct CustomDirListResponse {
    pub dirs: Vec<crate::custom_dir::CustomDir>,
}

// === Common responses ===

#[derive(Debug, Serialize)]
pub struct EmptyResponse {}
