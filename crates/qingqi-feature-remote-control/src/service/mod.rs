pub mod app_scanner;
pub mod process;
pub mod system;

use std::sync::Arc;

use qingqi_plugin::storage::AppPaths;

use self::process::ProcessManager;
use self::system::SystemService;

#[derive(Clone)]
pub struct RemoteControlService {
    paths: AppPaths,
    system_service: Arc<SystemService>,
    process_manager: Arc<std::sync::Mutex<ProcessManager>>,
    inner: Arc<std::sync::Mutex<ServiceInner>>,
}

#[derive(Clone, Debug)]
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
    /// List of paired devices.
    paired_devices: Vec<PairedDevice>,
}

impl RemoteControlService {
    pub fn new(paths: AppPaths) -> Self {
        Self {
            paths,
            system_service: Arc::new(SystemService::new()),
            process_manager: Arc::new(std::sync::Mutex::new(ProcessManager::new())),
            inner: Arc::new(std::sync::Mutex::new(ServiceInner {
                pairing_pin: None,
                server_running: false,
                server_port: 3721,
                paired_devices: Vec::new(),
            })),
        }
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

    /// List all paired devices (active tokens).
    pub fn list_paired_devices(&self) -> Vec<PairedDevice> {
        // We need access to the token store. For now, we'll store paired devices in the inner state.
        let inner = self.inner.lock().unwrap();
        inner.paired_devices.clone()
    }

    /// Register a newly paired device.
    pub fn register_paired_device(&self, name: String, token: String, expires_at: i64) {
        let mut inner = self.inner.lock().unwrap();
        // Remove existing device with same name
        inner.paired_devices.retain(|d| d.name != name);
        inner.paired_devices.push(PairedDevice {
            name,
            token,
            expires_at,
            paired_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        });
    }

    /// Revoke a paired device by name.
    pub fn revoke_device(&self, name: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let len_before = inner.paired_devices.len();
        inner.paired_devices.retain(|d| d.name != name);
        inner.paired_devices.len() < len_before
    }

    /// Check if there are any paired devices.
    pub fn has_paired_devices(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.paired_devices.is_empty()
    }
}
