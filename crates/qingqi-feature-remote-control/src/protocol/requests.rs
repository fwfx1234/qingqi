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

// === Server settings ===

#[derive(Debug, Default, Deserialize)]
pub struct UpdateSettingsRequest {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub auto_start: Option<bool>,
    #[serde(default)]
    pub minimize_to_tray: Option<bool>,
}

// === Windows ===

#[derive(Debug, Clone, Serialize)]
pub struct WindowInfo {
    pub id: usize,
    pub title: String,
    pub pid: u32,
    pub exe_path: Option<String>,
    pub is_visible: bool,
    pub is_foreground: bool,
    pub is_fullscreen: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowListResponse {
    pub windows: Vec<WindowInfo>,
    pub active_id: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct FocusWindowRequest {
    pub id: usize,
}

// === Files ===

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<u64>,
    pub is_hidden: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectoryListing {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<FileEntry>,
    pub total_count: usize,
}

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub path: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_auth_pair_request() {
        let json = r#"{"pin": "123456"}"#;
        let req: AuthPairRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.pin, "123456");
    }

    #[test]
    fn deserialize_shutdown_request_default() {
        let json = r#"{}"#;
        let req: ShutdownRequest = serde_json::from_str(json).unwrap();
        assert!(!req.force);
        assert_eq!(req.delay_secs, 0);
    }

    #[test]
    fn deserialize_shutdown_request_with_fields() {
        let json = r#"{"force": true, "delay_secs": 30}"#;
        let req: ShutdownRequest = serde_json::from_str(json).unwrap();
        assert!(req.force);
        assert_eq!(req.delay_secs, 30);
    }

    #[test]
    fn deserialize_sleep_request() {
        let json = r#"{"hibernate": true}"#;
        let req: SleepRequest = serde_json::from_str(json).unwrap();
        assert!(req.hibernate);
    }

    #[test]
    fn deserialize_process_list_query_default() {
        let json = r#"{}"#;
        let req: ProcessListQuery = serde_json::from_str(json).unwrap();
        assert_eq!(req.search, None);
        assert_eq!(req.page, 0);
        assert_eq!(req.page_size, 50);
    }

    #[test]
    fn deserialize_process_list_query_with_search() {
        let json = r#"{"search": "notepad", "page": 1, "page_size": 20}"#;
        let req: ProcessListQuery = serde_json::from_str(json).unwrap();
        assert_eq!(req.search, Some("notepad".to_string()));
        assert_eq!(req.page, 1);
        assert_eq!(req.page_size, 20);
    }

    #[test]
    fn deserialize_launch_app_request() {
        let json = r#"{"path": "C:\\Windows\\System32\\notepad.exe", "args": ["file.txt"]}"#;
        let req: LaunchAppRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.path, "C:\\Windows\\System32\\notepad.exe");
        assert_eq!(req.args, vec!["file.txt".to_string()]);
    }

    #[test]
    fn deserialize_launch_app_request_no_args() {
        let json = r#"{"path": "C:\\Windows\\System32\\calc.exe"}"#;
        let req: LaunchAppRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.path, "C:\\Windows\\System32\\calc.exe");
        assert!(req.args.is_empty());
    }

    #[test]
    fn deserialize_add_custom_dir_request() {
        let json = r#"{"path": "D:\\Games", "name": "游戏目录", "enabled": true, "max_depth": 3, "recursive": true}"#;
        let req: AddCustomDirRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.path, "D:\\Games");
        assert_eq!(req.name, Some("游戏目录".to_string()));
        assert!(req.enabled);
        assert_eq!(req.max_depth, 3);
        assert!(req.recursive);
    }

    #[test]
    fn deserialize_update_custom_dir_request() {
        let json = r#"{"name": "新名称", "enabled": false}"#;
        let req: UpdateCustomDirRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("新名称".to_string()));
        assert_eq!(req.enabled, Some(false));
        assert_eq!(req.path, None);
    }

    #[test]
    fn deserialize_update_settings_request() {
        let json = r#"{"port": 8080, "auto_start": true, "minimize_to_tray": false}"#;
        let req: UpdateSettingsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.port, Some(8080));
        assert_eq!(req.auto_start, Some(true));
        assert_eq!(req.minimize_to_tray, Some(false));
    }

    #[test]
    fn serialize_ws_event_foreground() {
        let event = WsEvent::ForegroundChanged {
            data: ForegroundInfo {
                pid: 1234,
                title: "测试窗口".to_string(),
                path: Some("C:\\test.exe".to_string()),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"foreground_changed\""));
        assert!(json.contains("\"pid\":1234"));
        assert!(json.contains("测试窗口"));
    }

    #[test]
    fn serialize_ws_event_process_started() {
        let event = WsEvent::ProcessStarted {
            data: ProcessBasic {
                pid: 5678,
                name: "test.exe".to_string(),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"process_started\""));
        assert!(json.contains("\"pid\":5678"));
        assert!(json.contains("test.exe"));
    }
}
