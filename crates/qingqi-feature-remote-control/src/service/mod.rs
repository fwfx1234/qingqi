pub mod process;
pub mod system;

use std::sync::Arc;

use qingqi_plugin::storage::AppPaths;

use self::process::ProcessManager;
use self::system::SystemService;

pub struct RemoteControlService {
    paths: AppPaths,
    system_service: Arc<SystemService>,
    process_manager: Arc<std::sync::Mutex<ProcessManager>>,
    inner: Arc<std::sync::Mutex<ServiceInner>>,
}

#[derive(Default)]
struct ServiceInner {
    /// The current pairing PIN (6-digit). Empty if not in pairing mode.
    pairing_pin: Option<String>,
    /// Whether the HTTP server is running.
    server_running: bool,
    /// The port the server listens on.
    server_port: u16,
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
}
