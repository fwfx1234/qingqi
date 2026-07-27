use serde::{Deserialize, Serialize};

// === Auth ===

#[derive(Debug, Deserialize)]
pub struct AuthPairRequest {
    pub pin: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthVerifyRequest {
    pub token: String,
}

// === System ===

#[derive(Debug, Default, Deserialize)]
pub struct ShutdownRequest {
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub delay_secs: u64,
}

#[derive(Debug, Default, Deserialize)]
pub struct SleepRequest {
    #[serde(default)]
    pub hibernate: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct RestartRequest {
    #[serde(default)]
    pub force: bool,
}

// === Processes ===

#[derive(Debug, Default, Deserialize)]
pub struct ProcessListQuery {
    pub search: Option<String>,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page() -> usize {
    0
}

fn default_page_size() -> usize {
    50
}

// === Apps ===

#[derive(Debug, Deserialize)]
pub struct LaunchAppRequest {
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchAppsQuery {
    pub query: String,
}

// === Custom directories ===

#[derive(Debug, Deserialize)]
pub struct AddCustomDirRequest {
    pub path: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_bool_true")]
    pub enabled: bool,
    #[serde(default)]
    pub max_depth: u32,
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
    #[serde(default = "default_bool_true")]
    pub recursive: bool,
}

fn default_bool_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct UpdateCustomDirRequest {
    pub path: Option<String>,
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub max_depth: Option<u32>,
    pub extensions: Option<Vec<String>>,
    pub recursive: Option<bool>,
}

// === WebSocket events ===

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    ForegroundChanged {
        data: ForegroundInfo,
    },
    ProcessStarted {
        data: ProcessBasic,
    },
    ProcessEnded {
        data: ProcessBasic,
    },
    SystemSuspend,
    SystemResume,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForegroundInfo {
    pub pid: u32,
    pub title: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessBasic {
    pub pid: u32,
    pub name: String,
}
