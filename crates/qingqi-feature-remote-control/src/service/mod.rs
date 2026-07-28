//! 核心服务 - SQLite 持久化

pub mod app_scanner;
pub mod process;
pub mod system;
pub mod window_monitor;
pub mod file_browser;

use std::path::PathBuf;
use std::sync::Arc;

use qingqi_plugin::{database::DatabaseService, storage::AppPaths};
use serde::{Deserialize, Serialize};

use self::process::ProcessManager;
use self::system::SystemService;

#[derive(Clone)]
pub struct RemoteControlService {
    paths: AppPaths,
    system_service: Arc<SystemService>,
    process_manager: Arc<std::sync::Mutex<ProcessManager>>,
    inner: Arc<std::sync::Mutex<ServiceInner>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairedDevice {
    pub name: String,
    pub token: String,
    pub expires_at: i64,
    pub paired_at: i64,
}

#[derive(Default)]
struct ServiceInner {
    /// The current pairing PIN (6-digit). Empty if not in pairing mode.
    pairing_pin: Option<String>,
    /// Whether the HTTP server is running.
    server_running: bool,
    /// The port the server listens on.
    server_port: u16,
    /// 服务器设置
    auto_start: bool,
    minimize_to_tray: bool,
    /// 本地 IP 地址
    local_ip: String,
}

impl RemoteControlService {
    pub fn new(paths: AppPaths) -> Self {
        let local_ip = Self::obtain_local_ip();
        Self {
            paths,
            system_service: Arc::new(SystemService::new()),
            process_manager: Arc::new(std::sync::Mutex::new(ProcessManager::new())),
            inner: Arc::new(std::sync::Mutex::new(ServiceInner {
                pairing_pin: None,
                server_running: false,
                server_port: 3721,
                auto_start: false,
                minimize_to_tray: false,
                local_ip,
            })),
        }
    }

    /// 获取本地 IP 地址
    fn obtain_local_ip() -> String {
        use local_ip_address::local_ip;
        local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "127.0.0.1".to_string())
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn system_service(&self) -> &SystemService {
        &self.system_service
    }

    pub fn process_manager(&self) -> &std::sync::Mutex<ProcessManager> {
        &self.process_manager
    }

    pub fn generate_pin(&self) -> String {
        let uuid = uuid::Uuid::new_v4();
        let bytes = uuid.as_bytes();
        let pin: String = (0..6)
            .map(|i| {
                let idx = (bytes[i] % 10) as u8;
                (b'0' + idx) as char
            })
            .collect();
        let mut inner = self.inner.lock().unwrap();
        inner.pairing_pin = Some(pin.clone());
        pin
    }

    pub fn verify_pin(&self, pin: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        if inner.pairing_pin.as_deref() == Some(pin) {
            inner.pairing_pin = None;
            true
        } else {
            false
        }
    }

    pub fn set_server_running(&self, running: bool, port: u16) {
        let mut inner = self.inner.lock().unwrap();
        inner.server_running = running;
        inner.server_port = port;
    }

    pub fn is_server_running(&self) -> bool {
        self.inner.lock().unwrap().server_running
    }

    pub fn server_port(&self) -> u16 {
        self.inner.lock().unwrap().server_port
    }

    /// 获取当前 PIN 码
    pub fn get_current_pin(&self) -> Option<String> {
        self.inner.lock().unwrap().pairing_pin.clone()
    }

    /// 获取本地 IP
    pub fn local_ip(&self) -> String {
        self.inner.lock().unwrap().local_ip.clone()
    }

    /// 获取服务器设置
    pub fn get_settings(&self) -> crate::protocol::responses::ServerSettings {
        let inner = self.inner.lock().unwrap();
        crate::protocol::responses::ServerSettings {
            port: inner.server_port,
            auto_start: inner.auto_start,
            minimize_to_tray: inner.minimize_to_tray,
        }
    }

    /// 更新服务器设置
    pub fn update_settings(&self, req: crate::protocol::requests::UpdateSettingsRequest) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(port) = req.port {
            inner.server_port = port;
        }
        if let Some(auto_start) = req.auto_start {
            inner.auto_start = auto_start;
        }
        if let Some(minimize_to_tray) = req.minimize_to_tray {
            inner.minimize_to_tray = minimize_to_tray;
        }
    }

    /// 列出已配对设备
    pub fn list_paired_devices(&self, database: &DatabaseService) -> Vec<PairedDevice> {
        let key = "remote-control/data";
        let conn = match database.connection(key) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut stmt = match conn.prepare(
            "SELECT device_name, token, expires_at, created_at FROM tokens ORDER BY created_at DESC",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = match stmt.query_map([], |row| {
            Ok(PairedDevice {
                name: row.get(0)?,
                token: row.get(1)?,
                expires_at: row.get(2)?,
                paired_at: row.get(3)?,
            })
        }) {
            Ok(rows) => rows,
            Err(_) => return Vec::new(),
        };
        rows.filter_map(|r| r.ok()).collect()
    }

    /// 撤销设备配对
    pub fn revoke_device(&self, name: &str, database: &DatabaseService) -> bool {
        let key = "remote-control/data";
        let conn = match database.connection(key) {
            Ok(c) => c,
            Err(_) => return false,
        };
        let affected = conn
            .execute("DELETE FROM tokens WHERE device_name = ?1", [name])
            .unwrap_or(0);
        affected > 0
    }
}
